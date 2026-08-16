//! Read-only reader for installed-package metadata, help databases, and
//! selected CRAN-like repository indexes in R's RDS serialization format.
//!
//! [`parse`] reads a decompressed XDR serialization stream. [`mod@file`] provides
//! the bounded standalone-file entry layer, including supported compression
//! envelopes.
//!
//! See the crate README for the concrete repository-index profiles covered by
//! deterministic fixtures and verified manually against real files.
//!
//! This crate is scoped to installed-R-package information and selected
//! CRAN-like repository indexes. Unknown SEXP
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
pub use decode::{NativeEncodingPolicy, parse, parse_with_limits, parse_with_options};
pub use error::Error;
pub use header::{Header, RVersion};
pub use value::{
    Attribute, Attributes, EnvHandle, Limits, Persisted, REncoding, RObject, RStr, RValue,
    SexpKind, Symbol,
};
