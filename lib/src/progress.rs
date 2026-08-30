//! A tiny transient spinner for long steps (resolving, downloading). It draws
//! on one line and erases itself when stopped, so nothing is left behind.

use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct Spinner {
    done: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    /// Start a spinner with a message. Draws to stderr so normal output stays
    /// clean and pipeable. When stderr isn't a terminal it's a no-op, so piped
    /// or captured output stays clean.
    pub fn start(message: impl Into<String>) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        if !std::io::stderr().is_terminal() {
            return Spinner { done, handle: None };
        }
        let flag = done.clone();
        let message = message.into();
        let handle = thread::spawn(move || {
            const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            while !flag.load(Ordering::Relaxed) {
                eprint!("\r  {} {}", FRAMES[i % FRAMES.len()], message);
                let _ = std::io::stderr().flush();
                i += 1;
                thread::sleep(Duration::from_millis(80));
            }
        });
        Spinner {
            done,
            handle: Some(handle),
        }
    }

    /// Stop the spinner and clear its line.
    pub fn stop(mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
            // Carriage return + clear-to-end-of-line (only when we drew).
            eprint!("\r\x1b[2K");
            let _ = std::io::stderr().flush();
        }
    }
}
