use std::{
    collections::HashMap,
    net::{TcpListener, SocketAddrV4, TcpStream},
    str::FromStr,
    io::{Read,Write},
    fmt::Debug
};


pub struct HttpListener {
    pub listener: TcpListener
}

impl HttpListener {
    pub fn bind(address: SocketAddrV4) -> std::io::Result<HttpListener> {
        Ok(HttpListener {
            listener: TcpListener::bind(address)?
        })
    }

    pub fn receive(&self) -> Result<HttpContext, HttpError> {
        let mut stream = match self.listener.accept() {
            Ok(s) => {
                println!("Request from {}", s.1);
                s.0
            }
            Err(e) => {
                println!("Error receiving stream: {:?}", e);
                return Err(HttpError::new(500, "Bad stream"))
            }
        };

        Ok(HttpContext {
            request: HttpRequest::read(&mut stream)?,
            stream
        })
    }
}


pub struct HttpContext {
    pub request: HttpRequest,
    stream: TcpStream,
}

impl HttpContext {
    pub fn send_response(&mut self, response: HttpResponse) {
        _ = self.stream.write_all(&mut response.compose().as_bytes());
    }
}

pub struct HttpRequest {
    pub method: HttpMethod,
    pub path: String,
    pub http_version: String,
    pub headers: HashMap<String,String>,
    pub query_params: HashMap<String,String>,
    pub content: Option<String>,
}

impl HttpRequest {
    pub fn read(stream: &mut TcpStream) -> Result<HttpRequest, HttpError> {
        let mut buffer = [0 as u8; 512];
        let length = match stream.read(&mut buffer) {
            Ok(b) => b,
            Err(_) => return Err(HttpError::new(500, "Unable to read request stream"))
        };

        let request_data  = match String::from_utf8(buffer[..length].to_vec()) {
            Ok(s) => s,
            Err(_) => return Err(HttpError::new(400, "Request content unreadable"))
        };

        let mut lines = request_data.split('\n');
        let mut firstline = lines.next().unwrap().split(' ');
        let method = match firstline.next() {
            Some(s) => match HttpMethod::from_str(s) {
                Ok(m) => m,
                Err(_) => return Err(HttpError::new(400, "Invalid method"))
            }
            None => HttpMethod::GET
        };
        let mut url = firstline.next().unwrap().split('?');
        let path = url.next().unwrap().to_string();
        let query_string = url.next();
        let mut query_params:HashMap<String, String> = HashMap::new();
        if query_string.is_some() {
            for pair in query_string.unwrap().split('&') {
                if pair.contains('=') {
                    let mut pair_split = pair.split('=');
                    let key = pair_split.next().unwrap();
                    let value = pair_split.next();
                    if value.is_none() { continue; }
                    query_params.insert(key.trim().to_string(), value.unwrap().trim().to_string());
                }
            }
        }
        
        let http_version = firstline.next().unwrap().to_string();

        let mut headers: HashMap<String,String> = HashMap::new();
        loop {
            let line = match lines.next() {
                Some(l) => l,
                None => break
            };

            if line.contains(':') { 
                let mut pair = line.split(':');
                headers.insert(pair.next().unwrap().trim().to_string(), pair.next().unwrap().trim().to_string());
            } else {
                break;
            }
        }

        let content_length = match headers.get("Content-Length") {
            Some(h) => h.parse().unwrap(),
            None => 0
        };

        let content:Option<String> = if content_length > 0 { 
            Some(request_data.split_at(content_length).1.to_string())
        } else {
            None
        };
        
        Ok(HttpRequest { method, path, http_version, headers, query_params, content })
    }
}



pub struct HttpResponse {
    pub status_code: HttpResponseCode,
    pub content: Option<String>,
    pub mime: Option<String>
}

impl HttpResponse {
    pub fn compose(&self) -> String {
        let mut response = String::from("HTTP/1.1 ");
        response.push_str(&self.status_code.code.to_string());
        response.push_str(" ");
        response.push_str(&self.status_code.description());
        response.push_str("\nServer:CrossPointApi");
        if self.content.is_some() {
            let content = self.content.as_ref().unwrap();
            response.push_str("\nContent-Length: ");
            response.push_str(&content.bytes().len().to_string());
            response.push_str("\nContent-Type: ");
            response.push_str(match self.mime.as_ref() { Some(m) => &m, None => "" });
            response.push('\n');
            response.push('\n');
            response.push_str(&content);
        }

        response
    }
}



#[derive(PartialEq,Clone)]
pub enum HttpMethod {
    GET, POST, PUT, PATCH, DELETE
}

impl Copy for HttpMethod { }

impl FromStr for HttpMethod {
    type Err = HttpError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "GET" => Ok(HttpMethod::GET),
            "POST" => Ok(HttpMethod::POST),
            "PUT" => Ok(HttpMethod::PUT),
            "PATCH" => Ok(HttpMethod::PATCH),
            "DELETE" => Ok(HttpMethod::DELETE),
            _ => Err(HttpError::new(405, "Invalid method"))
        }
    }
}


pub struct HttpResponseCode {
    pub code: i32,
}

impl HttpResponseCode {
    pub fn new(code: i32) -> HttpResponseCode {
        HttpResponseCode { code }
    }

    pub fn description(&self) -> String {
        String::from(match self.code {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            _ => "Bad Request"
        })
    }
}


pub struct HttpError {
    pub code: HttpResponseCode,
    pub message: String
}

impl HttpError {
    pub fn new(code: i32, message: &str) -> HttpError {
        HttpError {
            code: HttpResponseCode::new(code),
            message: String::from(message)
        }
    }
}

impl Debug for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
