//! Output guard - stdout takeover for controlled output


use std::io::Write;
use std::sync::Mutex;

static RAW_STDOUT_TAIL: Mutex<Option<String>> = Mutex::new(None);

/// Write raw text to stdout bypassing any takeover
pub fn write_raw_stdout(text: &str) {
    if text.is_empty() {
        return;
    }
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(text.as_bytes());
    let _ = stdout.flush();
}

/// Flush any pending raw stdout writes
pub fn flush_raw_stdout() {
    let _ = std::io::stdout().flush();
}
