use crossbeam::channel;
use log::{error, info, warn};
use std::{
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use thiserror::Error;

type Job = Box<dyn FnOnce() + Send + 'static>;
struct Worker {
    id: usize,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, rx: Arc<Mutex<channel::Receiver<Job>>>) -> Worker {
        let thread = thread::spawn(move || loop {
            let message = rx.lock().unwrap().recv();
            match message {
                Ok(job) => {
                    info!("Worker {} got a job: executing", id);
                    job();
                }
                Err(_) => {
                    error!("Working {} disconnected, shutting down...", id);
                    break;
                }
            }
        });
        Worker {
            id,
            thread: Some(thread),
        }
    }
}

pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<channel::Sender<Job>>,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PoolCreationError {
    #[error("Size of thread pool cannot be zero")]
    SizeZero,
}
impl ThreadPool {
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0);
        let mut workers: Vec<Worker> = Vec::new();
        let (tx, rx): (channel::Sender<Job>, channel::Receiver<Job>) = channel::unbounded();
        let rx = Arc::new(Mutex::new(rx));
        for i in 0..size {
            workers.push(Worker::new(i, Arc::clone(&rx)));
        }
        ThreadPool {
            workers,
            sender: Some(tx),
        }
    }
    pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError> {
        match size {
            0 => Err(PoolCreationError::SizeZero),
            _ => {
                let mut workers: Vec<Worker> = Vec::new();
                let (tx, rx): (channel::Sender<Job>, channel::Receiver<Job>) = channel::unbounded();
                let rx = Arc::new(Mutex::new(rx));
                for i in 0..size {
                    workers.push(Worker::new(i, Arc::clone(&rx)));
                }
                Ok(ThreadPool {
                    workers,
                    sender: Some(tx),
                })
            }
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender.as_ref().unwrap().send(job).unwrap();
    }
}
impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            warn!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}
