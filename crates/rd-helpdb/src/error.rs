use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by `rd-helpdb`.
///
/// New failure categories may be added in minor releases; consumers should
/// match this enum non-exhaustively with a wildcard arm.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("unsupported compression magic bytes {magic:02x?} in {path}")]
    UnsupportedCompression { path: PathBuf, magic: Vec<u8> },

    #[error(transparent)]
    Rds(#[from] rd_rds::Error),

    #[error(transparent)]
    RdsFile(#[from] rd_rds::file::ReadError),

    #[error("malformed help-db index: {0}")]
    MalformedIndex(String),

    #[error("unknown topic {topic:?}")]
    UnknownTopic { topic: String },

    #[error("unknown reference key {key:?}")]
    UnknownReference { key: String },

    #[error(
        "record size mismatch: the 4-byte length prefix declares {expected} bytes but zlib decompression produced {actual} bytes"
    )]
    RecordSizeMismatch { expected: usize, actual: usize },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
