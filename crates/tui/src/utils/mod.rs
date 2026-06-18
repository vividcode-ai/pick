pub(crate) mod ansi;
pub(crate) mod char;
pub(crate) mod slice;
pub(crate) mod wrap;

pub use ansi::{
    AnsiCodeMatch, AnsiCodeTracker, extract_ansi_code, is_image_line, strip_ansi,
    truncate_to_width, visible_width,
};
pub use char::{is_punctuation_char, is_whitespace_char};
pub use slice::{
    ExtractSegmentsResult, SliceResult, extract_segments, slice_by_column, slice_with_width,
};
pub use wrap::{apply_background_to_line, wrap_text_with_ansi};
