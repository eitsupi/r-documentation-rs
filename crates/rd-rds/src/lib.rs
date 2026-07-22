//! Read-only reader for the subset of R's RDS serialization format used by
//! installed-package help databases.
//!
//! [`parse`] reads a decompressed XDR serialization stream. [`mod@file`] provides
//! the bounded standalone-file entry layer, including supported compression
//! envelopes.
//!
//! This crate is scoped to installed-R-package information. Unknown SEXP
//! values are hard errors, never silently accepted. `RObject`/`RValue` are a
//! supported advanced API with encapsulated fields and non-exhaustive enums;
//! consumers must include wildcard match arms. The typed [`package`] views and
//! general [`matrix::CharacterMatrix`] view are the stable convenience surface.
//! `parse` accepts only decompressed XDR,
//! while [`file::from_bytes`] accepts raw XDR, gzip, xz, bzip2, and zstd
//! envelopes when enabled. Default resource limits are depth 5,000, vectors
//! 8,000,000 elements, 16,000,000 total elements, and 256 MiB file inputs.

mod cursor;
mod decode;
mod error;
pub mod file;
mod header;
pub mod matrix;
pub mod package;
mod value;

pub use cursor::ByteCursor;
pub use decode::{parse, parse_with_limits};
pub use error::Error;
pub use header::{Header, RVersion};
pub use value::{
    Attribute, Attributes, EnvHandle, Limits, Persisted, REncoding, RObject, RStr, RValue,
    SexpKind, Symbol,
};
