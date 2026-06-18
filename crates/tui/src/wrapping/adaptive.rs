use std::ops::Range;

use ratatui::text::Line;
use textwrap::core::display_width;
use textwrap::WordSeparator;
use textwrap::core::Word;

use crate::wrapping::options::{RtOptions, url_preserving_wrap_options};
use crate::wrapping::standard::word_wrap_line;
use crate::wrapping::url::{line_contains_url_like, line_has_mixed_url_and_non_url_tokens, is_url_like_token};
use crate::wrapping::helpers::{push_owned_lines, flatten_line, slice_line_spans, IntoLineInput};

#[must_use]
pub(crate) fn adaptive_wrap_line<'a>(line: &'a Line<'a>, base: RtOptions<'a>) -> Vec<Line<'a>> {
    if !line_contains_url_like(line) {
        return word_wrap_line(line, base);
    }
    if line_has_mixed_url_and_non_url_tokens(line) {
        mixed_url_wrap_line(line, base)
    } else {
        word_wrap_line(line, url_preserving_wrap_options(base))
    }
}

#[allow(private_bounds)]
pub(crate) fn adaptive_wrap_lines<'a, I, L>(
    lines: I,
    width_or_options: RtOptions<'a>,
) -> Vec<Line<'static>>
where
    I: IntoIterator<Item = L>,
    L: IntoLineInput<'a>,
{
    let base_opts = width_or_options;
    let mut out: Vec<Line<'static>> = Vec::new();
    for (idx, line) in lines.into_iter().enumerate() {
        let line_input = line.into_line_input();
        let opts = if idx == 0 {
            base_opts.clone()
        } else {
            base_opts.clone()
                .initial_indent(base_opts.subsequent_indent.clone())
        };
        let wrapped = adaptive_wrap_line(line_input.as_ref(), opts);
        push_owned_lines(&wrapped, &mut out);
    }
    out
}

#[derive(Clone, Debug)]
struct MixedUrlWord {
    range: Range<usize>,
    is_url: bool,
}

impl MixedUrlWord {
    fn width(&self, text: &str) -> usize {
        display_width(&text[self.range.clone()])
    }
}

fn mixed_url_wrap_line<'a>(line: &'a Line<'a>, rt_opts: RtOptions<'a>) -> Vec<Line<'a>> {
    let (flat, span_bounds) = flatten_line(line);
    let initial_width_available = rt_opts.width.saturating_sub(rt_opts.initial_indent.width()).max(1);
    let subsequent_width_available = rt_opts.width.saturating_sub(rt_opts.subsequent_indent.width()).max(1);
    let ranges = mixed_url_wrap_ranges(&flat, initial_width_available, subsequent_width_available);

    let mut out = Vec::new();
    for (idx, range) in ranges.iter().enumerate() {
        let mut wrapped_line = if idx == 0 {
            rt_opts.initial_indent.clone()
        } else {
            rt_opts.subsequent_indent.clone()
        }.style(line.style);
        let sliced = slice_line_spans(line, &span_bounds, range);
        let mut spans = wrapped_line.spans;
        spans.extend(
            sliced.spans.into_iter().map(|span| span.patch_style(line.style)),
        );
        wrapped_line.spans = spans;
        out.push(wrapped_line);
    }

    if out.is_empty() {
        vec![rt_opts.initial_indent.clone()]
    } else {
        out
    }
}

fn mixed_url_wrap_ranges(
    text: &str,
    initial_width: usize,
    subsequent_width: usize,
) -> Vec<Range<usize>> {
    let leading_space_width = text.chars().take_while(|ch| *ch == ' ').count();
    let mut words = Vec::new();
    let mut cursor = 0usize;
    for word in WordSeparator::AsciiSpace.find_words(text) {
        let word_start = cursor;
        let word_end = word_start + word.word.len();
        if !word.word.is_empty() {
            words.push(MixedUrlWord {
                range: word_start..word_end,
                is_url: is_url_like_token(word.word),
            });
        }
        cursor = word_end + word.whitespace.len();
    }

    let mut lines = Vec::new();
    let mut line_start = None;
    let mut line_end = 0usize;
    let mut line_width = 0usize;
    let mut line_limit = initial_width.max(1);

    for word in words {
        let mut pending = split_mixed_url_word(text, word, line_limit);
        let mut pending_idx = 0usize;

        while let Some(piece) = pending.get(pending_idx).cloned() {
            let empty_line_prefix_width = if line_start.is_none() && lines.is_empty() {
                leading_space_width
            } else {
                0
            };
            let empty_line_piece_limit = line_limit.saturating_sub(empty_line_prefix_width).max(1);
            if line_start.is_none() && !piece.is_url && piece.width(text) > empty_line_piece_limit {
                pending.splice(
                    pending_idx..=pending_idx,
                    split_mixed_url_word(text, piece, empty_line_piece_limit),
                );
                continue;
            }

            let piece_width = piece.width(text);
            let inter_word_space = line_start
                .map(|_| text[line_end..piece.range.start].len())
                .unwrap_or(0);
            let fits = if line_start.is_none() {
                piece.is_url
                    || empty_line_prefix_width + piece_width <= line_limit
                    || empty_line_prefix_width >= line_limit
            } else {
                line_width + inter_word_space + piece_width <= line_limit
            };

            if fits {
                if line_start.is_none() {
                    let is_first_output_line = lines.is_empty();
                    let start = if is_first_output_line { 0 } else { piece.range.start };
                    line_start = Some(start);
                    line_width = if is_first_output_line {
                        leading_space_width + piece_width
                    } else {
                        piece_width
                    };
                } else {
                    line_width += inter_word_space + piece_width;
                }
                line_end = piece.range.end;
                pending_idx += 1;
                continue;
            }

            if let Some(start) = line_start.take() {
                lines.push(start..line_end);
            }
            line_end = 0;
            line_width = 0;
            line_limit = subsequent_width.max(1);
        }
    }

    if let Some(start) = line_start {
        lines.push(start..line_end);
    }

    lines
}

fn split_mixed_url_word(text: &str, word: MixedUrlWord, line_limit: usize) -> Vec<MixedUrlWord> {
    if word.is_url || word.width(text) <= line_limit {
        return vec![word];
    }
    let source = Word::from(&text[word.range.clone()]);
    let mut offset = word.range.start;
    let mut pieces = Vec::new();
    for piece in source.break_apart(line_limit.max(1)) {
        let end = offset + piece.word.len();
        pieces.push(MixedUrlWord {
            range: offset..end,
            is_url: false,
        });
        offset = end;
    }
    pieces
}
