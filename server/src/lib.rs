use std::error::Error;
use std::fmt;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use uuid::Uuid;

pub struct Job {
    id: Uuid,
    do_fn: Box<dyn FnOnce() + Send + 'static>,
}

impl Job {
    pub fn new(do_fn: Box<dyn FnOnce() + Send + 'static>) -> Job {
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

pub const WORKER_POOL_MIN_SIZE: usize = 1;
pub const WORKER_POOL_MAX_SIZE: usize = 16;

pub struct WorkerPool {
    workers: Vec<Worker>,
    job_send: mpsc::Sender<JobMessage>,
}

impl WorkerPool {
    pub fn new(size: usize) -> Result<WorkerPool, WorkerPoolNewError> {
        if size < WORKER_POOL_MIN_SIZE || size > WORKER_POOL_MAX_SIZE {
            return Err(WorkerPoolNewError::new(format!(
                "invalid size: min={} max={} got={}",
                WORKER_POOL_MIN_SIZE, WORKER_POOL_MAX_SIZE, size
            )));
        }

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

    pub fn execute(&self, job: Job) -> Result<(), WorkerPoolExecuteError> {
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
