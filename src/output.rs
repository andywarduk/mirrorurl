use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(test)]
use std::sync::{Mutex, MutexGuard};

use log::{Level, Metadata, Record};
#[cfg(test)]
use thread_local::ThreadLocal;

macro_rules! output {
    ($($arg:tt)*) => {{
        log::info!($($arg)*)
    }};
}

macro_rules! error {
    ($($arg:tt)*) => {{
        log::error!($($arg)*)
    }};
}

macro_rules! debug {
    ($state:ident, $level:expr, $($arg:tt)*) => {
        {
            if $level <= $state.debug_level() {
                log::debug!($($arg)*)
            }
        }
    }
}

pub(crate) use {debug, error, output};

#[derive(Debug)]
pub struct Logger {
    all_targets: AtomicBool,
    #[cfg(test)]
    messages: ThreadLocal<Mutex<Vec<String>>>,
}

impl Logger {
    /// Creates a new logger
    pub fn new() -> Self {
        Self {
            all_targets: AtomicBool::new(false),
            #[cfg(test)]
            messages: ThreadLocal::new(),
        }
    }

    /// Sets the flag to log debug/trace from all targets
    pub fn set_all_targets(&self, all_targets: bool) {
        self.all_targets.store(all_targets, Ordering::Relaxed);
    }

    #[cfg(test)]
    /// Locks the messages vector and returns the mutex guard
    fn lock_messages(&self) -> MutexGuard<Vec<String>> {
        self.messages
            .get_or(|| Mutex::new(Vec::new()))
            .lock()
            .expect("Failed to lock messages")
    }

    #[cfg(test)]
    /// Returns the thread message vector and replaces with an empty vector
    pub fn get_messages(&self) -> Vec<String> {
        std::mem::take(&mut *self.lock_messages())
    }
}

impl log::Log for Logger {
    /// Logs the message to stdout/stderr if enabled
    fn log(&self, record: &Record) {
        let metadata = record.metadata();

        if self.enabled(metadata) {
            let level = metadata.level();

            match level {
                Level::Debug | Level::Trace if self.all_targets.load(Ordering::Relaxed) => {
                    eprintln!("{} {}: {}", level, metadata.target(), record.args())
                }
                Level::Error | Level::Warn | Level::Debug | Level::Trace => {
                    eprintln!("{}: {}", level, record.args())
                }
                Level::Info => println!("{}", record.args()),
            }

            #[cfg(test)]
            match level {
                Level::Error | Level::Warn | Level::Info => {
                    let mut messages = self.lock_messages();
                    messages.push(format!("{}: {}", record.level(), record.args()));
                }
                _ => (),
            }
        }
    }

    /// Flush is a no-op
    fn flush(&self) {}

    /// Returns true if the message should be output
    fn enabled(&self, metadata: &Metadata) -> bool {
        if metadata.level() > Level::Info {
            // Debug / Trace - all targets enabled?
            if self.all_targets.load(Ordering::Relaxed) {
                true
            } else {
                metadata.target().starts_with("mirrorurl")
            }
        } else {
            // Error / Warning / Info
            true
        }
    }
}
