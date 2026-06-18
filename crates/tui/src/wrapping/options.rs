use ratatui::text::Line;

#[derive(Debug, Clone)]
pub struct RtOptions<'a> {
    pub width: usize,
    pub line_ending: textwrap::LineEnding,
    pub initial_indent: Line<'a>,
    pub subsequent_indent: Line<'a>,
    pub break_words: bool,
    pub wrap_algorithm: textwrap::WrapAlgorithm,
    pub word_separator: textwrap::WordSeparator,
    pub word_splitter: textwrap::WordSplitter,
}

impl From<usize> for RtOptions<'_> {
    fn from(width: usize) -> Self {
        RtOptions::new(width)
    }
}

#[allow(dead_code)]
impl<'a> RtOptions<'a> {
    pub fn new(width: usize) -> Self {
        RtOptions {
            width,
            line_ending: textwrap::LineEnding::LF,
            initial_indent: Line::default(),
            subsequent_indent: Line::default(),
            break_words: true,
            word_separator: textwrap::WordSeparator::new(),
            wrap_algorithm: textwrap::WrapAlgorithm::FirstFit,
            word_splitter: textwrap::WordSplitter::HyphenSplitter,
        }
    }

    pub fn width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }

    pub fn initial_indent(mut self, indent: Line<'a>) -> Self {
        self.initial_indent = indent;
        self
    }

    pub fn subsequent_indent(mut self, indent: Line<'a>) -> Self {
        self.subsequent_indent = indent;
        self
    }

    pub fn break_words(mut self, break_words: bool) -> Self {
        self.break_words = break_words;
        self
    }

    pub fn word_separator(mut self, separator: textwrap::WordSeparator) -> Self {
        self.word_separator = separator;
        self
    }

    pub fn word_splitter(mut self, splitter: textwrap::WordSplitter) -> Self {
        self.word_splitter = splitter;
        self
    }
}

pub(crate) fn url_preserving_wrap_options<'a>(opts: RtOptions<'a>) -> RtOptions<'a> {
    opts.word_separator(textwrap::WordSeparator::AsciiSpace)
        .word_splitter(textwrap::WordSplitter::NoHyphenation)
        .break_words(false)
}
