use std::io::prelude::*;
use std::env;
use std::process;
use std::fs;
use std::path::Path;
use std::net::TcpStream;

fn main() {
    const INPUT_FILEPATH: &str = "client/data/input.csv";
    const GET_REQ: &[u8] = b"GET / HTTP/1.1\r\n";

    let mut args = env::args();
    args.next();
    let addr = match args.next() {
        Some(v) => v,
        None => {
            eprintln!("failed to get server address argument");
            process::exit(1);
        },
    };

    let mut inpath = env::current_dir().unwrap();
    inpath.push(Path::new(INPUT_FILEPATH));
    let input = match fs::read_to_string(inpath.as_path()) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("failed to open {:?}: {}", inpath, v);
            process::exit(1);
        },
    };

    let mut conn = match TcpStream::connect(addr) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("connection failure: {}", v);
            process::exit(1);
        },
    };
    for line in input.lines() {
        println!("{}", line);
        conn.write(GET_REQ).unwrap();
        conn.flush().unwrap();
        let mut buf = [0; 1024];
        conn.read(&mut buf).unwrap();
        println!("RESP: {}", String::from_utf8_lossy(&buf));
    }
}