use openssl::ssl::{HandshakeError, SslConnector, SslMethod, SslStream, SslVerifyMode};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::result::Result;

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

    pub fn start_server() {
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

    /// Like create_tcp_stream but verifies the cert and won't connect
    /// because the server uses self signed certs
    pub fn create_tcp_stream_secure() -> Result<SslStream<TcpStream>, HandshakeError<TcpStream>> {
        let connector = SslConnector::builder(SslMethod::tls()).unwrap();
        let connector = connector.build();
        let stream = TcpStream::connect("localhost:8443").unwrap();
        return connector.connect("localhost", stream);
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;

    #[test]
    fn http_long_message() {
        let mut server = TestServer::new();
        let big_buff: [u8; 8192] = ['A' as u8; 8192];
        let result = server.first_response_line(&big_buff);
        assert_eq!(result, "HTTP/1.1 413 PAYLOAD TOO LARGE");
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
    fn post_request_with_body() {
        let mut server = TestServer::new();
        let result = server.get_all(b"POST / HTTP/1.0\r\n\r\nHere is some data for you!");
        let first_line = result.lines().next().unwrap();
        assert_eq!(first_line, "HTTP/1.1 405 Method Not Allowed");
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
    fn connection_timeout() {
        let mut server = TestServer::new();
        // The time needs to be higher than in test_data/unit_test_config.json
        let sleep_time = time::Duration::from_secs_f32(6.0);
        server.write(b"GET / ");
        thread::sleep(sleep_time);
        server.write(b"HTTP/1.0\r\n\r\n");

        let resp = server.get_response();
        assert_eq!(resp, "HTTP/1.1 408 REQUEST TIMEOUT\r\n\r\n");
    }

    #[test]
    fn invalid_http_timeout() {
        let mut server = TestServer::new();
        server.write(b"A");

        let resp = server.get_response();
        assert_eq!(resp, "HTTP/1.1 408 REQUEST TIMEOUT\r\n\r\n");
    }

    #[test]
    fn connection_timeout_success() {
        let mut server = TestServer::new();
        // Config needs to be atleast 5 seconds
        let sleep_time = time::Duration::from_secs_f32(4.5);
        let msg = format!("GET {} HTTP/1.0", DASH_DOCUMENT);
        server.write(msg.as_bytes());
        thread::sleep(sleep_time);
        server.write(b"\r\n\r\n");

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
        server.write(b" HTTP/1.0");
        server.write(b"\r\n\r\n");

        let resp = server.get_response();
        assert!(resp.len() > 0);
        dash_document_succes(resp);
    }

    #[test]
    fn multi_part_msg_slow() {
        let mut server = TestServer::new();
        let sleep_time = time::Duration::from_secs_f32(0.2);
        server.write(b"GET ");
        thread::sleep(sleep_time);
        server.write(DASH_DOCUMENT.as_bytes());
        thread::sleep(sleep_time);
        server.write(b" HTTP/1.0");
        thread::sleep(sleep_time);
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

    #[test]
    fn invalid_cert_no_crash() {
        TestServer::start_server();
        for _ in 0..3 {
            let server = TestServer::create_tcp_stream_secure();
            // Connection always fails due the handshake error
            // because we use self signed certs during tests
            assert!(server.is_err());
        }
    }
}
