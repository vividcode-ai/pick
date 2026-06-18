//! Audit mode — view permission audit trail from .pick/audit.jsonl
//!
//! Usage: Pick audit [--recent N] [--tool NAME] [--decision TYPE] [--layer LAYER] [--json]

use std::path::Path;

use crate::args::Args;
use pick_agent::permission::audit::{AuditDecision, AuditEvent};

const PAGE_SIZE: usize = 20;

pub async fn run_audit_command(args: &Args, cwd: &Path) {
    let audit_path = cwd.join(".pick").join("audit.jsonl");

    if !audit_path.exists() {
        eprintln!("No audit data found at: {}", audit_path.display());
        eprintln!("Run Pick normally to generate audit events first.");
        return;
    }

    let events = match read_audit_file(&audit_path) {
        Ok(events) => events,
        Err(e) => {
            eprintln!("Error reading audit file: {}", e);
            return;
        }
    };

    if events.is_empty() {
        println!("No audit events found.");
        return;
    }

    let filtered = filter_events(&events, args);

    if filtered.is_empty() {
        println!("No matching audit events found.");
        return;
    }

    if args.audit_json {
        for event in &filtered {
            if let Ok(json) = serde_json::to_string(event) {
                println!("{}", json);
            }
        }
    } else {
        display_paged(&filtered);
    }
}

fn read_audit_file(path: &Path) -> Result<Vec<AuditEvent>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;

    let mut events = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<AuditEvent>(trimmed) {
            Ok(event) => events.push(event),
            Err(e) => {
                eprintln!("Warning: skipping malformed audit line: {}", e);
            }
        }
    }
    Ok(events)
}

fn filter_events(events: &[AuditEvent], args: &Args) -> Vec<AuditEvent> {
    let mut filtered: Vec<&AuditEvent> = events.iter().collect();

    if let Some(ref tool) = args.audit_tool {
        let tool_lower = tool.to_lowercase();
        filtered.retain(|e| e.tool_name.to_lowercase() == tool_lower);
    }

    if let Some(ref decision) = args.audit_decision {
        let decision_lower = decision.to_lowercase();
        filtered.retain(|e| {
            let d = match e.decision {
                AuditDecision::Allow => "allow",
                AuditDecision::Deny => "deny",
                AuditDecision::Ask => "ask",
            };
            d == decision_lower
        });
    }

    if let Some(ref layer) = args.audit_layer {
        let layer_lower = layer.to_lowercase();
        filtered.retain(|e| format!("{}", e.layer) == layer_lower);
    }

    // Sort by timestamp descending (newest first)
    filtered.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let recent = args.audit_recent.unwrap_or(PAGE_SIZE);
    let count = filtered.len().min(recent);
    filtered.truncate(count);

    filtered.into_iter().cloned().collect()
}

fn display_paged(events: &[AuditEvent]) {
    use std::io::Read;

    let page_size = PAGE_SIZE;
    let total = events.len();
    let mut offset: usize = 0;

    let _ = crossterm::terminal::enable_raw_mode();

    loop {
        let end = (offset + page_size).min(total);
        let page_events = &events[offset..end];

        // Clear screen and move cursor home
        print!("\x1B[2J\x1B[1;1H");

        // Header
        println!(
            " {:<19} | {:<8} | {:<10} | {:<20} | {}",
            "Timestamp", "Tool", "Decision", "Layer", "Target"
        );
        println!(
            " {:-<19}-+-{:-<8}-+-{:-<10}-+-{:-<20}-+-{:-<60}",
            "", "", "", "", ""
        );
        for e in page_events {
            let ts = format_timestamp(e.timestamp);
            let decision = match e.decision {
                AuditDecision::Allow => "allow",
                AuditDecision::Deny => "deny",
                AuditDecision::Ask => "ask",
            };
            let layer = format!("{}", e.layer);
            let target = if e.target.len() > 60 {
                format!("{}...", &e.target[..57])
            } else {
                e.target.clone()
            };
            println!(
                " {:<19} | {:<8} | {:<10} | {:<20} | {}",
                ts, e.tool_name, decision, layer, target
            );
        }

        if end >= total {
            println!();
            println!(" All {} events shown. Press any key to exit.", total);
            let mut buf = [0u8; 1];
            let _ = std::io::stdin().read_exact(&mut buf);
            break;
        }

        println!();
        println!(
            " Page {}/{} (showing {}-{}, {} total) — [Up] next  [Down] prev  [Esc/q] quit",
            (offset / page_size) + 1,
            (total + page_size - 1) / page_size,
            offset + 1,
            end,
            total
        );

        // Read escape sequences (supports arrow keys which send 3-byte sequences)
        let mut buf = [0u8; 3];
        let n = match std::io::stdin().read(&mut buf) {
            Ok(n) => n,
            Err(_) => break,
        };

        if n >= 1 {
            match buf[0] {
                b'q' | b'Q' => break, // q to quit
                0x1b => {
                    // ESC prefix
                    if n >= 3 && buf[1] == 0x5b {
                        match buf[2] {
                            0x41 => {
                                // Up arrow — next page
                                let next_offset = offset + page_size;
                                if next_offset < total {
                                    offset = next_offset;
                                }
                            }
                            0x42 => {
                                // Down arrow — prev page
                                offset = offset.saturating_sub(page_size);
                            }
                            _ => break,
                        }
                    } else {
                        break; // plain Esc — quit
                    }
                }
                _ => break, // any other key — quit
            }
        }
    }

    let _ = crossterm::terminal::disable_raw_mode();
}

fn format_timestamp(ts: i64) -> String {
    let secs = ts / 1000;
    let millis = ts % 1000;
    if let Some(dt) = chrono::DateTime::from_timestamp(secs, 0) {
        format!("{}.{:03}", dt.format("%Y-%m-%d %H:%M:%S"), millis)
    } else {
        format!("{}", ts)
    }
}
