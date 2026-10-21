//! A tiny HTTP/1.1 server for tests of code that talks to web APIs. Each
//! request is logged and answered by a handler; connections are kept
//! alive like a real server's.
//!
//! ```
//! use material_3_ui::testing::MockServer;
//! let server = MockServer::start(|req| match req.path.as_str() {
//!     "/token" => (200u16, r#"{"access_token":"at"}"#),
//!     _ => (404, ""),
//! });
//! // point the code under test at server.url, then check server.requests()
//! ```

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    /// The path with its query string.
    pub path: String,
    /// Lower-case names.
    pub headers: HashMap<String, String>,
    pub body: String,
}

impl Request {
    /// A field of an `application/x-www-form-urlencoded` body.
    pub fn form(&self, key: &str) -> Option<String> {
        decode_pairs(&self.body).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// A query-string parameter.
    pub fn query(&self, key: &str) -> Option<String> {
        let query = self.path.split_once('?')?.1;
        decode_pairs(query).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

fn decode_pairs(s: &str) -> Vec<(String, String)> {
    s.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                match std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What the handler answers.
pub struct Reply {
    pub status: u16,
    pub body: String,
    pub headers: Vec<(String, String)>,
}

impl Reply {
    pub fn new(status: u16, body: impl Into<String>) -> Self {
        Reply { status, body: body.into(), headers: Vec::new() }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl From<(u16, String)> for Reply {
    fn from((status, body): (u16, String)) -> Self {
        Reply::new(status, body)
    }
}

impl From<(u16, &str)> for Reply {
    fn from((status, body): (u16, &str)) -> Self {
        Reply::new(status, body)
    }
}

type Handler = Arc<dyn Fn(&Request) -> Reply + Send + Sync>;

/// The server; it lives until the test process ends.
pub struct MockServer {
    /// `http://127.0.0.1:<port>`, no trailing slash.
    pub url: String,
    pub requests: Arc<Mutex<Vec<Request>>>,
}

impl MockServer {
    pub fn start<R: Into<Reply>>(handler: impl Fn(&Request) -> R + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
        let url = format!("http://{}", listener.local_addr().expect("local address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        let handler: Handler = Arc::new(move |r: &Request| handler(r).into());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let log = log.clone();
                let handler = handler.clone();
                std::thread::spawn(move || serve(stream, &log, &handler));
            }
        });
        MockServer { url, requests }
    }

    /// Every request so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().map(|r| r.clone()).unwrap_or_default()
    }
}

fn serve(stream: TcpStream, log: &Mutex<Vec<Request>>, handler: &Handler) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut out = stream;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();
        let mut headers = HashMap::new();
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).unwrap_or(0) == 0 {
                return;
            }
            let h = h.trim_end();
            if h.is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }
        let len: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        let req = Request { method, path, headers, body: String::from_utf8_lossy(&body).into_owned() };
        if let Ok(mut l) = log.lock() {
            l.push(req.clone());
        }
        let reply = handler(&req);
        let mut head = format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\n", reply.status, reply.body.len());
        for (k, v) in &reply.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str("\r\n");
        if out.write_all(head.as_bytes()).and_then(|_| out.write_all(reply.body.as_bytes())).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_and_logs_requests() {
        let server = MockServer::start(|req| {
            if req.method == "POST" {
                Reply::new(201, "made").header("ETag", "\"1\"")
            } else {
                Reply::new(200, req.query("q").unwrap_or_default())
            }
        });
        let addr = server.url.trim_start_matches("http://").to_string();
        let mut s = TcpStream::connect(&addr).unwrap();
        // Two requests on one keep-alive connection.
        s.write_all(b"GET /search?q=a%20b+c HTTP/1.1\r\nHost: x\r\n\r\nPOST /new HTTP/1.1\r\nContent-Length: 7\r\n\r\nk=v%26w")
            .unwrap();
        s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut text = String::new();
        let mut buf = [0u8; 1024];
        while !text.ends_with("made") {
            let n = s.read(&mut buf).unwrap();
            assert!(n > 0, "closed early: {text}");
            text.push_str(&String::from_utf8_lossy(&buf[..n]));
        }
        assert!(text.starts_with("HTTP/1.1 200"));
        assert!(text.contains("\r\n\r\na b cHTTP/1.1 201"), "{text}");
        assert!(text.contains("ETag: \"1\""));
        let reqs = server.requests();
        assert_eq!(reqs.len(), 2);
        assert_eq!(reqs[0].path, "/search?q=a%20b+c");
        assert_eq!(reqs[1].form("k").as_deref(), Some("v&w"));
        assert_eq!(reqs[1].form("missing"), None);
        assert_eq!(percent_decode("%zz%4"), "%zz%4");
    }
}
