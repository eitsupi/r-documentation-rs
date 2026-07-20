use std::{fmt, ops::Range};

use rd_ast::RdDocument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePosition {
    line: u32,
    column: u32,
}
impl SourcePosition {
    pub fn line(&self) -> u32 {
        self.line
    }
    pub fn column(&self) -> u32 {
        self.column
    }
}
impl SourcePosition {
    pub(crate) fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    bytes: Range<usize>,
    start: SourcePosition,
    end: SourcePosition,
}
impl SourceSpan {
    pub(crate) fn new(bytes: Range<usize>, start: SourcePosition, end: SourcePosition) -> Self {
        Self { bytes, start, end }
    }
}
impl SourceSpan {
    pub fn bytes(&self) -> Range<usize> {
        self.bytes.clone()
    }
    pub fn start(&self) -> &SourcePosition {
        &self.start
    }
    pub fn end(&self) -> &SourcePosition {
        &self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    document: RdDocument,
    diagnostics: Vec<Diagnostic>,
}
impl Parsed {
    pub(crate) fn new(document: RdDocument, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            document,
            diagnostics,
        }
    }
    pub fn document(&self) -> &RdDocument {
        &self.document
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn into_parts(self) -> (RdDocument, Vec<Diagnostic>) {
        (self.document, self.diagnostics)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    span: SourceSpan,
}
impl Diagnostic {
    pub(crate) fn new(
        severity: Severity,
        code: DiagnosticCode,
        message: impl Into<String>,
        span: SourceSpan,
    ) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            span,
        }
    }
    pub fn severity(&self) -> &Severity {
        &self.severity
    }
    pub fn code(&self) -> &DiagnosticCode {
        &self.code
    }
    pub fn message(&self) -> &str {
        &self.message
    }
    pub fn span(&self) -> &SourceSpan {
        &self.span
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticCode {
    UnexpectedClosingDelimiter,
    UnexpectedOpeningDelimiter,
    UnclosedGroup,
    UnclosedOption,
    MissingArgument,
    UnknownTag,
    TagNotAllowedHere,
    InvalidOptionSyntax,
    InvalidArity,
    UnexpectedToken,
    UnexpectedEndIf,
    MissingEndIf,
    UnexpectedConditional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseError {
    InvalidUtf8 {
        valid_up_to: usize,
        error_len: Option<usize>,
    },
    NulByte {
        offset: usize,
    },
    UnsupportedEncoding {
        name: String,
        span: Option<SourceSpan>,
    },
    InputTooLarge,
    NestingLimitExceeded {
        span: SourceSpan,
    },
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 {
                valid_up_to,
                error_len,
            } => write!(f, "invalid UTF-8 at {valid_up_to} (length {error_len:?})"),
            Self::NulByte { offset } => write!(f, "embedded NUL byte at {offset}"),
            Self::UnsupportedEncoding { name, .. } => write!(f, "unsupported encoding: {name}"),
            Self::InputTooLarge => f.write_str("input is too large"),
            Self::NestingLimitExceeded { .. } => f.write_str("nesting limit exceeded"),
        }
    }
}
impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hard_input_errors_report_utf8_and_nul() {
        assert_eq!(
            crate::parse(b"a\0b"),
            Err(ParseError::NulByte { offset: 1 })
        );
        assert_eq!(
            crate::parse(&[b'a', 0xe2, 0x82]),
            Err(ParseError::InvalidUtf8 {
                valid_up_to: 1,
                error_len: None
            })
        );
        assert_eq!(
            crate::parse(&[0xff]),
            Err(ParseError::InvalidUtf8 {
                valid_up_to: 0,
                error_len: Some(1)
            })
        );
    }
    #[test]
    fn empty_and_whitespace_documents_are_preserved() {
        assert!(crate::parse(b"").unwrap().document().nodes().is_empty());
        let document = crate::parse(b" \t\n").unwrap().into_parts().0;
        assert_eq!(document.nodes(), &[rd_ast::RdNode::Text(" \t\n".into())]);
    }
}
