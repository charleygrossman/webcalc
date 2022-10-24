use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::process;
use server::{Job, WorkerPool, WORKER_POOL_MAX_SIZE};

fn main() {
    const ADDR: &str = "127.0.0.1:8000";

    let pool = WorkerPool::new(WORKER_POOL_MAX_SIZE).unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    });

    let listener = TcpListener::bind(ADDR).unwrap_or_else(|err| {
        eprintln!("failed to serve on {}: {}", ADDR, err);
        process::exit(1);
    });
    for stream in listener.incoming() {
        let stream = stream.unwrap_or_else(|err| {
            eprintln!("connection failure: {}", err);
            process::exit(1);
        });
        let job = Job::new(Box::new(|| { handle_connection(stream)}));
        if let Err(e) = pool.execute(job) {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn handle_connection(mut stream: TcpStream) {
    const GET_REQ: &[u8] = b"GET / HTTP/1.1\r\n";
    const OK_RESP: &str = "<!DOCTYPE html><html lang=\"en\"><head>
        <meta charset=\"utf-8\"><title>ok</title></head><body>
        <h1>ok</h1></body></html>";
    const NOT_FOUND_RESP: &str = "<!DOCTYPE html><html lang=\"en\"><head>
        <meta charset=\"utf-8\"><title>not found</title></head><body>
        <h1>not found</h1></body></html>";

    let resp: String;
    let mut buf = [0; 1024];
    stream.read(&mut buf).unwrap();
    if buf.starts_with(GET_REQ) {
        resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            OK_RESP.len(), OK_RESP,
        ); 
    } else {
        resp = format!(
            "HTTP/1.1 404 NOT FOUND\r\nContent-Length: {}\r\n\r\n{}",
            NOT_FOUND_RESP.len(), NOT_FOUND_RESP,
        );
    }
    stream.write(resp.as_bytes()).unwrap();
    stream.flush().unwrap();
}
