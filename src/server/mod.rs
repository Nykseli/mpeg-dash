use openssl::ssl;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use std::fs;
use std::io::{Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use crate::config;
use mpeg_dash::ThreadPool;

const MAX_REQUEST_SIZE: usize = 4096;

/// Is the last 4 bytes the end of the http header
/// TODO: may not be usable if support for POST requests are added
fn is_end_of_header(buffer: &[u8]) -> bool {
    if buffer.len() < 4 {
        return false;
    }

    // HTTP standard defines http header end as "\r\n\r\n"
    let end = &['\r' as u8, '\n' as u8, '\r' as u8, '\n' as u8];
    let mut temp_buf = &buffer[..];
    while !temp_buf.is_empty() {
        if temp_buf.ends_with(end) {
            return true;
        }
        temp_buf = &temp_buf[..temp_buf.len() - 1];
    }

    false
    // This could be used if we could make sure that the buffer doesn't contain any body data
    // The method above is actually only really slower IF there is body data
    // buffer[buffer.len() - 4..buffer.len()] == end
}

/// Check if the error happend in I/O (false) or in ssl/tsl stack (true)
fn is_ssl_error(error: ssl::Error) -> bool {
    // Result returns ssl::Error as Result Err and io::Error as Ok
    error.into_io_error().is_err()
}

/// 404 File not found
fn response_404(mut stream: SslStream<TcpStream>) {
    stream
        .write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes())
        .unwrap();
}

/// 408 Request Timeout
fn response_408(mut stream: SslStream<TcpStream>) {
    stream
        .write("HTTP/1.1 408 REQUEST TIMEOUT\r\n\r\n".as_bytes())
        .unwrap();
}

/// 413 Payload Too Large
fn response_413(mut stream: SslStream<TcpStream>) {
    stream
        .write("HTTP/1.1 413 PAYLOAD TOO LARGE\r\n\r\n".as_bytes())
        .unwrap();
}

fn handle_client(mut stream: SslStream<TcpStream>) {
    let config = config::GlobalConfig::config();

    // SslStream doesn't have a timeout so we need to set it to the underlying TcpStream
    stream
        .get_ref()
        .set_read_timeout(Some(Duration::from_secs_f64(
            config.performance.connection_timeout,
        )))
        .unwrap();

    // TODO: is there more optimal way of reading?
    let mut buf = vec![];
    loop {
        // TODO: why this doesn't work with vec![]?
        //       with ./test_client.py this recieves data_len == 0 with vec![]
        //let mut buf2 = vec![];
        let mut temp_buf = [0 as u8; MAX_REQUEST_SIZE];
        match stream.ssl_read(&mut temp_buf) {
            Ok(data_len) => {
                buf.extend_from_slice(&temp_buf[..data_len]);

                if data_len == 0 {
                    // Not completely sure if this even ever happens
                    break;
                } else if is_end_of_header(&buf[..]) {
                    break;
                } else if buf.len() >= MAX_REQUEST_SIZE {
                    response_413(stream);
                    return;
                }
            }
            Err(error) => {
                // If ssl_error happens, the connection is not usable so we
                // can just ignore it but we can still handle the io errors
                // TODO: figure out how to test the self signed cert error
                // TODO: log ssl errors
                if !is_ssl_error(error) {
                    // TODO: what other errors there might be?
                    response_408(stream);
                }
                return;
            }
        }
    }

    // TODO: is lossy a good (fast) option?
    let request_full = String::from_utf8_lossy(&buf);

    // TODO: check all the lines
    // TODO: handle ERr
    let first_line = request_full.lines().next().unwrap();
    let mut request_parts = first_line.split_whitespace();

    // Only gets are currenlty supported
    if request_parts.next().unwrap() != "GET" {
        stream
            .write("HTTP/1.1 405 Method Not Allowed\r\n\r\n".as_bytes())
            .unwrap();
        return;
    }

    let path = request_parts.next().unwrap();
    // Currently the root path doesn't contain anything
    if path.len() <= 1 {
        response_404(stream);
        return;
    }

    let relative_path = &path[1..path.len()];
    let file_data = match fs::read(relative_path) {
        Ok(data) => data,
        Err(_) => {
            response_404(stream);
            return;
        }
    };

    let file_type = if relative_path.ends_with(".mpd") {
        "application/dash+xml"
    } else {
        "application/octet-stream"
    };

    // TODO: handle Err
    // TODO: should all the responses contain information about the server? version number etc?
    let access_origin = &config.network.allow_origin[..];
    let out = format!("HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: {}\r\nContent-type: {}\r\nContent-Length: {}\r\n\r\n", access_origin, file_type, file_data.len());
    stream.write(out.as_bytes()).unwrap();
    stream.write_all(&file_data[..]).unwrap();
    stream.flush().unwrap();
    // TODO: this should happen on every error.
    //       create struct out of the stream that implements drop
    // TODO:: actully do we even need this because of write_all?
    //stream.shutdown().unwrap();
}

pub struct DashServer {
    acceptor: Arc<SslAcceptor>,
    listener: std::net::TcpListener,
    thread_pool: ThreadPool,
}

impl DashServer {
    pub fn new() -> DashServer {
        let config = config::GlobalConfig::config();

        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();

        // TODO: pass down the error
        acceptor
            .set_private_key_file(&config.security.private_key_file[..], SslFiletype::PEM)
            .unwrap();
        acceptor
            .set_certificate_file(&config.security.certificate_file[..], SslFiletype::PEM)
            .unwrap();
        acceptor.check_private_key().unwrap();
        let acceptor = Arc::new(acceptor.build());

        let address = format!("{}:{}", config.network.address, config.network.port);
        let listener = TcpListener::bind(address).unwrap();
        // TODO: would we benefit from M:N model?
        let pool = ThreadPool::new(config.performance.thread_pool_size);

        DashServer {
            acceptor: acceptor,
            listener: listener,
            thread_pool: pool,
        }
    }

    // TODO: support for regular http
    pub fn start_server(&self) {
        for stream in self.listener.incoming() {
            match stream {
                Ok(stream) => {
                    let acceptor = self.acceptor.clone();
                    self.thread_pool.execute(move || {
                        // Ignore streams with tls handshake errors
                        if let Ok(stream) = acceptor.accept(stream) {
                            handle_client(stream);
                        }
                    });
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    }

    /// Graefully stop the server
    #[allow(dead_code)]
    pub fn stop_server(&self) {
        drop(&self.listener);
        drop(&self.thread_pool);
    }
}
