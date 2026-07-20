use thiserror::Error;

use crate::SexpKind;

/// Decode errors produced by `rd-rds`.
///
/// New failure categories may be added in minor releases; consumers should
/// match this enum non-exhaustively with a wildcard arm.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    #[error(
        "unexpected end of input at byte offset {offset} while reading {needed} bytes ({remaining} remaining)"
    )]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },

    #[error("unsupported RDS format marker bytes {marker:?} at byte offset {offset}")]
    UnsupportedMarker { marker: [u8; 2], offset: usize },

    #[error("unsupported RDS serialization format version {version} at byte offset {offset}")]
    UnsupportedVersion { version: u32, offset: usize },

    #[error("native encoding name is not valid UTF-8 at byte offset {offset}")]
    InvalidUtf8 { offset: usize },

    #[error("invalid SEXP type tag {tag:#x} at byte offset {offset}")]
    InvalidSexpType { tag: u32, offset: usize },

    #[error(
        "REFSXP index {index} out of range (reference table has {len} entries) at byte offset {offset}"
    )]
    RefIndexOutOfRange {
        index: u32,
        len: usize,
        offset: usize,
    },

    #[error("unsupported SEXP kind {kind:?} with type code {type_code} at byte offset {offset}")]
    UnsupportedSexp {
        kind: SexpKind,
        type_code: u8,
        offset: usize,
    },

    #[error("recursion depth limit ({limit}) exceeded while decoding")]
    DepthLimitExceeded { limit: u32 },

    #[error("vector length {length} exceeds limit {limit} at byte offset {offset}")]
    VectorLengthLimitExceeded {
        limit: usize,
        length: usize,
        offset: usize,
    },

    #[error("decoded element total {total} exceeds limit {limit} at byte offset {offset}")]
    TotalElementsLimitExceeded {
        limit: usize,
        total: usize,
        offset: usize,
    },

    #[error("string bytes are not valid for the recorded encoding")]
    InvalidStringEncoding,

    #[error(
        "PERSISTSXP payload declares an unsupported long-vector length ({len}) at byte offset {offset}"
    )]
    PersistedLongVectorUnsupported { len: u64, offset: usize },

    #[error("vector declares an unsupported long-vector length ({len}) at byte offset {offset}")]
    LongVectorUnsupported { len: u64, offset: usize },

    #[error("negative vector length {len} at byte offset {offset}")]
    NegativeLength { len: i32, offset: usize },

    #[error("attribute pairlist tag is not a symbol at byte offset {offset}")]
    InvalidAttributeTag { offset: usize },

    #[error("SYMSXP print name is not a concrete text string")]
    InvalidSymbolName,
}
