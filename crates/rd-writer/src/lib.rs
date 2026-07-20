//! Serialize canonical Rd documents to source text.
//!
//! A successful write guarantees that parsing the returned source with
//! `rd-source` produces a diagnostic-free document equal to the input.
//! Formatting and byte identity are not guaranteed. The writer is strict:
//! documents with raw nodes, malformed shapes, or content without a faithful
//! Rd representation return a hard error.

mod error;
mod escape;
mod options;
mod spec;
mod writer;

pub use error::{UnserializableKind, WriteError};
pub use options::{LineEnding, WriterOptions};
use rd_ast::RdDocument;
pub use writer::Writer;

/// Serialize a document using default LF options.
pub fn write_document(document: &RdDocument) -> Result<String, WriteError> {
    Writer::new(WriterOptions::default()).write_document(document)
}

/// Serialize a complete document before writing it to `sink`.
pub fn write_document_to<W: std::io::Write>(
    document: &RdDocument,
    sink: &mut W,
) -> Result<(), WriteError> {
    Writer::new(WriterOptions::default()).write_document_to(document, sink)
}
