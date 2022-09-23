use std::{io::{Write, BufReader, BufRead, Error}, time::Duration};
use serialport::SerialPort;

const INFO_CMD: [u8;3] = [ 0x49, 0x0D, 0x0A ]; // "I\r\n"

pub struct CrossPoint {
    port: Box<dyn SerialPort>,

    serial_port: String,
    input_count: i32,
    output_count: i32,
    audio_support: bool,
}

impl CrossPoint {
    pub fn connect(port_name: &str) -> Result<CrossPoint, Error> {

        let mut port = serialport::new(port_name, 9600)
            .data_bits(serialport::DataBits::Eight)
            .flow_control(serialport::FlowControl::None)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(Duration::from_secs(1))
            .open()?;

        let serial_port = String::from(port_name);

        port.write(&INFO_CMD)?;
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
        let cmd = [ 0x1B, ((preset_number / 10) + 0x30) as u8, ((preset_number % 10) + 0x30) as u8, 0x4E, 0x47, 0x0D ]; // "[Esc]NGxx[CR]"
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
        let mut cmd: Vec<u8> = Vec::new();
        if preset_number > 10 {
            cmd.push(((preset_number / 10) + 0x30) as u8);
        }
        cmd.push(((preset_number % 10) + 0x30) as u8);
        cmd.push(',' as u8);
        cmd.push(0x0D);
        cmd.push(0x0A);

        self.send_command(&cmd)
    }

    pub fn load_preset(&mut self, preset_number: i32) -> Result<String, Error> {
        let mut cmd: Vec<u8> = Vec::new();
        if preset_number > 10 {
            cmd.push(((preset_number / 10) + 0x30) as u8);
        }
        cmd.push(((preset_number % 10) + 0x30) as u8);
        cmd.push('.' as u8);
        cmd.push(0x0D);
        cmd.push(0x0A);

        self.send_command(&cmd)
    }

    fn io_char(io_type: CrossPointIO) -> u8 {
        (match io_type {
            CrossPointIO::All => '!',
            CrossPointIO::RGB => '&',
            CrossPointIO::Vid => '%',
            CrossPointIO::Aud => '$'
        }) as u8
    }

    pub fn create_preset(&mut self, new_preset: CrossPointPreset) -> Result<String, Error>
    {
        let io = CrossPoint::io_char(new_preset.io_type);
        //Clear preset first
        let mut cmd: Vec<u8> = Vec::new();
        cmd.push(0x1B); //Esc
        cmd.push('+' as u8);
        if new_preset.number > 10 {
            cmd.push(((new_preset.number / 10) + 0x30) as u8);
        }
        cmd.push(((new_preset.number % 10) + 0x30) as u8);
        cmd.push('P' as u8);
        cmd.push('0' as u8);
        cmd.push('*' as u8);
        cmd.push('!' as u8);
        cmd.push('\r' as u8);

        self.send_command(&cmd)?;

        cmd.clear();
        cmd.push(0x1B as u8); //Esc
        cmd.push('+' as u8);
        if new_preset.number > 10 {
            cmd.push(((new_preset.number / 10) + 0x30) as u8);
        }
        cmd.push(((new_preset.number % 10) + 0x30) as u8);
        cmd.push('P' as u8);
        for tie in new_preset.ties
        {
            let in_channel = tie.input_channel;
            for out_channel in tie.output_channels {
                if in_channel > 10 {
                    cmd.push(((in_channel / 10) + 0x30) as u8);
                }
                cmd.push(((in_channel % 10) + 0x30) as u8);
                cmd.push('*' as u8);
                if out_channel > 10 {
                    cmd.push(((out_channel / 10) + 0x30) as u8);
                }
                cmd.push(((out_channel % 10) + 0x30) as u8);
                cmd.push(io);
            }
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
                input_channel: i.1["InputChannel"].as_i32().unwrap(),
                output_channels: i.1["OutputChannels"].entries().map(|channel| channel.1.as_i32().unwrap()).collect()
            }
        }).collect();

        CrossPointPreset { number, name, ties, io_type: CrossPointIO::All }
    }
}

pub struct CrossPointPreset
{
    pub number: i32,
    pub name: String,
    pub ties: Vec<CrossPointTie>,
    pub io_type: CrossPointIO
}

pub struct  CrossPointTie
{
    pub input_channel: i32,
    pub output_channels: Vec<i32>
}

pub enum CrossPointIO
{
    All,
    RGB,
    Vid,
    Aud,
}