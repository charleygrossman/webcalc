use std::env;
use std::process;
use client::{Client, ClientConfig};

fn main() {
    let cfg = match ClientConfig::new(env::args()) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("Error: {}", v);
            process::exit(1);
        }
    };
    let mut client = match Client::new(cfg) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("Error: {}", v);
            process::exit(1);
        }
    };
    if let Err(e) = client.calculate() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}