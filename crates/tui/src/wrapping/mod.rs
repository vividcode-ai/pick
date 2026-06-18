pub(crate) mod url;
pub(crate) mod options;
pub(crate) mod standard;
pub(crate) mod adaptive;
pub(crate) mod helpers;

pub use options::RtOptions;

pub(crate) use url::{line_contains_url_like, line_has_mixed_url_and_non_url_tokens};
pub(crate) use standard::{word_wrap_lines};
pub(crate) use adaptive::{adaptive_wrap_line, adaptive_wrap_lines};
