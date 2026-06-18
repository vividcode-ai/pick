//! Terminal abstraction

use std::io::Write;

/// Terminal abstraction trait providing terminal I/O operations.
pub trait Terminal: Send + Sync {
    /// Get terminal size as (width, height)
    fn get_size(&self) -> (u16, u16);

    /// Write text to the terminal
    fn write(&self, text: &str);

    /// Read a line of input
    fn read_line(&self) -> String;

    /// Hide the cursor
    fn hide_cursor(&self) {
        print!("\x1b[?25l");
        std::io::stdout().flush().ok();
    }

    /// Show the cursor
    fn show_cursor(&self) {
        print!("\x1b[?25h");
        std::io::stdout().flush().ok();
    }

    /// Clear the entire screen and move cursor to (1,1)
    fn clear_screen(&self) {
        print!("\x1b[2J\x1b[H");
        std::io::stdout().flush().ok();
    }

    /// Clear from cursor to end of line
    fn clear_line(&self) {
        print!("\x1b[K");
        std::io::stdout().flush().ok();
    }

    /// Clear from cursor to end of screen
    fn clear_from_cursor(&self) {
        print!("\x1b[J");
        std::io::stdout().flush().ok();
    }

    /// Move cursor by relative offset
    fn move_by(&self, col: i16, row: i16) {
        let (c, r) = escape_move_by(col, row);
        print!("{}{}", c, r);
        std::io::stdout().flush().ok();
    }

    /// Set window title
    fn set_title(&self, title: &str) {
        print!("\x1b]0;{}\x07", title);
        std::io::stdout().flush().ok();
    }

    /// Enable bracketed paste mode
    fn enable_bracketed_paste(&self) {
        print!("\x1b[?2004h");
        std::io::stdout().flush().ok();
    }

    /// Disable bracketed paste mode
    fn disable_bracketed_paste(&self) {
        print!("\x1b[?2004l");
        std::io::stdout().flush().ok();
    }
}

fn escape_move_by(col: i16, row: i16) -> (String, String) {
    let c = if col > 0 {
        format!("\x1b[{}C", col)
    } else if col < 0 {
        format!("\x1b[{}D", -col)
    } else {
        String::new()
    };
    let r = if row > 0 {
        format!("\x1b[{}B", row)
    } else if row < 0 {
        format!("\x1b[{}A", -row)
    } else {
        String::new()
    };
    (c, r)
}

/// Process-based terminal implementation using crossterm
pub struct ProcessTerminal;

impl Terminal for ProcessTerminal {
    fn get_size(&self) -> (u16, u16) {
        match crossterm::terminal::size() {
            Ok((w, h)) => (w, h),
            Err(_) => (80, 24),
        }
    }

    fn write(&self, text: &str) {
        print!("{}", text);
        std::io::stdout().flush().ok();
    }

    fn read_line(&self) -> String {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        input.trim().to_string()
    }
}
