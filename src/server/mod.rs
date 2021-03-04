use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

use crate::config;
use mpeg_dash::ThreadPool;

fn response_404(mut stream: SslStream<TcpStream>) {
    stream
        .write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes())
        .unwrap();
}

fn handle_client(mut stream: SslStream<TcpStream>) {
    let config = config::GlobalConfig::config();

    // TODO: dynamic size
    let mut buf = [0 as u8; 1024];
    // TODO: handle Err
    stream.read(&mut buf).unwrap();

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
    // let out = format!("HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-type: {}\r\nContent-Length: {}\r\n\r\n", file_type, file_data.len());
    let access_origin = &config.network.allow_origin[..];
    let out = format!("HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: {}\r\nContent-type: {}\r\nContent-Length: {}\r\n\r\n", access_origin, file_type, file_data.len());
    stream.write(out.as_bytes()).unwrap();
    stream.write_all(&file_data[..]).unwrap();
    stream.flush().unwrap();
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
                        let stream = acceptor.accept(stream).unwrap();
                        handle_client(stream);
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
