use std::{
    io::{Write, BufReader, BufRead, Error},
    time::Duration,
    str::FromStr
};
use serialport::SerialPort;
use crate::vextron::VirtualCrosspoint;

const INFO_CMD: &[u8;3] = b"I\r\n";
const CLEAR_PRESET_CMD: &[u8;9] = b"\x1B+00P0*!\r";
const GET_PRESET_NAME_CMD: &[u8;6] = b"\x1B00NG\r";
const SAVE_CURRENT_CONFIG_CMD: &[u8;5] = b"00,\r\n";
const LOAD_PRESET_CMD: &[u8;5] = b"00.\r\n";

pub struct CrossPoint {
    port: Box<dyn SerialPort>,

    serial_port: String,
    input_count: i32,
    output_count: i32,
    audio_support: bool,
}

impl CrossPoint {
    pub fn connect(port_name: &str) -> Result<CrossPoint, Error> {
        
        let mut port: Box<dyn SerialPort>;
        
        if port_name == "virtual" {
            port = VirtualCrosspoint::load_or_new();
        } else {
            port = serialport::new(port_name, 9600)
                .data_bits(serialport::DataBits::Eight)
                .flow_control(serialport::FlowControl::None)
                .parity(serialport::Parity::None)
                .stop_bits(serialport::StopBits::One)
                .timeout(Duration::from_secs(1))
                .open()?;
        }

        let serial_port = String::from(port_name);
        
        port.write(INFO_CMD)?;
        port.flush()?;

        let mut reader = BufReader::new(port);
        let mut response = String::new();
        reader.read_line(&mut response)?;
        port = reader.into_inner(); //Reclaim ownership of port

        let parts = response.trim().split(' ');
        let mut input_count = 0;
        let mut output_count = 0;
        let mut audio_support = false;
        for part in parts {
            if part.starts_with('V') {
                let mut port_counts = part.trim_start_matches('V').split('X');
                input_count = match port_counts.next().unwrap().parse() { Ok(incount) => incount, _ => 0 };
                output_count = match port_counts.next().unwrap().parse() { Ok(outcount) => outcount, _ => 0 };
            }
            else if part.starts_with('A') {
                audio_support = true;
            }
        }

        Ok(CrossPoint { port, serial_port, input_count, output_count, audio_support })
    }

    pub fn input_port_count(&self) -> i32 { self.input_count }

    pub fn output_port_count(&self) -> i32 { self.output_count }

    pub fn audio_is_supported(&self) -> bool { self.audio_support }

    pub fn port_name(&self) -> &str { &self.serial_port }

    pub fn get_preset_name(&mut self, preset_number: i32) -> Result<String, Error> {
        let mut cmd = GET_PRESET_NAME_CMD.clone();
        cmd[1] = ((preset_number / 10) + 0x30) as u8;
        cmd[2] = ((preset_number % 10) + 0x30) as u8;

        self.send_command(&cmd)
    }

    fn send_command(&mut self, cmd: &[u8]) -> Result<String, Error> {
        //Write command
        self.port.write(&cmd)?;
        self.port.flush()?;

        //Read response
        let mut response: Vec<u8> = Vec::new();
        let mut read_count: usize;
        let mut buffer = [0 as u8; 8];
        'bufferloop: loop  {
            read_count = match self.port.read(&mut buffer) {
                Ok(count) => { 
                    for idx in 0..count {
                        let ch = buffer[idx];
                        if ch == 0x0D {
                            break 'bufferloop
                        } else {
                            response.push(ch);
                        }
                    }
                    count
                }
                Err(_) => 0
            };
            if read_count == 0 { break; }
        }

        Ok(String::from_utf8(response).unwrap())
    }

    pub fn save_current_config(&mut self, preset_number: i32) -> Result<String, Error> {
        let mut cmd= SAVE_CURRENT_CONFIG_CMD.clone();
        cmd[0] = ((preset_number / 10) + 0x30) as u8;
        cmd[1] = ((preset_number % 10) + 0x30) as u8;

        self.send_command(&cmd)
    }

    pub fn load_preset(&mut self, preset_number: i32) -> Result<String, Error> {
        let mut cmd = LOAD_PRESET_CMD.clone();
        cmd[0] = ((preset_number / 10) + 0x30) as u8;
        cmd[1] = ((preset_number % 10) + 0x30) as u8;

        self.send_command(&cmd)
    }

    pub fn clear_preset(&mut self, preset_number: i32) -> Result<String, Error> {
        let mut cmd = CLEAR_PRESET_CMD.clone();
        cmd[2] = ((preset_number / 10) + 0x30) as u8;
        cmd[3] = ((preset_number % 10) + 0x30) as u8;

        self.send_command(&cmd)
    }

    pub fn create_preset(&mut self, new_preset: CrossPointPreset) -> Result<String, Error> {
        //Clear preset first
        self.clear_preset(new_preset.number)?;

        let mut cmd: Vec<u8> = Vec::new();
        cmd.push(0x1B as u8); //Esc
        cmd.push('+' as u8);
        if new_preset.number > 10 {
            cmd.push(((new_preset.number / 10) + 0x30) as u8);
        }
        cmd.push(((new_preset.number % 10) + 0x30) as u8);
        cmd.push('P' as u8);
        for tie in new_preset.ties {
            let in_channel = tie.input_channel;
            if in_channel > 10 {
                cmd.push(((in_channel / 10) + 0x30) as u8);
            }
            cmd.push(((in_channel % 10) + 0x30) as u8);

            cmd.push('*' as u8);

            let out_channel = tie.output_channel;
            if out_channel > 10 {
                cmd.push(((out_channel / 10) + 0x30) as u8);
            }
            cmd.push(((out_channel % 10) + 0x30) as u8);
            cmd.push(tie.io_type.to_char() as u8);
        }
        cmd.push('\r' as u8);

        self.send_command(&cmd)
    }

}

impl CrossPointPreset {
    pub fn from(json_request: json::JsonValue) -> CrossPointPreset {
        let name = json_request["PresetName"].as_str().unwrap().to_string();
        let number = json_request["PresetNumber"].as_i32().unwrap();
        let ties = json_request["Inputs"].entries().map(|i| { 
            CrossPointTie {
                input_channel: i.1["InputChannel"].as_u8().unwrap(),
                output_channel: i.1["OutputChannels"].as_u8().unwrap(),
                io_type: CrossPointIO::from_str(i.1["IOType"].as_str().unwrap_or_default()).unwrap()
            }
        }).collect();

        CrossPointPreset { number, name, ties }
    }
}

pub struct CrossPointPreset {
    pub number: i32,
    pub name: String,
    pub ties: Vec<CrossPointTie>,
}

pub struct  CrossPointTie {
    pub input_channel: u8,
    pub output_channel: u8,
    pub io_type: CrossPointIO
}

pub enum CrossPointIO {
    All,
    RGB,
    Vid,
    Aud,
}

impl CrossPointIO {
    fn to_char(&self) -> char {
        match self {
            CrossPointIO::All => '!',
            CrossPointIO::RGB => '&',
            CrossPointIO::Vid => '%',
            CrossPointIO::Aud => '$'
        }
    }
}

impl FromStr for CrossPointIO {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s  {
            "All" => Ok(CrossPointIO::All),
            "RGB" => Ok(CrossPointIO::RGB),
            "Vid" => Ok(CrossPointIO::Vid),
            "Aud" => Ok(CrossPointIO::Aud),
            _ => Ok(CrossPointIO::All)
        }
    }
}