use std::io::prelude::*;
use std::error::Error;
use std::env;
use std::fmt;
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;
use regex::Regex;

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
            let job = Job::new(Box::new(|| { handle(conn) }));
            if let Err(e) = self.pool.execute(job) {
                return Err(ServerError::new(e.to_string()));
            }
        }
        return Ok(());
    }
}

fn handle(mut conn: TcpStream) {
    const POST_PREFIX: &str = "POST / HTTP/1.1";
    const BAD_REQ_RESP: &str = "HTTP/1.1 400 BAD REQUEST\r\n";

    let mut buf = [0; 1024];
    let n = conn.read(&mut buf).unwrap();
    let req = String::from_utf8_lossy(&buf[..n]);

    let resp: String;
    if !req.starts_with(POST_PREFIX) {
        resp = String::from(BAD_REQ_RESP);
    } else {
        let r = Regex::new(r"(?s)operator=(.*)&operands=(.*)").unwrap();
        let c = r.captures(req.trim()).unwrap();
        let operator = c.get(1).map_or("", |v| v.as_str());
        let operands = c.get(2).map_or("", |v| v.as_str());
        let calc = Calculation::parse(operator, operands).unwrap();
        let result: Option<f64> = match calc.operator {
            Operator::Add => calc.operands.into_iter().reduce(|a, b| a + b),
            Operator::Sub => calc.operands.into_iter().reduce(|a, b| a - b),
            Operator::Mul => calc.operands.into_iter().reduce(|a, b| a * b),
            Operator::Div => calc.operands.into_iter().reduce(|a, b| a / b),
            Operator::Rem => calc.operands.into_iter().reduce(|a, b| a % b),
        };
        resp = match result {
            Some(v) => v.to_string(),
            None => String::from("nan"),
        };
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
        let mut pool = WorkerPool {
            workers: Vec::with_capacity(size),
            job_send: send,
        };
        for i in 0..size {
            let w = match Worker::new(i as u16, Arc::clone(&recv)) {
                Ok(w) => w,
                Err(w) => return Err(WorkerPoolNewError::new(w.to_string())),
            };
            pool.workers.push(w);
        }
        return Ok(pool);
    }

    fn execute(&self, job: Job) -> Result<(), WorkerPoolExecuteError> {
        let job_id = job.id;
        if let Err(e) = self.job_send.send(JobMessage::NewJob(job)) {
            return Err(WorkerPoolExecuteError::new(format!(
                "job_id={} err={}", job_id, e.to_string())));
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
const OPERATOR_MUL: &str = "mul";
const OPERATOR_DIV: &str = "div";
const OPERATOR_REM: &str = "rem";

#[derive(Debug)]
enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

struct Calculation {
    operator: Operator,
    operands: Vec<f64>,
}

impl Calculation {
    fn parse(operator: &str, operands: &str) -> Result<Calculation, CalculationParseError> {
        let operator = match operator.trim().to_lowercase().as_str() {
            OPERATOR_ADD => Operator::Add,
            OPERATOR_SUB => Operator::Sub,
            OPERATOR_MUL => Operator::Mul,
            OPERATOR_DIV => Operator::Div,
            OPERATOR_REM => Operator::Rem,
            _ => return Err(CalculationParseError::new(format!("unknown operator: {}", operator)))
        };
        let mut ops = Vec::new();
        for t in operands.trim().split(",") {
            let v: f64 = match t.trim().parse() {
                Ok(v) => v,
                Err(v) => return Err(CalculationParseError::new(format!("{}: {}", v, operands)))
            };
            ops.push(v);
        }
        return Ok(Calculation{
            operator: operator,
            operands: ops,
        });
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
