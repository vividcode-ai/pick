//! Image processing utilities

use std::io::Cursor;
use std::path::Path;

/// Supported output formats for image conversion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

/// Image orientation based on EXIF data
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOrientation {
    Normal,
    FlipHorizontal,
    Rotate180,
    FlipVertical,
    Rotate90FlipHorizontal,
    Rotate90,
    Rotate90FlipVertical,
    Rotate270,
}

/// Read image orientation from EXIF data
pub fn read_exif_orientation(path: &Path) -> Option<ImageOrientation> {
    let file = std::fs::read(path).ok()?;
    let mut reader = std::io::BufReader::new(Cursor::new(file));
    let exif_reader = exif::Reader::new();
    let exif = exif_reader.read_from_container(&mut reader).ok()?;

    let orientation = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    let value = orientation.value.display_as(exif::Tag::Orientation).to_string();
    match value.as_str() {
        "1" => Some(ImageOrientation::Normal),
        "2" => Some(ImageOrientation::FlipHorizontal),
        "3" => Some(ImageOrientation::Rotate180),
        "4" => Some(ImageOrientation::FlipVertical),
        "5" => Some(ImageOrientation::Rotate90FlipHorizontal),
        "6" => Some(ImageOrientation::Rotate90),
        "7" => Some(ImageOrientation::Rotate90FlipVertical),
        "8" => Some(ImageOrientation::Rotate270),
        _ => None,
    }
}

/// Apply orientation correction to image data
pub fn apply_orientation(bytes: &[u8], orientation: ImageOrientation) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("Failed to load image: {}", e))?;
    let rotated = match orientation {
        ImageOrientation::Normal => img,
        ImageOrientation::FlipHorizontal => img.fliph(),
        ImageOrientation::Rotate180 => img.rotate180(),
        ImageOrientation::FlipVertical => img.flipv(),
        ImageOrientation::Rotate90 => img.rotate90(),
        ImageOrientation::Rotate270 => img.rotate270(),
        ImageOrientation::Rotate90FlipHorizontal => img.rotate90().fliph(),
        ImageOrientation::Rotate90FlipVertical => img.rotate90().flipv(),
    };

    let mut output = Cursor::new(Vec::new());
    rotated.write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image: {}", e))?;
    Ok(output.into_inner())
}

/// Resize image to target dimensions (maintaining aspect ratio if only one dimension provided)
pub fn resize_image(
    bytes: &[u8],
    width: Option<u32>,
    height: Option<u32>,
) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("Failed to load image: {}", e))?;

    let (new_width, new_height) = match (width, height) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let h = (img.height() as f64 * w as f64 / img.width() as f64).round() as u32;
            (w, h.max(1))
        }
        (None, Some(h)) => {
            let w = (img.width() as f64 * h as f64 / img.height() as f64).round() as u32;
            (w.max(1), h)
        }
        (None, None) => return Ok(bytes.to_vec()),
    };

    let resized = img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);
    let mut output = Cursor::new(Vec::new());
    resized.write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode resized image: {}", e))?;
    Ok(output.into_inner())
}

/// Convert image to a different format
pub fn convert_image(bytes: &[u8], target_format: ImageFormat) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("Failed to load image: {}", e))?;

    let rust_format = match target_format {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::Webp => image::ImageFormat::WebP,
    };

    let mut output = Cursor::new(Vec::new());
    img.write_to(&mut output, rust_format)
        .map_err(|e| format!("Failed to encode image as {:?}: {}", target_format, e))?;
    Ok(output.into_inner())
}

/// Detect supported image MIME type from file extension
pub fn detect_supported_image_mime_type(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_lowercase();
    ImageFormat::from_extension(&ext).map(|f| f.mime_type())
}

/// Check if a file is a supported image type
pub fn is_supported_image(path: &str) -> bool {
    detect_supported_image_mime_type(path).is_some()
}
