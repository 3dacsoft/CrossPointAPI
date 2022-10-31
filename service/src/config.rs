use std::{
    fmt::{Display,Debug},
    net::{SocketAddrV4},
    collections::HashMap,
    fs, str::FromStr,
};

pub struct ServiceConfiguration {
    pub serial_port: Option<String>,
    pub http_root: Option<String>,
    pub binding: Option<SocketAddrV4>,
    pub inputs: HashMap<i32, String>,
    pub outputs: HashMap<i32, String>
}

impl ServiceConfiguration {
    pub fn load() -> Result<ServiceConfiguration, ConfigurationError> {
        let config_file = match fs::read_to_string("service.json") {
            Ok(f) => f,
            Err(e) => return Err(ConfigurationError::new(&format!("Error opening config file. {}.", e)))
        };
        let config_json = match json::parse(&config_file) {
            Ok(j) => j,
            Err(_) => return Err(ConfigurationError::new("Config file is not in JSON format"))
        };

        let mut config = ServiceConfiguration {
            binding: None,
            http_root: None,
            serial_port: None,
            inputs: HashMap::new(),
            outputs: HashMap::new()
        };
        
        let serial_port = config_json["serial-port"].as_str();
        if serial_port.is_some() { config.serial_port = Some(serial_port.unwrap().to_string()); }

        let http_root = config_json["http_root"].as_str();
        if http_root.is_some() { config.http_root = Some(http_root.unwrap().to_string()); }
        
        let binding = SocketAddrV4::from_str(config_json["binding"].as_str().unwrap_or_default());
        if binding.is_ok() { config.binding = Some(binding.unwrap()); }

        let input_array = config_json["inputs"].to_owned();
        if input_array.is_array() {
            for input in input_array.members() {
                let name = input["description"].as_str();
                let number = input["channel"].as_i32();
                if name.is_some() && number.is_some() {
                    config.inputs.insert(number.unwrap(), String::from(name.unwrap()));
                }
            }
        }
    
        let output_array = config_json["outputs"].to_owned();
        if output_array.is_array() {
            for output in output_array.members() {
                let name = output["description"].as_str();
                let number = output["channel"].as_i32();
                if name.is_some() && number.is_some() {
                    config.outputs.insert(number.unwrap(), String::from(name.unwrap()));
                }
            }
        }

        Ok(config)
    }
}

pub struct ConfigurationError {
    description: String
}

impl std::error::Error for ConfigurationError { }

impl ConfigurationError {
    pub fn new(desc: &str) -> ConfigurationError {
        ConfigurationError { description: String::from(desc) }
    }
}

impl Debug for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl Display for ConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
