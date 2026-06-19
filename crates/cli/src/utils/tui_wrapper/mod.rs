//! Minimal TUI wrapper for selectors.
//! Provides terminal raw-mode setup, keyboard event handling, and rendering helpers.

pub mod selector;

pub use selector::{run_extended_selector, run_list_selector};

use std::io::Write;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::queue;
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size};

#[derive(Debug)]
pub enum SelectResult {
    Selected(usize),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum ExtendedSelectResult {
    Selected(usize),
    Cancelled,
    ToggleScope,
    Preview(usize),
    Delete(usize),
}

#[derive(Debug, Clone, PartialEq)]
enum Key {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Esc,
    Backspace,
    Tab,
    CtrlC,
    CtrlD,
    CtrlE,
    Delete,
    Char(char),
}

fn read_key() -> Option<Key> {
    loop {
        match crossterm::event::read().ok()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                return match key.code {
                    KeyCode::Up => Some(Key::Up),
                    KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Key::Up)
                    }
                    KeyCode::Down => Some(Key::Down),
                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Key::Down)
                    }
                    KeyCode::Left => Some(Key::Left),
                    KeyCode::Right => Some(Key::Right),
                    KeyCode::PageUp => Some(Key::PageUp),
                    KeyCode::PageDown => Some(Key::PageDown),
                    KeyCode::Home => Some(Key::Home),
                    KeyCode::End => Some(Key::End),
                    KeyCode::Enter => Some(Key::Enter),
                    KeyCode::Esc => Some(Key::Esc),
                    KeyCode::Backspace => Some(Key::Backspace),
                    KeyCode::Tab => Some(Key::Tab),
                    KeyCode::Delete => Some(Key::Delete),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Key::CtrlC)
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Key::CtrlD)
                    }
                    KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Key::CtrlE)
                    }
                    KeyCode::Char(c) => Some(Key::Char(c)),
                    _ => None,
                };
            }
            _ => continue,
        }
    }
}

pub fn read_single_key() -> Option<char> {
    loop {
        match crossterm::event::read().ok()? {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press
                    && let KeyCode::Char(c) = key.code
                {
                    return Some(c);
                }
                continue;
            }
            _ => continue,
        }
    }
}

pub fn show_session_preview(
    first_message: &str,
    all_text: &str,
    message_count: usize,
    session_id: &str,
) {
    if enable_raw_mode().is_err() {
        return;
    }
    let mut stdout = std::io::stdout();
    let _ = queue!(stdout, Hide);
    let _ = stdout.flush();

    let (width, height) = size().unwrap_or((80, 24));

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("\x1b[1mSession Preview: {}\x1b[0m", session_id));
    lines.push(format!(
        "\x1b[2m{} messages | first: {}\x1b[0m",
        message_count,
        first_message.chars().take(60).collect::<String>()
    ));
    lines.push(String::new());
    lines.push("\x1b[2m── Transcript ──────────────────────────────────\x1b[0m".to_string());

    let max_lines = (height as usize).saturating_sub(6);
    let transcript: Vec<&str> = all_text.split('\n').collect();
    let display_lines = if transcript.len() > max_lines {
        let take = max_lines.saturating_sub(3);
        let mut truncated: Vec<String> = transcript
            .iter()
            .take(take)
            .map(|s| (*s).to_string())
            .collect();
        truncated.push(format!(
            "\x1b[2m... and {} more lines\x1b[0m",
            transcript.len() - take
        ));
        truncated
    } else {
        transcript.iter().map(|s| (*s).to_string()).collect()
    };

    for line in &display_lines {
        let truncated: String = line
            .chars()
            .take(width.saturating_sub(4) as usize)
            .collect();
        lines.push(truncated);
    }

    lines.push(String::new());
    lines.push("\x1b[2mPress any key to close preview\x1b[0m".to_string());

    let output = format!("{}{}", crossterm::cursor::MoveTo(0, 0), lines.join("\r\n"));

    let _ = queue!(stdout, Clear(ClearType::All));
    let _ = queue!(stdout, crossterm::style::Print(output));
    let _ = stdout.flush();

    loop {
        match read_key() {
            Some(_) => break,
            None => continue,
        }
    }

    let _ = queue!(stdout, Show);
    let _ = stdout.flush();
    let _ = disable_raw_mode();
}
