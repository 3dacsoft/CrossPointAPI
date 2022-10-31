use std::{
    fs::File,
    path::{Path,PathBuf},
    net::SocketAddrV4,
    io::Read,
    str::FromStr
};
use json::{Array, JsonValue,object::Object,number::Number};
use crate::crosspoint::{CrossPoint,CrossPointPreset};
use crate::config::{ServiceConfiguration};
use crate::http::{HttpContext,HttpListener,HttpMethod,HttpResponseCode,HttpError,HttpResponse};

const DEFAULT_BINDING: &str = "127.0.0.1:1872";
const DEFAULT_HTTP_ROOT: &str = "../site/";
const TEXT_PLAIN: &str = "text/plain";
const APPLICATION_JSON: &str = "application/json";


pub fn start(config: ServiceConfiguration) {
    std::thread::spawn(move || {

        if config.serial_port.is_none() {
            panic!("Serial port required")
        }

        let serial_port = config.serial_port.unwrap();
        let mut port = CrossPoint::connect(&serial_port)
            .expect(&(String::from("Failed to connect to CrossPoint on port ") + &serial_port));
        println!("Connected to CrossPoint on port {}", port.port_name());

        let binding = config.binding.unwrap_or(SocketAddrV4::from_str(DEFAULT_BINDING).unwrap());

        let listener = HttpListener::bind(binding)
            .expect("Failed to bind");
        println!("Listening on: {}", binding);

        let http_root = config.http_root.unwrap_or(String::from(DEFAULT_HTTP_ROOT));
        loop {
            let request = match listener.receive() {
                Ok(r) => r,
                Err(_) => continue
            };

            handle_request(request, &mut port, &http_root);
        }
    });
}

fn handle_request(mut context: HttpContext, cp: &mut CrossPoint, http_root: &str) {

    let body = match context.request.content.as_ref() {
        Some(c) => c,
        None => ""
    };
    let method = context.request.method;
    let path = context.request.path.as_str();

    let response = match (method, path) {
        //Look for ajax function
        (HttpMethod::GET, "/activePresets") => get_presets_names(cp, true),
        (HttpMethod::GET, "/saveCurrentToPreset") => save_current_config(cp, context.request.query_params.get("preset")),
        (HttpMethod::GET, "/loadPreset") => load_preset(cp, context.request.query_params.get("preset")),
        (HttpMethod::POST, "/createPreset") => create_preset(cp, &body),
        (HttpMethod::GET, "/presetNames") => get_presets_names(cp, false),
        (HttpMethod::GET, _) => match get_page(&path, http_root) {
            Ok(p) => Ok(HttpResponse {
                status_code: HttpResponseCode::new(200),
                content: Some(p.0),
                mime: Some(p.1)
            }),
            Err(e) => Err(e)
        },
        (_, _) => Err(HttpError::new(404, "Resource does not exist"))
    };

    context.send_response(match response {
        Ok(r) => r,
        Err(e) => HttpResponse {
            content: Some(e.message),
            mime: Some(String::from(TEXT_PLAIN)),
            status_code: e.code
        }
    });
}

fn get_page(relative_path: &str, http_root: &str) -> Result<(String, String), HttpError> {
    let relative_path = if relative_path == "/" { "index.html" } else { relative_path.trim_start_matches('/') };

    let mut fullpath = PathBuf::new();
    fullpath.push(http_root);
    fullpath.push(relative_path);

    let path = Path::new(relative_path);
    let extension = path.extension().unwrap_or_default().to_str().unwrap_or_default();

    let mut content = String::new();
    let mut page_file = match File::open(fullpath) {
        Ok(f) => f,
        Err(_) => return Err(HttpError::new(404, "File not found"))
    };

    match page_file.read_to_string(&mut content) {
        Ok(f) => f,
        Err(e) => return Err(HttpError::new(500, e.to_string().as_str()))
    };

    let mime = String::from(match extension {
        "js" => "text/javascript",
        "css" => "text/css",
        "html" => "text/html",
        "xml" => "text/xml",
        _ => TEXT_PLAIN
    });

    Ok((content, mime))
}

fn get_presets_names(cp: &mut CrossPoint, only_active: bool) -> Result<HttpResponse, HttpError> {
    let mut presets = Array::new();
    for i in 1..=32 {
        let name = match cp.get_preset_name(i) {
            Ok(n) => n,
            Err(e) => return Err(HttpError::new(500, e.to_string().as_str()))
        };
        let name = name.trim().to_string();
        if !only_active || name != "[unassigned]" {
            let mut o = Object::new();
            o.insert("Number", JsonValue::Number(Number::from(i)));
            o.insert("Name", JsonValue::String(name));
            presets.push(JsonValue::Object(o));
        }
    }
    let mut response = Object::new();
    response.insert("Presets", JsonValue::Array(presets));

    Ok(HttpResponse {
        content: Some(json::stringify(response)),
        mime: Some(String::from(APPLICATION_JSON)),
        status_code: HttpResponseCode { code: 200 }
    })
}

fn save_current_config(cp: &mut CrossPoint, preset_number_param: Option<&String>) -> Result<HttpResponse, HttpError> {
    let preset_number = match preset_number_param.unwrap_or(&String::new()).parse() {
        Ok(n) => n,
        Err(_) => return Err(HttpError::new(400, "Missing preset number argument"))
    };

    if preset_number < 1 && preset_number > 32 {
        return Err(HttpError::new(400, "Invalid preset number"));
    }

    match cp.save_current_config(preset_number) {
        Ok(_) => Ok(HttpResponse { status_code: HttpResponseCode::new(200), content: None, mime: None }),
        Err(_) => Err(HttpError::new(400, "Invalid preset number"))
    }
}

fn load_preset(port: &mut CrossPoint, preset_number_param: Option<&String>) -> Result<HttpResponse, HttpError> {
    let preset_number: i32 = match preset_number_param {
        Some(p) => match p.parse() {
            Ok(n) => n,
            Err(_) => 0
        }
        None => return Err(HttpError::new(400, "Missing preset number argument"))
    };

    if preset_number < 1 && preset_number > 32 {
        return Err(HttpError::new(400, "Invalid preset number"));
    }

    match port.load_preset(preset_number) {
        Ok(_) => return Ok(HttpResponse {
            content: None,
            mime: None,
            status_code: HttpResponseCode { code: 200 }
        }),
        Err(_) => return Err(HttpError::new(400, "Missing or invalid preset"))
    }
}

fn create_preset(port: &mut CrossPoint, content: &str) -> Result<HttpResponse, HttpError> {
    let json_obj = match json::parse(content) {
        Ok(j) => j,
        Err(_) => return Err(HttpError::new(400, "Unparseable content"))
     };
     
    let preset = CrossPointPreset::from(json_obj);
    match port.create_preset(preset) {
        Ok(_) => Ok(HttpResponse {
            content: None,
            mime: None,
            status_code: HttpResponseCode::new(200)
        }),
        Err(e) => Err(HttpError::new(500, &e.to_string()))
    }
}