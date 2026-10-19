//! One log for the whole app: shown in the window, written to a file,
//! and printed to the console (when there is one).
//! The file is `bocchi.log`.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// How many lines the window keeps.
const KEEP: usize = 500;
const FILE: &str = "bocchi.log";

fn lines() -> &'static Mutex<VecDeque<String>> {
    static LINES: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    LINES.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn write(message: String) {
    let line = format!("[{}] {message}", time_now());
    println!("{line}");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(FILE) {
        let _ = writeln!(file, "{line}");
    }
    let mut lines = lines().lock().unwrap();
    lines.push_back(line);
    while lines.len() > KEEP {
        lines.pop_front();
    }
}

/// A copy of the recent lines, oldest first.
pub fn recent() -> Vec<String> {
    lines().lock().unwrap().iter().cloned().collect()
}

/// Local time, e.g. 04:27:14.
fn time_now() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// `log!("Joined {}", name)` works like `println!`.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => { $crate::log::write(format!($($arg)*)) };
}
