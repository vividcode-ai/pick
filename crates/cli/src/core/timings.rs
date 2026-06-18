//! Timing instrumentation for startup profiling

//! Enable with PI_TIMING=1 or pick_TIMING=1 environment variable.

use std::sync::Mutex;
use std::time::Instant;

static TIMINGS: Mutex<Option<Vec<TimingEntry>>> = Mutex::new(None);
static LAST_TIME: Mutex<Option<Instant>> = Mutex::new(None);

struct TimingEntry {
    label: String,
    ms: f64,
}

fn is_enabled() -> bool {
    std::env::var("PI_TIMING").as_deref() == Ok("1")
        || std::env::var("pick_TIMING").as_deref() == Ok("1")
}

/// Reset all timing data
pub fn reset_timings() {
    if !is_enabled() {
        return;
    }
    let mut data = TIMINGS.lock().unwrap();
    *data = Some(Vec::new());
    let mut last = LAST_TIME.lock().unwrap();
    *last = Some(Instant::now());
}

/// Record a timing entry
pub fn time(label: &str) {
    if !is_enabled() {
        return;
    }
    let now = Instant::now();
    let mut last = LAST_TIME.lock().unwrap();
    let elapsed = last.map(|t| now.duration_since(t).as_secs_f64() * 1000.0);
    *last = Some(now);

    if let Some(ms) = elapsed {
        let mut data = TIMINGS.lock().unwrap();
        data.get_or_insert_with(Vec::new).push(TimingEntry {
            label: label.to_string(),
            ms,
        });
    }
}

/// Print all timing data to stderr
pub fn print_timings() {
    if !is_enabled() {
        return;
    }
    let data = TIMINGS.lock().unwrap();
    let entries = match data.as_ref() {
        Some(e) if !e.is_empty() => e,
        _ => return,
    };

    eprintln!("\n--- Startup Timings ---");
    let mut total = 0.0;
    for entry in entries {
        eprintln!("  {}: {:.0}ms", entry.label, entry.ms);
        total += entry.ms;
    }
    eprintln!("  TOTAL: {:.0}ms", total);
    eprintln!("------------------------\n");
}
