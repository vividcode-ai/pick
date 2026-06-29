use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri, header};
use axum::response::IntoResponse;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist"]
struct WebAssets;

pub async fn spa_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(&mime).unwrap());
        return (StatusCode::OK, headers, content.data.to_vec());
    }

    if let Some(content) = WebAssets::get("index.html") {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html"));
        return (StatusCode::OK, headers, content.data.to_vec());
    }

    (
        StatusCode::NOT_FOUND,
        HeaderMap::new(),
        b"Not found".to_vec(),
    )
}
