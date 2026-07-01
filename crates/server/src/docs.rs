use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::rest::health,
        crate::rest::list_sessions,
        crate::rest::create_session,
        crate::rest::get_session,
        crate::rest::delete_session,
        crate::rest::update_session,
        crate::rest::fork_session,
        crate::rest::get_session_messages,
        crate::rest::get_session_status,
        crate::rest::summarize_session,
        crate::rest::list_providers,
        crate::routes::ask,
        crate::routes::cancel,
        crate::routes::approve,
        crate::routes::answer_question,
        crate::files::read_file_handler,
        crate::files::list_dir_handler,
        crate::files::find_text_handler,
        crate::files::find_files_handler,
        crate::mcp_routes::list_mcp_servers,
        crate::mcp_routes::add_mcp_server,
        crate::mcp_routes::remove_mcp_server,
        crate::mcp_routes::reconnect_mcp_server,
        crate::plugins::list_plugins,
        crate::pty::create_pty_handler,
        crate::pty::list_pty_handler,
        crate::pty::destroy_pty_handler,
    ),
    components(
        schemas(
            crate::rest::HealthResponse,
            crate::rest::CreateSessionRequest,
            crate::rest::CreateSessionResponse,
            crate::rest::UpdateSessionRequest,
            crate::session::SessionInfo,
            crate::rest::ProviderInfo,
            crate::rest::ModelInfo,
            crate::routes::AskRequest,
            crate::routes::CancelRequest,
            crate::routes::ApproveRequest,
            crate::routes::AnswerQuestionRequest,
            crate::files::FileReadParams,
            crate::files::ListDirParams,
            crate::files::FindTextParams,
            crate::files::FindFilesParams,
            crate::files::FileEntry,
            crate::files::ListDirResponse,
            crate::files::SearchMatch,
            crate::files::FindTextResponse,
            crate::files::FileSearchResult,
            crate::files::FindFilesResponse,
            crate::mcp_routes::AddMcpServerRequest,
            crate::mcp_routes::McpServerStatus,
            crate::plugins::PluginInfo,
            crate::rest::SummarizeResponse,
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "sessions", description = "Session management"),
        (name = "providers", description = "AI provider and model listing"),
        (name = "agent", description = "Agent execution endpoints"),
        (name = "files", description = "File system operations"),
        (name = "mcp", description = "MCP server management"),
        (name = "plugins", description = "Plugin system"),
        (name = "pty", description = "PTY terminal management"),
    ),
)]
pub struct ApiDoc;

/// Serve the OpenAPI JSON spec
pub async fn openapi_json() -> impl IntoResponse {
    let spec = ApiDoc::openapi().to_pretty_json().unwrap_or_default();
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    (StatusCode::OK, headers, spec)
}

/// Serve the Swagger UI HTML page (loads from CDN)
pub async fn docs_ui() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("swagger.html"),
    )
}
