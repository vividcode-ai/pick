use axum::http::{Uri, header};
use axum::response::IntoResponse;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../web/dist"]
struct WebAssets;

pub async fn spa_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = WebAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return ([(header::CONTENT_TYPE, mime.as_ref())], content.data);
    }

    if let Some(content) = WebAssets::get("index.html") {
        return ([(header::CONTENT_TYPE, "text/html")], content.data);
    }

    (
        [(header::CONTENT_TYPE, "text/plain")],
        std::borrow::Cow::Borrowed(b"Not found"),
    )
}
