use unicode_width::UnicodeWidthStr;

pub fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new("\x1b\\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}

pub fn visible_width(s: &str) -> usize {
    UnicodeWidthStr::width(strip_ansi(s).as_str())
}

pub fn truncate_to_width(s: &str, max_width: usize) -> String {
    if visible_width(s) <= max_width {
        return s.to_string();
    }
    let target = max_width.saturating_sub(3);
    let mut result = String::new();
    let mut vis_width = 0;
    let mut in_escape = false;
    let mut escape_buf = String::new();
    for c in s.chars() {
        if in_escape {
            escape_buf.push(c);
            if c == 'm' {
                result.push_str(&escape_buf);
                in_escape = false;
                escape_buf.clear();
            }
            continue;
        }
        if c == '\x1b' {
            in_escape = true;
            escape_buf.clear();
            escape_buf.push(c);
            continue;
        }
        let cw = UnicodeWidthStr::width(c.to_string().as_str());
        if vis_width + cw > target {
            result.push_str("...");
            break;
        }
        result.push(c);
        vis_width += cw;
    }
    result
}

pub fn is_image_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("\x1b_G") || trimmed.starts_with("\x1b]1337;File=")
}

pub fn extract_ansi_code(s: &str, pos: usize) -> Option<AnsiCodeMatch> {
    let bytes = s.as_bytes();
    if pos >= bytes.len() || bytes[pos] != 0x1b {
        return None;
    }

    let next = bytes.get(pos + 1).copied();

    if next == Some(b'[') {
        let mut j = pos + 2;
        while j < bytes.len() && !matches!(bytes[j], b'm' | b'G' | b'K' | b'H' | b'J') {
            j += 1;
        }
        if j < bytes.len() {
            let code = s[pos..=j].to_string();
            return Some(AnsiCodeMatch {
                code,
                length: j + 1 - pos,
            });
        }
        return None;
    }

    if next == Some(b']') {
        let mut j = pos + 2;
        while j < bytes.len() {
            if bytes[j] == 0x07 {
                let code = s[pos..=j].to_string();
                return Some(AnsiCodeMatch {
                    code,
                    length: j + 1 - pos,
                });
            }
            if bytes[j] == 0x1b && bytes.get(j + 1) == Some(&b'\\') {
                let code = s[pos..=j + 2].to_string();
                return Some(AnsiCodeMatch {
                    code,
                    length: j + 2 - pos,
                });
            }
            j += 1;
        }
        return None;
    }

    if next == Some(b'_') {
        let mut j = pos + 2;
        while j < bytes.len() {
            if bytes[j] == 0x07 {
                let code = s[pos..=j].to_string();
                return Some(AnsiCodeMatch {
                    code,
                    length: j + 1 - pos,
                });
            }
            if bytes[j] == 0x1b && bytes.get(j + 1) == Some(&b'\\') {
                let code = s[pos..=j + 2].to_string();
                return Some(AnsiCodeMatch {
                    code,
                    length: j + 2 - pos,
                });
            }
            j += 1;
        }
        return None;
    }

    None
}

#[derive(Debug, Clone)]
pub struct AnsiCodeMatch {
    pub code: String,
    pub length: usize,
}

#[derive(Debug, Clone)]
pub struct AnsiCodeTracker {
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    inverse: bool,
    hidden: bool,
    strikethrough: bool,
    fg_color: Option<String>,
    bg_color: Option<String>,
}

impl AnsiCodeTracker {
    pub fn new() -> Self {
        Self {
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            blink: false,
            inverse: false,
            hidden: false,
            strikethrough: false,
            fg_color: None,
            bg_color: None,
        }
    }

    pub fn process(&mut self, ansi_code: &str) {
        if !ansi_code.ends_with('m') {
            return;
        }

        let params = ansi_code
            .trim_start_matches('\x1b')
            .trim_start_matches('[')
            .trim_end_matches('m');

        if params.is_empty() || params == "0" {
            self.reset();
            return;
        }

        let parts: Vec<&str> = params.split(';').collect();
        let mut i = 0;
        while i < parts.len() {
            let code: u16 = match parts[i].parse() {
                Ok(v) => v,
                Err(_) => {
                    i += 1;
                    continue;
                }
            };

            if code == 38 || code == 48 {
                if i + 2 < parts.len() && parts.get(i + 1) == Some(&"5") {
                    let color_code = format!("{};{};{}", parts[i], parts[i + 1], parts[i + 2]);
                    if code == 38 {
                        self.fg_color = Some(color_code);
                    } else {
                        self.bg_color = Some(color_code);
                    }
                    i += 3;
                    continue;
                } else if i + 4 < parts.len() && parts.get(i + 1) == Some(&"2") {
                    let color_code = format!(
                        "{};{};{};{};{}",
                        parts[i],
                        parts[i + 1],
                        parts[i + 2],
                        parts[i + 3],
                        parts[i + 4]
                    );
                    if code == 38 {
                        self.fg_color = Some(color_code);
                    } else {
                        self.bg_color = Some(color_code);
                    }
                    i += 5;
                    continue;
                }
            }

            match code {
                0 => self.reset(),
                1 => self.bold = true,
                2 => self.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                5 => self.blink = true,
                7 => self.inverse = true,
                8 => self.hidden = true,
                9 => self.strikethrough = true,
                21 | 22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                25 => self.blink = false,
                27 => self.inverse = false,
                28 => self.hidden = false,
                29 => self.strikethrough = false,
                39 => self.fg_color = None,
                49 => self.bg_color = None,
                _ => {
                    if (30..=37).contains(&code) || (90..=97).contains(&code) {
                        self.fg_color = Some(code.to_string());
                    } else if (40..=47).contains(&code) || (100..=107).contains(&code) {
                        self.bg_color = Some(code.to_string());
                    }
                }
            }
            i += 1;
        }
    }

    fn reset(&mut self) {
        self.bold = false;
        self.dim = false;
        self.italic = false;
        self.underline = false;
        self.blink = false;
        self.inverse = false;
        self.hidden = false;
        self.strikethrough = false;
        self.fg_color = None;
        self.bg_color = None;
    }

    pub fn clear(&mut self) {
        self.reset();
    }

    pub fn get_active_codes(&self) -> String {
        let mut codes: Vec<String> = Vec::new();
        if self.bold {
            codes.push("1".to_string());
        }
        if self.dim {
            codes.push("2".to_string());
        }
        if self.italic {
            codes.push("3".to_string());
        }
        if self.underline {
            codes.push("4".to_string());
        }
        if self.blink {
            codes.push("5".to_string());
        }
        if self.inverse {
            codes.push("7".to_string());
        }
        if self.hidden {
            codes.push("8".to_string());
        }
        if self.strikethrough {
            codes.push("9".to_string());
        }
        if let Some(ref c) = self.fg_color {
            codes.push(c.clone());
        }
        if let Some(ref c) = self.bg_color {
            codes.push(c.clone());
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }

    pub fn has_active_codes(&self) -> bool {
        self.bold
            || self.dim
            || self.italic
            || self.underline
            || self.blink
            || self.inverse
            || self.hidden
            || self.strikethrough
            || self.fg_color.is_some()
            || self.bg_color.is_some()
    }

    pub fn get_line_end_reset(&self) -> String {
        let mut result = String::new();
        if self.underline {
            result.push_str("\x1b[24m");
        }
        result
    }
}

impl Default for AnsiCodeTracker {
    fn default() -> Self {
        Self::new()
    }
}
