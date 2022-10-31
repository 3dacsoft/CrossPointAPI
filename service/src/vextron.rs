use std::{
   io::{Write,Read},
   fs,
   time::Duration,
   cmp::min_by
};

use json::JsonValue;
use serialport::{SerialPort,FlowControl,Parity,DataBits,StopBits};
use crate::{
    crosspoint::{CrossPointPreset,CrossPointTie,CrossPointIO},
    config::ConfigurationError
};

const UNASSIGNED: &str = "[unassigned]";

pub struct VirtualCrosspoint {
    buffer: Vec<u8>,
    in_channels: usize,
    out_channels: usize,
    audio: bool,
    current: CrossPointPreset,
    presets: [Option<CrossPointPreset>;32]
}

impl VirtualCrosspoint {
    pub fn load_or_new() -> Box<dyn SerialPort> {

        let newone: VirtualCrosspoint = match fs::read_to_string("virtual.json") {
            Ok(mut f) => {
                match VirtualCrosspoint::parse_json(&f) {
                    Ok(p) => p,
                    Err(_) => VirtualCrosspoint::new()
                }
            }
            Err(_) => VirtualCrosspoint::new()
        };

        Box::new(newone)
    }

    fn parse_json(data: &str) -> Result<VirtualCrosspoint, ConfigurationError> {
        let jconfig = match json::parse(&data) {
            Ok(j) => j,
            Err(_) => return Err(ConfigurationError::new("Invalid JSON"))
        };

        let in_channels = jconfig["input_channels"].as_usize().unwrap_or(16);
        if in_channels > 16  { return Err(ConfigurationError::new("Invalid input port count")); }

        let out_channels = jconfig["output_channels"].as_usize().unwrap_or(16);
        if out_channels > 16  { return Err(ConfigurationError::new("Invalid output port count")); }

        let audio = jconfig["audio_support"].as_bool().unwrap_or(false);

        let ties = Self::get_ties(&jconfig["current_ties"]).unwrap_or(Vec::new());

        let current = CrossPointPreset {
            number: 0,
            name: String::from("Current"),
            ties
        };

        let mut presets:[Option<CrossPointPreset>;32] = [
            None, None, None, None, None, None, None, None,
            None, None, None, None, None, None, None, None,
            None, None, None, None, None, None, None, None,
            None, None, None, None, None, None, None, None
        ];

        for preset in jconfig["presets"].members() {
            let number = match preset["number"].as_usize() { Some(n) => n, None => continue };
            let name = preset["name"].as_str().unwrap_or(UNASSIGNED);
            let ties = Self::get_ties(&preset["ties"]).unwrap_or(Vec::new());
            if ties.len() > 0 {
                presets[number] = Some(CrossPointPreset { number: number as i32, name: String::from(name), ties });
            }
        }

        Ok(VirtualCrosspoint {
            in_channels, out_channels, audio, buffer: Vec::new(), current,
            presets
        })
    }

    fn get_ties(tiesobj: &JsonValue) -> Result<Vec<CrossPointTie>, ConfigurationError> {
        let mut ties: Vec<CrossPointTie> = Vec::new();

        if tiesobj.is_array() {
            for tie in tiesobj.members() {
                if tie.is_empty() { continue; }

                let input_channel = match tie["in"].as_u8() { Some(i) => i, None => continue };
                let output_channel = match tie["out"].as_u8() { Some(i) => i, None => continue };
                ties.push(CrossPointTie { input_channel, output_channel, io_type: CrossPointIO::All });
            }
        }

        Ok(ties)
    }

    fn new() -> VirtualCrosspoint {
        VirtualCrosspoint {
            buffer: Vec::new(),
            in_channels: 12,
            out_channels: 8,
            audio: true,
            current: CrossPointPreset {
                number: 0,
                name: String::from("Current"),
                ties: Vec::new(),
            },
            presets: [
                None, None, None, None, None, None, None, None,
                None, None, None, None, None, None, None, None,
                None, None, None, None, None, None, None, None,
                None, None, None, None, None, None, None, None
            ]
        }
    }

    fn evaluate_command(&mut self, buf: &[u8]) {
        match buf[0] {
            0x49 => self.info(),
            0x1B => self.escape_commands(&buf[1..]),
            _ => todo!()
        }
    }

    fn info(&mut self) {
        let response = format!("V{:0>2}X{:0>2} A{:0>2}X{:0>2}\r\n", self.in_channels, self.out_channels,
            if self.audio { self.in_channels } else { 0 }, if self.audio { self.out_channels } else { 0 });
        let bytes = response.bytes();
        for b in bytes { self.buffer.push(b) }
    }

    fn escape_commands(&mut self, buf: &[u8]) {
        let mut byte = *buf.get(0).unwrap_or(&0);
        let mut x9: usize = 0;
        if byte.is_ascii_digit() { x9 = (byte - 0x30) as usize; }
        byte = *buf.get(1).unwrap_or(&0);
        if byte.is_ascii_digit() { x9 = (x9 * 10) + (byte - 0x30) as usize; }

        if x9 > 0 {
            let c1 = buf.get(2);
            let c2 = buf.get(3);
            if c1.is_some() && *c1.unwrap() == 'N' as u8 && c2.is_some() && *c2.unwrap() == 'G' as u8 {
                self.preset_name(x9);
            }
        }
    }

    fn preset_name(&mut self, preset_number: usize) {
        let preset = &self.presets[preset_number - 1];
         if preset.is_some() {
            let preset = preset.as_ref().unwrap();
            let name = &preset.name;
            for c in name.bytes() {
                self.buffer.push(c);
            }
        } else {
            for c in UNASSIGNED.bytes() {
                self.buffer.push(c);
            }
        }
    }
}

impl Write for VirtualCrosspoint {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.evaluate_command(&buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Read for VirtualCrosspoint {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read_count = min_by(buf.len(), self.buffer.len(), |b, q| b.cmp(q));
        let mut read_range = self.buffer.drain(0..read_count);
        for i in 0..read_count {
            buf[i] = read_range.next().unwrap();
        }
        Ok(read_count)
    }
}

impl SerialPort for VirtualCrosspoint {
    fn name(&self) -> Option<String> {
        Some(String::from("dummy"))
    }

    fn baud_rate(&self) -> serialport::Result<u32> {
        Ok(9600)
    }

    fn data_bits(&self) -> serialport::Result<DataBits> {
        Ok(DataBits::Eight)
    }

    fn flow_control(&self) -> serialport::Result<FlowControl> {
        Ok(FlowControl::None)
    }

    fn parity(&self) -> serialport::Result<Parity> {
        Ok(Parity::None)
    }

    fn stop_bits(&self) -> serialport::Result<StopBits> {
        Ok(StopBits::One)
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn set_baud_rate(&mut self, _: u32) -> serialport::Result<()> {
        Ok(())
    }

    fn set_data_bits(&mut self, _: DataBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_flow_control(&mut self, _: FlowControl) -> serialport::Result<()> {
        Ok(())
    }

    fn set_parity(&mut self, _: Parity) -> serialport::Result<()> {
        Ok(())
    }

    fn set_stop_bits(&mut self, _: StopBits) -> serialport::Result<()> {
        Ok(())
    }

    fn set_timeout(&mut self, _: Duration) -> serialport::Result<()> {
        Ok(())
    }

    fn write_request_to_send(&mut self, level: bool) -> serialport::Result<()> {
        todo!()
    }

    fn write_data_terminal_ready(&mut self, level: bool) -> serialport::Result<()> {
        todo!()
    }

    fn read_clear_to_send(&mut self) -> serialport::Result<bool> {
        Ok(true)
    }

    fn read_data_set_ready(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn read_ring_indicator(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn read_carrier_detect(&mut self) -> serialport::Result<bool> {
        todo!()
    }

    fn bytes_to_read(&self) -> serialport::Result<u32> {
        Ok(self.buffer.len() as u32)
    }

    fn bytes_to_write(&self) -> serialport::Result<u32> {
        todo!()
    }

    fn clear(&self, buffer_to_clear: serialport::ClearBuffer) -> serialport::Result<()> {
        todo!()
    }

    fn try_clone(&self) -> serialport::Result<Box<dyn SerialPort>> {
        todo!()
    }

    fn set_break(&self) -> serialport::Result<()> {
        todo!()
    }

    fn clear_break(&self) -> serialport::Result<()> {
        todo!()
    }
}