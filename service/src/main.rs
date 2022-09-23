pub mod crosspoint;
pub mod service;
pub mod config;
pub mod http;

use std::{
    io::stdin,
    error::Error,
};
use crosspoint::CrossPoint;
use config::ServiceConfiguration;

fn main() -> Result<(), Box<dyn Error>> {

    let config = ServiceConfiguration::load()?;

    service::start(config);

    println!("Press enter to exit");
    _ = stdin().read_line(&mut String::new());

    Ok(())
}

/*
fn select_port() -> Result<String, Box<dyn Error>> {
    let ports = serialport::available_ports()?;
    if ports.len() == 0 { return Err(Box::new(IoError::from(std::io::ErrorKind::NotFound))); }

    let mut port_num: usize;
    let mut input_buffer = String::new();

    loop {
        //[2J
        print!("{}", 27 as char);
        println!("Available ports:");

        let mut index = 1;
        for port_info in &ports {
            println!("{}) {}", index, port_info.port_name);
            index += 1;
        }
    
        println!("Select port: ");
        stdin().read_line(&mut input_buffer)?;
        port_num = input_buffer.trim().parse()?;

        if port_num > 0 { break; } 
    }

    Ok(ports[port_num - 1].port_name.clone())
}
*/