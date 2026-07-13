//! Threading Support for Roze
//!
//! This module provides multi-threading capabilities

use std::thread;

pub struct ThreadPool {
    threads: Vec<thread::JoinHandle<()>>,
}

impl ThreadPool {
    pub fn new(size: usize) -> Self {
        let mut threads = Vec::with_capacity(size);
        for i in 0..size {
            threads.push(thread::spawn(move || {
                // Worker thread loop
                println!("🧵 Thread {} started", i);
            }));
        }
        Self { threads }
    }

    pub fn join_all(&mut self) {
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Default for ThreadPool {
    fn default() -> Self {
        Self::new(4) // Default to 4 threads
    }
}

// Simple thread spawn function
pub fn spawn<F, T>(f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    thread::spawn(f)
}

// Get CPU count (simple fallback)
pub fn cpu_count() -> usize {
    // Try to get from environment, fallback to 4
    std::env::var("NUM_CPUS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4)
}