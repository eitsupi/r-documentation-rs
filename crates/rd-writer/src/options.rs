//! Writer configuration.

/// The line ending emitted for canonical newlines.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style line feeds.
    #[default]
    Lf,
    /// Windows-style carriage-return/line-feed pairs.
    CrLf,
}

/// Options controlling serialization.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub struct WriterOptions {
    /// The line ending used for newlines present in emitted content.
    pub line_ending: LineEnding,
}

impl WriterOptions {
    /// Construct default options.
    pub const fn new() -> Self {
        Self {
            line_ending: LineEnding::Lf,
        }
    }

    /// Select the line ending used by the writer.
    pub const fn with_line_ending(mut self, line_ending: LineEnding) -> Self {
        self.line_ending = line_ending;
        self
    }
}
