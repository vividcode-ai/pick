//! Image component for rendering images in terminal

use crate::terminal_image::{
    ImageDimensions, ImageRenderOptions, allocate_image_id, get_capabilities, get_cell_dimensions,
    image_fallback, render_image,
};

/// Options for image display
#[derive(Clone, Default)]
pub struct ImageOptions {
    pub max_width_cells: Option<u32>,
    pub max_height_cells: Option<u32>,
    pub filename: Option<String>,
    pub image_id: Option<u32>,
}

/// Image component that renders an image in the terminal using Kitty or iTerm2 protocol
pub struct Image {
    base64_data: String,
    mime_type: String,
    dimensions: ImageDimensions,
    options: ImageOptions,
    image_id: Option<u32>,
    cached_lines: Option<Vec<String>>,
    cached_width: Option<usize>,
}

impl Image {
    pub fn new(
        base64_data: impl Into<String>,
        mime_type: impl Into<String>,
        options: ImageOptions,
        dimensions: Option<ImageDimensions>,
    ) -> Self {
        let data = base64_data.into();
        let mime = mime_type.into();
        let dims = dimensions.unwrap_or_else(|| {
            get_image_dimensions(&data, &mime).unwrap_or(ImageDimensions {
                width_px: 800,
                height_px: 600,
            })
        });

        Self {
            image_id: options.image_id,
            base64_data: data,
            mime_type: mime,
            dimensions: dims,
            options,
            cached_lines: None,
            cached_width: None,
        }
    }

    pub fn get_image_id(&self) -> Option<u32> {
        self.image_id
    }

    pub fn invalidate(&mut self) {
        self.cached_lines = None;
        self.cached_width = None;
    }

    pub fn render(&mut self, width: usize) -> Vec<String> {
        if let (Some(lines), Some(cw)) = (self.cached_lines.as_ref(), self.cached_width.as_ref())
            && *cw == width
        {
            return lines.clone();
        }

        let max_width = std::cmp::max(
            1,
            std::cmp::min(
                width.saturating_sub(2) as u32,
                self.options.max_width_cells.unwrap_or(60),
            ),
        );
        let cell_dims = get_cell_dimensions();
        let default_max_height = std::cmp::max(
            1,
            (max_width * cell_dims.width_px / cell_dims.height_px.max(1)).max(1),
        );
        let max_height = self.options.max_height_cells.unwrap_or(default_max_height);

        let caps = get_capabilities();
        let lines: Vec<String> = if caps.images.is_some() {
            let img_id = if self.image_id.is_none() {
                let id = allocate_image_id();
                self.image_id = Some(id);
                id
            } else {
                self.image_id.unwrap()
            };

            let opts = ImageRenderOptions {
                max_width_cells: Some(max_width),
                max_height_cells: Some(max_height),
                image_id: Some(img_id),
                move_cursor: Some(false),
                ..Default::default()
            };

            match render_image(&self.base64_data, self.dimensions, opts) {
                Some(result) => {
                    if let Some(rid) = result.image_id {
                        self.image_id = Some(rid);
                    }

                    if caps.images == Some(crate::terminal_image::ImageProtocol::Kitty) {
                        let mut v = vec![result.sequence];
                        for _ in 0..result.rows.saturating_sub(1) {
                            v.push(String::new());
                        }
                        v
                    } else {
                        let mut v = Vec::new();
                        for _ in 0..result.rows.saturating_sub(1) {
                            v.push(String::new());
                        }
                        let row_offset = result.rows.saturating_sub(1);
                        let move_up = if row_offset > 0 {
                            format!("\x1b[{}A", row_offset)
                        } else {
                            String::new()
                        };
                        v.push(move_up + &result.sequence);
                        v
                    }
                }
                None => {
                    let fallback = image_fallback(
                        &self.mime_type,
                        Some(self.dimensions),
                        self.options.filename.as_deref(),
                    );
                    vec![fallback]
                }
            }
        } else {
            let fallback = image_fallback(
                &self.mime_type,
                Some(self.dimensions),
                self.options.filename.as_deref(),
            );
            vec![fallback]
        };

        self.cached_lines = Some(lines.clone());
        self.cached_width = Some(width);
        lines
    }
}

/// Get image dimensions from base64-encoded data
pub fn get_image_dimensions(base64_data: &str, mime_type: &str) -> Option<ImageDimensions> {
    let bytes = base64_decode(base64_data)?;

    match mime_type {
        "image/png" => get_png_dims(&bytes),
        "image/jpeg" => get_jpeg_dims(&bytes),
        "image/gif" => get_gif_dims(&bytes),
        "image/webp" => get_webp_dims(&bytes),
        _ => None,
    }
}

fn base64_decode(data: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

fn get_png_dims(data: &[u8]) -> Option<ImageDimensions> {
    if data.len() < 24 || data[0] != 0x89 || data[1] != 0x50 || data[2] != 0x4e || data[3] != 0x47 {
        return None;
    }
    let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some(ImageDimensions {
        width_px: width,
        height_px: height,
    })
}

fn get_jpeg_dims(data: &[u8]) -> Option<ImageDimensions> {
    if data.len() < 2 || data[0] != 0xff || data[1] != 0xd8 {
        return None;
    }

    let mut offset = 2;
    while offset + 9 <= data.len() {
        if data[offset] != 0xff {
            offset += 1;
            continue;
        }

        let marker = data[offset + 1];

        if (0xc0..=0xc2).contains(&marker) {
            let height = u16::from_be_bytes([data[offset + 5], data[offset + 6]]);
            let width = u16::from_be_bytes([data[offset + 7], data[offset + 8]]);
            return Some(ImageDimensions {
                width_px: width as u32,
                height_px: height as u32,
            });
        }

        if offset + 3 > data.len() {
            return None;
        }
        let length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
        if length < 2 {
            return None;
        }
        offset += 2 + length as usize;
    }

    None
}

fn get_gif_dims(data: &[u8]) -> Option<ImageDimensions> {
    if data.len() < 10 {
        return None;
    }
    let sig = std::str::from_utf8(&data[..6]).ok()?;
    if sig != "GIF87a" && sig != "GIF89a" {
        return None;
    }
    let width = u16::from_le_bytes([data[6], data[7]]);
    let height = u16::from_le_bytes([data[8], data[9]]);
    Some(ImageDimensions {
        width_px: width as u32,
        height_px: height as u32,
    })
}

fn get_webp_dims(data: &[u8]) -> Option<ImageDimensions> {
    if data.len() < 30 {
        return None;
    }
    let riff = std::str::from_utf8(&data[..4]).ok()?;
    let webp = std::str::from_utf8(&data[8..12]).ok()?;
    if riff != "RIFF" || webp != "WEBP" {
        return None;
    }

    let chunk = std::str::from_utf8(&data[12..16]).ok()?;
    match chunk {
        "VP8 " => {
            if data.len() < 30 {
                return None;
            }
            let w = u16::from_le_bytes([data[26], data[27]]) & 0x3fff;
            let h = u16::from_le_bytes([data[28], data[29]]) & 0x3fff;
            Some(ImageDimensions {
                width_px: w as u32,
                height_px: h as u32,
            })
        }
        "VP8L" => {
            if data.len() < 25 {
                return None;
            }
            let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
            let w = (bits & 0x3fff) + 1;
            let h = ((bits >> 14) & 0x3fff) + 1;
            Some(ImageDimensions {
                width_px: w,
                height_px: h,
            })
        }
        "VP8X" => {
            if data.len() < 30 {
                return None;
            }
            let w =
                (u32::from(data[24]) | (u32::from(data[25]) << 8) | (u32::from(data[26]) << 16))
                    + 1;
            let h =
                (u32::from(data[27]) | (u32::from(data[28]) << 8) | (u32::from(data[29]) << 16))
                    + 1;
            Some(ImageDimensions {
                width_px: w,
                height_px: h,
            })
        }
        _ => None,
    }
}
