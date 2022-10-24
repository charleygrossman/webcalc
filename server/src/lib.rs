use std::io::prelude::*;
use std::error::Error;
use std::env;
use std::fmt;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

pub struct Server {
    addr: String,
    pool: WorkerPool,
}

impl Server {
    pub fn new(cfg: ServerConfig) -> Result<Server, ServerError> {
        let pool = match WorkerPool::new(cfg.pool_size) {
            Ok(v) => v,
            Err(v) => return Err(ServerError::new(v.to_string()))
        };
        return Ok(Server {
            addr: format!("{}:{}", cfg.host, cfg.port),
            pool: pool,
        });
    }

    pub fn start(&mut self) -> Result<(), ServerError> {
        let listener = match TcpListener::bind(self.addr.clone()) {
            Ok(v) => v,
            Err(v) => return Err(ServerError::new(format!(
                "failed to serve on {}: {}", self.addr, v)))
        };
        for conn in listener.incoming() {
            let conn = match conn {
                Ok(v) => v,
                Err(v) => return Err(ServerError::new(format!("connection error: {}", v)))
            };
            let job = Job::new(Box::new(|| { handle_conn(conn)}));
            if let Err(e) = self.pool.execute(job) {
                return Err(ServerError::new(e.to_string()));
            }
        }
        return Ok(());
    }
}

fn handle_conn(mut conn: TcpStream) {
    const POST_REQ: &[u8] = b"POST / HTTP/1.1\r\n";
    const OK_RESP: &str = "<!DOCTYPE html><html lang=\"en\"><head>
        <meta charset=\"utf-8\"><title>ok</title></head><body>
        <h1>ok</h1></body></html>";
    const NOT_FOUND_RESP: &str = "<!DOCTYPE html><html lang=\"en\"><head>
        <meta charset=\"utf-8\"><title>not found</title></head><body>
        <h1>not found</h1></body></html>";

    let resp: String;
    let mut buf = [0; 1024];
    conn.read(&mut buf).unwrap();
    if buf.starts_with(POST_REQ) {
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
    conn.write(resp.as_bytes()).unwrap();
    conn.flush().unwrap();
}

#[derive(Debug)]
pub struct ServerError {
    msg: String,
}

impl ServerError {
    pub fn new(msg: String) -> ServerError {
        return ServerError { msg };
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "server error: {}", self.msg);
    }
}

impl Error for ServerError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

pub struct ServerConfig {
    pub host: String,
    pub port: String,
    pub pool_size: usize,
}

impl ServerConfig {
    pub fn new(mut args: env::Args) -> Result<ServerConfig, ServerConfigError> {
        args.next();
        let host = match args.next() {
            Some(v) => v,
            None => return Err(ServerConfigError::new(String::from("failed to get host argument"))),
        };
        let port = match args.next() {
            Some(v) => v,
            None => return Err(ServerConfigError::new(String::from("failed to get port argument"))),
        };
        let pool_size = match args.next() {
            Some(v) => v,
            None => return Err(ServerConfigError::new(String::from("failed to get pool size argument"))),
        };
        let size: usize = match pool_size.trim().parse() {
            Ok(v) => v,
            Err(v) => return Err(ServerConfigError::new(format!(
                "failed to parse pool size as usize: got={} error={}",
                pool_size, v
            ))),
        };
        if size < WORKER_POOL_MIN_SIZE || size > WORKER_POOL_MAX_SIZE {
            return Err(ServerConfigError::new(format!(
                "invalid pool size: min={} max={} got={}",
                WORKER_POOL_MIN_SIZE, WORKER_POOL_MAX_SIZE, size
            )));
        }
        return Ok(ServerConfig {
            host: host,
            port: port,
            pool_size: size,
        });
    }
}

#[derive(Debug)]
pub struct ServerConfigError {
    msg: String,
}

impl ServerConfigError {
    pub fn new(msg: String) -> ServerConfigError {
        return ServerConfigError { msg };
    }
}

impl fmt::Display for ServerConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to initialize server config: {}", self.msg);
    }
}

impl Error for ServerConfigError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

struct Job {
    id: Uuid,
    do_fn: Box<dyn FnOnce() + Send + 'static>,
}

impl Job {
    fn new(do_fn: Box<dyn FnOnce() + Send + 'static>) -> Job {
        return Job {
            id: Uuid::new_v4(),
            do_fn: do_fn,
        };
    }
}

enum JobMessage {
    NewJob(Job),
    Terminate,
}

const WORKER_POOL_MIN_SIZE: usize = 1;
const WORKER_POOL_MAX_SIZE: usize = 16;

struct WorkerPool {
    workers: Vec<Worker>,
    job_send: mpsc::Sender<JobMessage>,
}

impl WorkerPool {
    fn new(size: usize) -> Result<WorkerPool, WorkerPoolNewError> {
        let (send, recv) = mpsc::channel();
        let recv = Arc::new(Mutex::new(recv));
        let mut result = WorkerPool {
            workers: Vec::with_capacity(size),
            job_send: send,
        };
        for i in 0..size {
            let w = match Worker::new(i as u16, Arc::clone(&recv)) {
                Ok(w) => w,
                Err(w) => return Err(WorkerPoolNewError::new(w.to_string())),
            };
            result.workers.push(w);
        }
        return Ok(result);
    }

    fn execute(&self, job: Job) -> Result<(), WorkerPoolExecuteError> {
        let job_id = job.id;
        if let Err(e) = self.job_send.send(JobMessage::NewJob(job)) {
            return Err(WorkerPoolExecuteError::new(format!(
                "job_id={} err={}",
                job_id,
                e.to_string()
            )));
        }
        return Ok(());
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        for _ in &mut self.workers {
            self.job_send.send(JobMessage::Terminate).unwrap();    
        }
        for w in &mut self.workers {
            if let Some(thread) = w.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

#[derive(Debug)]
pub struct WorkerPoolNewError {
    msg: String,
}

impl WorkerPoolNewError {
    pub fn new(msg: String) -> WorkerPoolNewError {
        return WorkerPoolNewError { msg };
    }
}

impl fmt::Display for WorkerPoolNewError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to initialize worker pool: {}", self.msg);
    }
}

impl Error for WorkerPoolNewError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

#[derive(Debug)]
pub struct WorkerPoolExecuteError {
    msg: String,
}

impl WorkerPoolExecuteError {
    pub fn new(msg: String) -> WorkerPoolExecuteError {
        return WorkerPoolExecuteError { msg };
    }
}

impl fmt::Display for WorkerPoolExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to execute job: {}", self.msg);
    }
}

impl Error for WorkerPoolExecuteError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}

struct Worker {
    id: u16,
    thread: Option<thread::JoinHandle<()>>,
}

impl Worker {
    fn new(id: u16, job_recv: Arc<Mutex<mpsc::Receiver<JobMessage>>>) -> Result<Worker, Box<dyn Error>> {
        let t = thread::Builder::new().spawn(move || {
            loop {
                let job_msg = job_recv.lock().unwrap().recv().unwrap();
                match job_msg {
                    JobMessage::NewJob(job) => {
                        println!("new job received: worker_id={} job_id={}", id, job.id);
                        (job.do_fn)();
                    }, 
                    JobMessage::Terminate => {
                        println!("terminate received: worker_id={}", id);
                        break;
                    },
                }
            }
        })?;
        return Ok(Worker { id: id, thread: Some(t) });
    }
}

const OPERATOR_ADD: &str = "add";
const OPERATOR_SUB: &str = "sub";

enum Operator {
    Add(String),
    Sub(String),
}

struct Calculation {
    operator: Operator,
    operands: Vec<f64>,
}

impl Calculation {
    fn parse(s: &str) -> Result<Calculation, CalculationParseError> {
        let mut tokens = s.trim().split(",");
        let operator = match tokens.next() {
            Some(v) => {
                let v = v.trim().to_lowercase();
                match v.as_str() {
                    OPERATOR_ADD => Operator::Add(v),
                    OPERATOR_SUB => Operator::Sub(v),
                    _ => return Err(CalculationParseError::new(format!("unknown operator: {}", s)))
                }
            },
            None => {
                return Err(CalculationParseError::new(format!("missing operator: {}", s)))
            },
        };
        let mut operands = Vec::new();
        for t in tokens {
            let v: f64 = match t.trim().parse() {
                Ok(v) => v,
                Err(v) => return Err(CalculationParseError::new(format!("{}: {}", v, s)))
            };
            operands.push(v);
        }
        return Ok(Calculation{operator, operands});
    }
}

#[derive(Debug)]
pub struct CalculationParseError {
    msg: String,
}

impl CalculationParseError {
    pub fn new(msg: String) -> CalculationParseError {
        return CalculationParseError { msg };
    }
}

impl fmt::Display for CalculationParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        return write!(f, "failed to parse calculation: {}", self.msg);
    }
}

impl Error for CalculationParseError {
    fn description(&self) -> &str {
        return &self.msg;
    }
}
