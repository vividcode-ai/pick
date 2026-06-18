//! Process @file CLI arguments into text content and image attachments

use serde_json::Value;
use std::path::Path;

/// Result of processing file arguments
pub struct ProcessedFiles {
    pub text: String,
    pub images: Vec<Value>,
}

/// Options for file processing
pub struct ProcessFileOptions {
    /// Whether to auto-resize images to 2000x2000 max. Default: true
    pub auto_resize_images: bool,
}

impl Default for ProcessFileOptions {
    fn default() -> Self {
        Self {
            auto_resize_images: true,
        }
    }
}

/// Process @file arguments into text content and image attachments
pub async fn process_file_arguments(
    file_args: &[String],
    options: &ProcessFileOptions,
    cwd: &Path,
) -> Result<ProcessedFiles, String> {
    use crate::core::tools::path_utils::resolve_to_cwd;
    use crate::utils::image::detect_supported_image_mime_type;

    let mut text = String::new();
    let mut images: Vec<Value> = Vec::new();

    for file_arg in file_args {
        let cwd_str = cwd.to_string_lossy();
        let absolute_path_str = resolve_to_cwd(file_arg, &cwd_str);
        let absolute_path = Path::new(&absolute_path_str);

        // Check if file exists
        if !absolute_path.exists() {
            return Err(format!("File not found: {}", absolute_path_str));
        }

        // Check if file is empty
        if absolute_path
            .metadata()
            .map(|m| m.len() == 0)
            .unwrap_or(false)
        {
            continue;
        }

        let mime_type = detect_supported_image_mime_type(&absolute_path_str);

        if let Some(mime) = mime_type {
            // Handle image file
            let content = tokio::fs::read(absolute_path)
                .await
                .map_err(|e| format!("Failed to read image: {}", e))?;

            let resized_data = if options.auto_resize_images {
                crate::utils::image::resize_image(&content, Some(2000), Some(2000))
                    .unwrap_or(content)
            } else {
                content
            };

            let attachment = serde_json::json!({
                "type": "image",
                "mimeType": mime,
                "data": resized_data,
            });

            images.push(attachment);
            text.push_str(&format!(r#"<file name="{}"></file>"#, absolute_path_str));
            text.push('\n');
        } else {
            // Handle text file
            let content = tokio::fs::read_to_string(absolute_path)
                .await
                .map_err(|e| format!("Could not read file {}: {}", absolute_path_str, e))?;
            text.push_str(&format!(r#"<file name="{}">"#, absolute_path_str));
            text.push('\n');
            text.push_str(&content);
            text.push('\n');
            text.push_str("</file>\n");
        }
    }

    Ok(ProcessedFiles { text, images })
}
