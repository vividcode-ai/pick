//! Terminal image rendering for Kitty and iTerm2 protocols

use std::sync::{Mutex, OnceLock};

/// Image protocol supported by the terminal
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageProtocol {
    Kitty,
    Iterm2,
}

/// Terminal capabilities
#[derive(Debug, Clone)]
pub struct TerminalCapabilities {
    pub images: Option<ImageProtocol>,
    pub true_color: bool,
    pub hyperlinks: bool,
}

/// Cell dimensions in pixels
#[derive(Debug, Clone, Copy)]
pub struct CellDimensions {
    pub width_px: u32,
    pub height_px: u32,
}

/// Image dimensions in pixels
#[derive(Debug, Clone, Copy)]
pub struct ImageDimensions {
    pub width_px: u32,
    pub height_px: u32,
}

/// Image render options
#[derive(Debug, Clone)]
pub struct ImageRenderOptions {
    pub max_width_cells: Option<u32>,
    pub max_height_cells: Option<u32>,
    pub preserve_aspect_ratio: Option<bool>,
    pub image_id: Option<u32>,
    pub move_cursor: Option<bool>,
}

impl Default for ImageRenderOptions {
    fn default() -> Self {
        Self {
            max_width_cells: None,
            max_height_cells: None,
            preserve_aspect_ratio: None,
            image_id: None,
            move_cursor: None,
        }
    }
}

static CACHED_CAPABILITIES: OnceLock<TerminalCapabilities> = OnceLock::new();

static CELL_DIMS: Mutex<CellDimensions> = Mutex::new(CellDimensions {
    width_px: 9,
    height_px: 18,
});

pub fn get_cell_dimensions() -> CellDimensions {
    *CELL_DIMS.lock().unwrap()
}

pub fn set_cell_dimensions(dims: CellDimensions) {
    *CELL_DIMS.lock().unwrap() = dims;
}

/// Detect terminal capabilities from environment variables
pub fn detect_capabilities() -> TerminalCapabilities {
    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_lowercase();
    let term = std::env::var("TERM").unwrap_or_default().to_lowercase();
    let color_term = std::env::var("COLORTERM")
        .unwrap_or_default()
        .to_lowercase();
    let has_true_color = color_term == "truecolor" || color_term == "24bit";

    // tmux/screen
    let in_tmux =
        std::env::var("TMUX").is_ok() || term.starts_with("tmux") || term.starts_with("screen");
    if in_tmux {
        return TerminalCapabilities {
            images: None,
            true_color: has_true_color,
            hyperlinks: false,
        };
    }

    // Kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() || term_program == "kitty" {
        return TerminalCapabilities {
            images: Some(ImageProtocol::Kitty),
            true_color: true,
            hyperlinks: true,
        };
    }

    // Ghostty
    if term_program == "ghostty"
        || term.contains("ghostty")
        || std::env::var("GHOSTTY_RESOURCES_DIR").is_ok()
    {
        return TerminalCapabilities {
            images: Some(ImageProtocol::Kitty),
            true_color: true,
            hyperlinks: true,
        };
    }

    // WezTerm
    if std::env::var("WEZTERM_PANE").is_ok() || term_program == "wezterm" {
        return TerminalCapabilities {
            images: Some(ImageProtocol::Kitty),
            true_color: true,
            hyperlinks: true,
        };
    }

    // iTerm2
    if std::env::var("ITERM_SESSION_ID").is_ok() || term_program == "iterm.app" {
        return TerminalCapabilities {
            images: Some(ImageProtocol::Iterm2),
            true_color: true,
            hyperlinks: true,
        };
    }

    // Windows Terminal
    if std::env::var("WT_SESSION").is_ok() {
        return TerminalCapabilities {
            images: None,
            true_color: true,
            hyperlinks: true,
        };
    }

    // VSCode
    if term_program == "vscode" {
        return TerminalCapabilities {
            images: None,
            true_color: true,
            hyperlinks: true,
        };
    }

    // Alacritty
    if term_program == "alacritty" {
        return TerminalCapabilities {
            images: None,
            true_color: true,
            hyperlinks: true,
        };
    }

    TerminalCapabilities {
        images: None,
        true_color: has_true_color,
        hyperlinks: false,
    }
}

/// Get cached terminal capabilities
pub fn get_capabilities() -> TerminalCapabilities {
    CACHED_CAPABILITIES.get_or_init(detect_capabilities).clone()
}

pub fn reset_capabilities_cache() {
    // OnceLock doesn't support reset; capabilities are detected once per process
}

/// Allocate a random image ID for Kitty graphics protocol
pub fn allocate_image_id() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 0xfffffffe).max(1)
}

/// Encode base64 data as Kitty graphics protocol sequence
pub fn encode_kitty(
    base64_data: &str,
    columns: Option<u32>,
    rows: Option<u32>,
    image_id: Option<u32>,
    move_cursor: bool,
) -> String {
    const CHUNK_SIZE: usize = 4096;

    let mut params = Vec::new();
    params.push("a=T".to_string());
    params.push("f=100".to_string());
    params.push("q=2".to_string());

    if !move_cursor {
        params.push("C=1".to_string());
    }
    if let Some(c) = columns {
        params.push(format!("c={}", c));
    }
    if let Some(r) = rows {
        params.push(format!("r={}", r));
    }
    if let Some(id) = image_id {
        params.push(format!("i={}", id));
    }

    let params_str = params.join(",");

    if base64_data.len() <= CHUNK_SIZE {
        return format!("\x1b_G{};{}\x1b\\\\", params_str, base64_data);
    }

    let mut chunks = Vec::new();
    let mut offset = 0;
    let mut is_first = true;

    while offset < base64_data.len() {
        let end = std::cmp::min(offset + CHUNK_SIZE, base64_data.len());
        let chunk = &base64_data[offset..end];
        let is_last = end >= base64_data.len();

        if is_first {
            chunks.push(format!("\x1b_G{},m=1;{}\x1b\\\\", params_str, chunk));
            is_first = false;
        } else if is_last {
            chunks.push(format!("\x1b_Gm=0;{}\x1b\\\\", chunk));
        } else {
            chunks.push(format!("\x1b_Gm=1;{}\x1b\\\\", chunk));
        }

        offset = end;
    }

    chunks.concat()
}

/// Delete a Kitty graphics image by ID
pub fn delete_kitty_image(image_id: u32) -> String {
    format!("\x1b_Ga=d,d=A,i={},q=2\x1b\\\\", image_id)
}

/// Delete all visible Kitty graphics images
pub fn delete_all_kitty_images() -> String {
    "\x1b_Ga=d,d=A,q=2\x1b\\\\".to_string()
}

/// Encode base64 data as iTerm2 inline image protocol
pub fn encode_iterm2(
    base64_data: &str,
    width: Option<&str>,
    height: Option<&str>,
    name: Option<&str>,
    preserve_aspect_ratio: bool,
    inline: bool,
) -> String {
    let mut params = Vec::new();
    params.push(format!("inline={}", if inline { 1 } else { 0 }));

    if let Some(w) = width {
        params.push(format!("width={}", w));
    }
    if let Some(h) = height {
        params.push(format!("height={}", h));
    }
    if let Some(n) = name {
        params.push(format!("name={}", base64_encode_str(n)));
    }
    if !preserve_aspect_ratio {
        params.push("preserveAspectRatio=0".to_string());
    }

    format!("\x1b]1337;File={}:{}\x07", params.join(";"), base64_data)
}

/// Calculate image cell size
#[derive(Debug, Clone, Copy)]
pub struct ImageCellSize {
    pub columns: u32,
    pub rows: u32,
}

pub fn calculate_image_cell_size(
    image_dims: ImageDimensions,
    max_width_cells: u32,
    max_height_cells: Option<u32>,
    cell_dims: CellDimensions,
) -> ImageCellSize {
    let max_width = std::cmp::max(1, max_width_cells);
    let img_w = std::cmp::max(1, image_dims.width_px) as f64;
    let img_h = std::cmp::max(1, image_dims.height_px) as f64;

    let width_scale = (max_width as f64 * cell_dims.width_px as f64) / img_w;
    let height_scale = match max_height_cells {
        Some(mh) if mh > 0 => (mh as f64 * cell_dims.height_px as f64) / img_h,
        _ => width_scale,
    };
    let scale = width_scale.min(height_scale);

    let scaled_w = img_w * scale;
    let scaled_h = img_h * scale;
    let columns = (scaled_w / cell_dims.width_px as f64).ceil() as u32;
    let rows = (scaled_h / cell_dims.height_px as f64).ceil() as u32;

    ImageCellSize {
        columns: std::cmp::max(1, std::cmp::min(max_width, columns)),
        rows: std::cmp::max(
            1,
            match max_height_cells {
                Some(mh) if mh > 0 => std::cmp::min(mh, rows),
                _ => rows,
            },
        ),
    }
}

/// Render an image to a terminal escape sequence
pub struct RenderImageResult {
    pub sequence: String,
    pub rows: u32,
    pub image_id: Option<u32>,
}

pub fn render_image(
    base64_data: &str,
    image_dims: ImageDimensions,
    options: ImageRenderOptions,
) -> Option<RenderImageResult> {
    let caps = get_capabilities();
    let images = caps.images?;

    let max_width = options.max_width_cells.unwrap_or(80);
    let size = calculate_image_cell_size(
        image_dims,
        max_width,
        options.max_height_cells,
        get_cell_dimensions(),
    );

    match images {
        ImageProtocol::Kitty => {
            let image_id = options.image_id.or_else(|| Some(allocate_image_id()));
            let sequence = encode_kitty(
                base64_data,
                Some(size.columns),
                Some(size.rows),
                image_id,
                options.move_cursor.unwrap_or(true),
            );
            Some(RenderImageResult {
                sequence,
                rows: size.rows,
                image_id,
            })
        }
        ImageProtocol::Iterm2 => {
            let sequence = encode_iterm2(
                base64_data,
                Some(&size.columns.to_string()),
                Some("auto"),
                None,
                options.preserve_aspect_ratio.unwrap_or(true),
                true,
            );
            Some(RenderImageResult {
                sequence,
                rows: size.rows,
                image_id: None,
            })
        }
    }
}

/// Create an OSC 8 hyperlink
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, text)
}

/// Create an image fallback text
pub fn image_fallback(
    mime_type: &str,
    dimensions: Option<ImageDimensions>,
    filename: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(f) = filename {
        parts.push(f.to_string());
    }
    parts.push(format!("[{}]", mime_type));
    if let Some(d) = dimensions {
        parts.push(format!("{}x{}", d.width_px, d.height_px));
    }
    format!("[Image: {}]", parts.join(" "))
}

fn base64_encode_str(s: &str) -> String {
    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    engine.encode(s)
}
