//! Parse R documentation source into the common `rd_ast` document model.

mod diagnostic;
mod lexer;
mod parser;
mod source_map;

#[doc(hidden)]
pub mod unstable_rlike;

/// v1 implementation limit for the complete UTF-8 input, in bytes.
pub const MAX_INPUT_SIZE: usize = 64 * 1024 * 1024;

pub use diagnostic::{
    Diagnostic, DiagnosticCode, ParseError, Parsed, Severity, SourcePosition, SourceSpan,
};

/// Parse a UTF-8 Rd source document.
pub fn parse(input: &[u8]) -> Result<Parsed, ParseError> {
    if input.len() > MAX_INPUT_SIZE {
        return Err(ParseError::InputTooLarge);
    }
    if let Some(offset) = input.iter().position(|byte| *byte == 0) {
        return Err(ParseError::NulByte { offset });
    }
    let source = std::str::from_utf8(input).map_err(|error| ParseError::InvalidUtf8 {
        valid_up_to: error.valid_up_to(),
        error_len: error.error_len(),
    })?;
    parser::Parser::new(input, source).parse()
}
