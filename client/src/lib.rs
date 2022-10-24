use std::io::prelude::*;
use std::error::Error;
use std::env;
use std::fmt;
use std::fs::File;
use std::net::TcpStream;

pub struct Client {
    server_addr: String,
    infile: File,
    outfile: File,
}

impl Client {
    pub fn new(cfg: ClientConfig) -> Result<Client, ClientNewError> {
        let infile = match File::open(cfg.inpath) {
            Ok(v) => v,
            Err(v) => return Err(ClientNewError::new(v.to_string()))
        };
        let outfile = match File::open(cfg.outpath) {
            Ok(v) => v,
            Err(v) => return Err(ClientNewError::new(v.to_string()))
        };
        return Ok(Client {
            server_addr: cfg.server_addr,
            infile: infile,
            outfile: outfile,
        });
    }

    pub fn calculate(&mut self) -> Result<(), ClientCalculateError> {
        let mut input = String::new();
        if let Err(e) = self.infile.read_to_string(&mut input) {
            return Err(ClientCalculateError::new(format!("failed to read input file: {}", e)));
        }
        for line in input.lines() {
            let req = match Client::parse_request(&line) {
                Ok(v) => v,
                Err(v) => return Err(ClientCalculateError::new(format!("{}: {}", v, &line)))
            };
            println!("REQ: {}", req.clone());
            let mut conn = match TcpStream::connect(self.server_addr.clone()) {
                Ok(v) => v,
                Err(v) => {
                    return Err(ClientCalculateError::new(v.to_string()))
                },
            };
            if let Err(e) = conn.write(req.as_bytes()) {
                return Err(ClientCalculateError::new(e.to_string()))
            }
            if let Err(e) = conn.flush() {
                return Err(ClientCalculateError::new(e.to_string()))
            }
            let mut buf = [0; 1024];
            if let Err(e) = conn.read(&mut buf) {
                return Err(ClientCalculateError::new(e.to_string()))
            }
            println!("RESP: {}", String::from_utf8_lossy(&buf[..]));
        }
        return Ok(());
    }

    fn parse_request(s: &str) -> Result<String, &'static str> {
        let mut tokens = s.trim().split(",");
        let operator = match tokens.next() {
            Some(v) => v,
            None => {
                return Err("missing operator");
            },
        };
        let operands: Vec<& str> = tokens.collect();
        let content = format!("operator={}&operands={:?}", operator, operands);
        return Ok(format!(
                "POST / HTTP/1.1\r\n
                Content-Type: application/x-www-form-urlencoded\r\n
                Content-Length: {}\r\n\r\n{}",
                content.len(), content));
    }
}

#[derive(Debug)]
pub struct ClientNewError {
    msg: String,
}

impl ClientNewError {
    pub fn new(msg: String) -> ClientNewError {
        return ClientNewError { msg };
    }
}

impl fmt::Display for ClientNewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to initialize client: {}", self.msg);
    }
}

impl Error for ClientNewError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

#[derive(Debug)]
pub struct ClientCalculateError {
    msg: String,
}

impl ClientCalculateError {
    pub fn new(msg: String) -> ClientCalculateError {
        return ClientCalculateError { msg };
    }
}

impl fmt::Display for ClientCalculateError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "calculate error: {}", self.msg);
    }
}

impl Error for ClientCalculateError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

pub struct ClientConfig {
    pub server_addr: String,
    pub inpath: String,
    pub outpath: String,
}

impl ClientConfig {
    pub fn new(mut args: env::Args) -> Result<ClientConfig, ClientConfigError> {
        let inpath = match env::var("INPUT_FILEPATH") {
            Ok(v) => v,
            Err(v) => return Err(ClientConfigError::new(format!("must set INPUT_FILEPATH: {}", v)))
        };
        let outpath = match env::var("OUTPUT_FILEPATH") {
            Ok(v) => v,
            Err(v) => return Err(ClientConfigError::new(format!("must set OUTPUT_FILEPATH: {}", v)))
        };
        args.next();
        let addr = match args.next() {
            Some(v) => v,
            None => return Err(ClientConfigError::new(String::from("failed to get server address argument"))),
        };
        return Ok(ClientConfig {
            server_addr: addr,
            inpath: inpath,
            outpath: outpath,
        });
    }
}

#[derive(Debug)]
pub struct ClientConfigError {
    msg: String,
}

impl ClientConfigError {
    pub fn new(msg: String) -> ClientConfigError {
        return ClientConfigError { msg };
    }
}

impl fmt::Display for ClientConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to initialize client config: {}", self.msg);
    }
}

impl Error for ClientConfigError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}