//! Clipboard utilities

/// Read text from clipboard
pub fn read_clipboard_text() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    clipboard.get_text().ok()
}

/// Write text to clipboard
pub fn write_clipboard_text(text: &str) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {}", e))?;
    clipboard
        .set_text(text)
        .map_err(|e| format!("Failed to set clipboard text: {}", e))
}

/// Read image from clipboard (returns PNG bytes)
pub fn read_clipboard_image() -> Option<Vec<u8>> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let image_data = clipboard.get_image().ok()?;
    let width = image_data.width as u32;
    let height = image_data.height as u32;
    let bytes = image_data.bytes;

    // Convert raw RGBA to PNG
    let img = image::RgbaImage::from_raw(width, height, bytes.to_vec())?;
    let mut output = std::io::Cursor::new(Vec::new());
    img.write_to(&mut output, image::ImageFormat::Png).ok()?;
    Some(output.into_inner())
}

/// Write image to clipboard from PNG bytes
pub fn write_clipboard_image(png_bytes: &[u8]) -> Result<(), String> {
    let img =
        image::load_from_memory(png_bytes).map_err(|e| format!("Failed to decode image: {}", e))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Failed to open clipboard: {}", e))?;
    let image_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    };
    clipboard
        .set_image(image_data)
        .map_err(|e| format!("Failed to set clipboard image: {}", e))
}

/// Check if the current session is a Wayland session (Linux only)
pub fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
}

/// Get the MIME type extension for clipboard image data
pub fn extension_for_image_mime_type(mime_type: &str) -> Option<&'static str> {
    match mime_type {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}
