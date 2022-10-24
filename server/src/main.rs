use std::env;
use std::process;
use server::{Server, ServerConfig};

fn main() {
    let cfg = match ServerConfig::new(env::args()) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("Error: {}", v);
            process::exit(1);
        }
    };
    let mut server = match Server::new(cfg) {
        Ok(v) => v,
        Err(v) => {
            eprintln!("Error: {}", v);
            process::exit(1);
        }
    };
    if let Err(e) = server.start() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}
