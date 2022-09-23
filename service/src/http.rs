use std::collections::HashMap;
use std::error::Error;

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub http_version: String,
    pub headers: HashMap<String,String>,
    pub query_params: HashMap<String,String>,
    pub content: Option<String>
}

impl HttpRequest {
    pub fn parse(request: &str) -> Result<HttpRequest, Box<dyn Error>> {
        let mut lines = request.split('\n');
        let mut firstline = lines.next().unwrap().split(' ');
        let method = match firstline.next() {
            Some(s) => s.to_string(),
            None => String::new()
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

        let content:Option<String> = if (method == "POST" || method == "PUT") && content_length > 0 { 
            Some(request.split_at(content_length).1.to_string())
        } else {
            None
        };
        
        Ok(HttpRequest {
            method,
            path,
            http_version,
            headers,
            query_params,
            content
        })
    }
}

pub struct HttpResponse {
    pub status_code: i32,
    pub content: String,
    pub mime: String
}

impl HttpResponse {
    pub fn compose(&self) -> String {
        format!("HTTP/1.1 {} {}\nServer:CrossPointApi\nContent-Length: {}\nContent-Type: {}\n\n{}",
            self.status_code,
            HttpResponse::get_status_message(self.status_code),
            self.content.len(),
            self.mime,
            self.content)
    }

    fn get_status_message(code: i32) -> String {
        String::from(match code {
            200 => "OK",
            400 => "Bad Request",
            404 => "Not Found",
            _ => "Bad Request"
        })
    }
}