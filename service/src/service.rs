use std::{
    fs::File,
    path::{Path,PathBuf},
    net::{TcpListener, TcpStream, SocketAddrV4},
    io::{Read, Write},
    str::FromStr
};
use json::{
    object::Object,
    Array, JsonValue,
    number::Number
};
use crate::{CrossPoint, crosspoint::CrossPointPreset, ServiceConfiguration};
use crate::http::{HttpRequest,HttpResponse};

const DEFAULT_BINDING: &str = "127.0.0.1:1872";
const DEFAULT_HTTP_ROOT: &str = "../site/";

pub fn start(config: ServiceConfiguration) {

    std::thread::spawn(move || {

        if config.serial_port.is_none() {
            panic!("Serial port required")
        }

        let serial_port = config.serial_port.unwrap();
        let mut port = CrossPoint::connect(&serial_port)
            .expect(&(String::from("Failed to connect to CrossPoint on port ") + &serial_port));
        println!("Connected to CrossPoint on port {}", port.port_name());

        let binding = match config.binding {
            Some(b) => b,
            None => SocketAddrV4::from_str(DEFAULT_BINDING).unwrap()
        };

        let listener = TcpListener::bind(binding)
            .expect("Failed to bind");

        println!("Listening on: {}", binding);

        let http_root = config.http_root.unwrap_or(String::from(DEFAULT_HTTP_ROOT));
        for stream_result in listener.incoming() {
            let stream = match stream_result { Ok(s) => s, Err(_) => continue };
            println!("Request from {}", match stream.peer_addr() { Ok(ip) => ip, Err(_) => continue });
            handle_request(stream, &mut port, &http_root);
        }
    });
}

fn handle_request(mut stream: TcpStream, port: &mut CrossPoint, http_root: &str) {
    let mut buffer = [0 as u8; 512];

    let length = stream.read(&mut buffer).expect("Error reading request data");
    if length > 0 {
        let str = String::from_utf8(buffer[..length].to_vec()).unwrap();
        let request = HttpRequest::parse(&str).unwrap();

        let (content, mime, status_code) = match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => { get_page("index.html", http_root) }
            ("GET", "/activePresets") => { get_presets_names(port, true) }
            ("GET", "/saveCurrentToPreset") => { save_current_config(port, request.query_params.get("preset")) }
            ("GET", "/loadPreset") => { load_preset(port, request.query_params.get("preset")) }
            ("POST", "/createPreset") => { create_preset(port, &request.content.unwrap()) }
            ("GET", "/presetNames") => { get_presets_names(port, false) }
            ("GET", _) => { get_page(&request.path, http_root) }
            (_, _) => { (String::new(), String::new(), 404) }
        };

        let response = HttpResponse {
            content,
            status_code,
            mime
        };
        _ = stream.write_all(response.compose().as_bytes());
    }
}

fn get_page(relative_path: &str, http_root: &str) -> (String, String, i32) {
    let mut fullpath = PathBuf::new();
    fullpath.push(http_root);
    fullpath.push(relative_path.trim_start_matches('/'));

    let path = Path::new(relative_path);
    let extension = path.extension().unwrap();

    let mut content = String::new();
    let mut page_file = match File::open(fullpath) {
        Ok(f) => f,
        Err(_) => return (content, String::new(), 404)
    };

    match page_file.read_to_string(&mut content) {
        Ok(f) => f,
        Err(_) => return (content, String::new(), 500)
    };

    let mime = String::from(match extension.to_str().unwrap() {
        "js" => "text/javascript",
        "css" => "text/css",
        "html" => "text/html",
        "xml" => "text/xml",
        _ => "text/plain"
    });

    (content, mime, 200)
}

fn get_presets_names(port: &mut CrossPoint, only_active: bool) -> (String, String, i32)  {
    let mut presets = Array::new();
    for i in 1..=32 {
        let mut name = port.get_preset_name(i).unwrap();
        name = name.trim().to_string();
        if !only_active || name != "[unassigned]" {
            let mut o = Object::new();
            o.insert("Number", JsonValue::Number(Number::from(i)));
            o.insert("Name", JsonValue::String(name));
            presets.push(JsonValue::Object(o));
        }
    }
    let mut response = Object::new();
    response.insert("Presets", JsonValue::Array(presets));
    (json::stringify(response), String::from("application/json"), 200)
}


fn save_current_config(port: &mut CrossPoint, preset_number_param: Option<&String>) -> (String, String ,i32) {
    if preset_number_param.is_some() {
        let preset_number: i32 = preset_number_param.unwrap().parse().unwrap_or_default();
        if preset_number > 0 && preset_number <= 32 {
            return (port.save_current_config(preset_number).unwrap(), String::new(), 200)
        }
    }

    (String::new(), String::new(), 400)
}

fn load_preset(port: &mut CrossPoint, preset_number_param: Option<&String>) -> (String, String, i32) {
    if preset_number_param.is_some() {
        let preset_number: i32 = preset_number_param.unwrap().parse().unwrap_or_default();
        if preset_number > 0 && preset_number <= 32 {
            return (port.load_preset(preset_number).unwrap(), String::new(), 200)
        }
    }

    (String::new(), String::new(), 400)
}

fn create_preset(port: &mut CrossPoint, content: &str) -> (String, String, i32) {
    let json = json::parse(content);
    if json.is_ok() {
        let preset = CrossPointPreset::from(json.unwrap());

        return (port.create_preset(preset).unwrap(), String::new(), 200)
    }

    //Error response
    (String::new(), String::new(), 400)
}