use openssl::ssl::{SslConnector, SslMethod, SslStream, SslVerifyMode};
use std::io::{Read, Write};
use std::net::TcpStream;

use std::{thread, time};

#[cfg(test)]
#[path = "../src/config.rs"]
mod config;

#[cfg(test)]
#[path = "../src/server/mod.rs"]
mod server;

// This requres the tests to be run on a single thread
static mut IS_SERVER_INIT: bool = false;

const DASH_DOCUMENT: &str = "/test_data/unit_test_dash_document.mpd";

struct TestServer {
    connector: SslStream<TcpStream>,
}

impl TestServer {
    fn new() -> TestServer {
        TestServer::start_server();
        let connector = TestServer::create_tcp_stream();
        TestServer { connector }
    }

    pub fn write(&mut self, buf: &[u8]) {
        self.connector.write(buf).unwrap();
    }

    /// Buf is data sent to the server
    pub fn write_all(&mut self, buf: &[u8]) {
        self.connector.write_all(buf).unwrap();
    }

    pub fn get_response(&mut self) -> String {
        let mut res = vec![];
        self.connector.read_to_end(&mut res).unwrap();
        String::from_utf8_lossy(&res).as_ref().to_owned()
    }

    /// Buf is data sent to the server
    pub fn get_all(&mut self, buf: &[u8]) -> String {
        self.write_all(buf);
        self.get_response()
    }

    /// Buf is data sent to the server
    /// Get the first line of the response
    pub fn first_response_line(&mut self, buf: &[u8]) -> String {
        let all_data = self.get_all(buf);
        all_data.lines().next().unwrap().to_owned()
    }

    fn start_server() {
        unsafe {
            if IS_SERVER_INIT {
                return;
            }
            IS_SERVER_INIT = true;
        }

        let _ = config::GlobalConfig::init("test_data/unit_test_config.json");
        thread::spawn(|| {
            let server = server::DashServer::new();
            server.start_server();
        });

        let sleep_time = time::Duration::from_secs(1);
        thread::sleep(sleep_time);
    }

    fn create_tcp_stream() -> SslStream<TcpStream> {
        let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
        // Accept all certs. We are testing the tcp socket, not the tls security
        connector.set_verify_callback(SslVerifyMode::NONE, |_, _| true);
        let connector = connector.build();
        let stream = TcpStream::connect("localhost:8443").unwrap();
        return connector.connect("localhost", stream).unwrap();
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;

    /// Named aaa_ so it's run first to set to buffer.
    /// Other tests may fail because of this because the socket buffer is full off carbage
    #[test]
    fn aaa_http_long_message() {
        let mut server = TestServer::new();
        let big_buff: [u8; 8192] = ['A' as u8; 8192];
        let result = server.first_response_line(&big_buff);
        // TODO: should this be "HTTP/1.1 400 Bad Request"?
        assert_eq!(result, "HTTP/1.1 405 Method Not Allowed");
    }

    #[test]
    fn simple_http_connection() {
        let mut server = TestServer::new();
        let result = server.get_all(b"GET / HTTP/1.0\r\n\r\n");
        assert!(result.len() > 0);
    }

    #[test]
    fn http_root_404() {
        let mut server = TestServer::new();
        let result = server.get_all(b"GET / HTTP/1.0\r\n\r\n");
        let first_line = result.lines().next().unwrap();
        assert_eq!(first_line, "HTTP/1.1 404 NOT FOUND");
    }

    #[test]
    fn http_only_allow_get_method() {
        // Methods are from https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods
        let m_list = [
            "HEAD", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "TRACE", "PATCH",
        ];

        for m in &m_list {
            // Server client can only handle one request
            let mut server = TestServer::new();
            let request = format!("{} / HTTP/1.0\r\n\r\n", m);
            let resp = server.first_response_line(request.as_bytes());
            assert_eq!(resp, "HTTP/1.1 405 Method Not Allowed");
        }
    }

    #[test]
    #[should_panic]
    fn connection_timeout() {
        let mut server = TestServer::new();
        for _ in 1..=100 {
            let sleep_time = time::Duration::from_secs_f32(0.01);
            thread::sleep(sleep_time);
            server.write(b"A");
        }

        // Needs to panic befor this
        let resp = server.get_response();
        assert!(resp.len() > 0);
    }

    // Helper function to parsing response when requesting DASH_DOCUMENT
    fn dash_document_succes(resp: String) {
        let mut lines = resp.lines();
        let first_line = lines.next().unwrap();
        assert_eq!(first_line, "HTTP/1.1 200 OK");

        let mut content_len: i32 = -1;
        let mut access_control = "";
        let mut content_type = "";
        while let Some(line) = lines.next() {
            if line.starts_with("Content-Length:") {
                let tup: Vec<&str> = line.split_ascii_whitespace().collect();
                content_len = tup[1].parse::<i32>().unwrap();
            } else if line.starts_with("Access-Control-Allow-Origin") {
                let tup: Vec<&str> = line.split_ascii_whitespace().collect();
                access_control = tup[1];
            } else if line.starts_with("Content-type") {
                let tup: Vec<&str> = line.split_ascii_whitespace().collect();
                content_type = tup[1];
            }
        }

        assert_eq!(content_len, 1280);
        assert_eq!(access_control, "*");
        assert_eq!(content_type, "application/dash+xml");
    }

    #[test]
    fn multi_part_msg() {
        let mut server = TestServer::new();
        server.write(b"GET ");
        server.write(DASH_DOCUMENT.as_bytes());
        server.write(b"HTTP/1.0");
        server.write(b"\r\n\r\n");

        let resp = server.get_response();
        assert!(resp.len() > 0);
        dash_document_succes(resp);
    }

    #[test]
    fn successfull_document() {
        let mut server = TestServer::new();
        let msg = format!("GET {} HTTP/1.0\r\n\r\n", DASH_DOCUMENT);
        server.write_all(msg.as_bytes());

        let resp = server.get_response();
        assert!(resp.len() > 0);
        dash_document_succes(resp);
    }
}
