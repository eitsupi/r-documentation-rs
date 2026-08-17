//! Bounded standalone RDS file reading.
//!
//! This module accepts an entire standalone RDS file and recognizes only the
//! `X\n` XDR marker, gzip (`1f 8b`), xz (`fd 37 7a 58 5a 00`), bzip2
//! (`BZh`), and zstd (`28 b5 2f fd`) envelopes. Compression support is
//! controlled by the corresponding crate features. Input and decompressed data
//! are capped at 256 MiB by default; [`ReadOptions`] can lower or raise those
//! caps and configure
//! decoder [`Limits`]. This is distinct from [`crate::parse`], which accepts
//! only an already decompressed XDR stream.

use std::{
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{Limits, NativeEncodingPolicy, ParseOptions, RObject};

const XDR_MARKER: &[u8; 2] = b"X\n";
const ZSTD_MAGIC: &[u8; 4] = &[0x28, 0xb5, 0x2f, 0xfd];
const DEFAULT_CAP: usize = 256 * 1024 * 1024;

/// Options controlling standalone-file size limits and RDS decoding limits.
#[derive(Debug, Clone, Copy)]
pub struct ReadOptions {
    parse_options: ParseOptions,
    max_compressed_bytes: usize,
    max_decompressed_bytes: usize,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            parse_options: ParseOptions::default(),
            max_compressed_bytes: DEFAULT_CAP,
            max_decompressed_bytes: DEFAULT_CAP,
        }
    }
}

impl ReadOptions {
    pub fn limits(mut self, limits: Limits) -> Self {
        self.parse_options = self.parse_options.limits(limits);
        self
    }

    /// Replaces the complete decompressed-stream parsing configuration.
    pub fn parse_options(mut self, options: ParseOptions) -> Self {
        self.parse_options = options;
        self
    }

    pub fn max_compressed_bytes(mut self, limit: usize) -> Self {
        self.max_compressed_bytes = limit;
        self
    }

    pub fn max_decompressed_bytes(mut self, limit: usize) -> Self {
        self.max_decompressed_bytes = limit;
        self
    }

    /// Selects the native-string policy used when the RDS header has no
    /// encoding, which means format 2; retained `RStr` values are validated
    /// when converted, while `SYMSXP` print names are converted during parsing.
    pub fn native_encoding_policy(mut self, policy: NativeEncodingPolicy) -> Self {
        self.parse_options = self.parse_options.native_encoding_policy(policy);
        self
    }
}

/// Compression envelopes recognized by standalone RDS reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Compression {
    Gzip,
    Xz,
    Bzip2,
    Zstd,
}

impl fmt::Display for Compression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Gzip => "gzip",
            Self::Xz => "xz",
            Self::Bzip2 => "bzip2",
            Self::Zstd => "zstd",
        })
    }
}

/// Errors from bounded standalone RDS file reading.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("unknown RDS envelope magic bytes {magic:02x?}")]
    UnknownEnvelope { magic: Vec<u8> },
    #[error("{format} support is disabled")]
    CompressionDisabled { format: Compression },
    #[error("{format} decompression failed: {source}")]
    Decompression {
        format: Compression,
        #[source]
        source: io::Error,
    },
    #[error("compressed input exceeds the {limit}-byte limit")]
    CompressedSizeLimitExceeded { limit: usize },
    #[error("decompressed input exceeds the {limit}-byte limit")]
    DecompressedSizeLimitExceeded { limit: usize },
    #[error(transparent)]
    Decode(#[from] crate::Error),
}

/// Reads a standalone RDS file from disk.
pub fn read(path: impl AsRef<Path>) -> Result<RObject, ReadError> {
    read_with_options(path, &ReadOptions::default())
}

/// Reads a standalone RDS file from disk with explicit bounds.
pub fn read_with_options(
    path: impl AsRef<Path>,
    options: &ReadOptions,
) -> Result<RObject, ReadError> {
    let path = path.as_ref();
    let file = File::open(path).map_err(|source| ReadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.take(options.max_compressed_bytes.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| ReadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() > options.max_compressed_bytes {
        return Err(ReadError::CompressedSizeLimitExceeded {
            limit: options.max_compressed_bytes,
        });
    }
    from_bytes_with_options(&bytes, options)
}

/// Decodes a complete standalone RDS file held in memory.
pub fn from_bytes(bytes: &[u8]) -> Result<RObject, ReadError> {
    from_bytes_with_options(bytes, &ReadOptions::default())
}

/// Decodes a complete standalone RDS file held in memory with explicit bounds.
pub fn from_bytes_with_options(bytes: &[u8], options: &ReadOptions) -> Result<RObject, ReadError> {
    if bytes.len() > options.max_compressed_bytes {
        return Err(ReadError::CompressedSizeLimitExceeded {
            limit: options.max_compressed_bytes,
        });
    }

    let format = if is_xdr_stream(bytes) {
        None
    } else if bytes.starts_with(&[0x1f, 0x8b]) {
        Some(Compression::Gzip)
    } else if bytes.starts_with(&[0xfd, 0x37, 0x7a, 0x58, 0x5a, 0x00]) {
        Some(Compression::Xz)
    } else if bytes.starts_with(b"BZh") {
        Some(Compression::Bzip2)
    } else if bytes.starts_with(ZSTD_MAGIC) {
        Some(Compression::Zstd)
    } else {
        return Err(ReadError::UnknownEnvelope {
            magic: bytes[..bytes.len().min(6)].to_vec(),
        });
    };

    let decoded = match format {
        None => {
            if bytes.len() > options.max_decompressed_bytes {
                return Err(ReadError::DecompressedSizeLimitExceeded {
                    limit: options.max_decompressed_bytes,
                });
            }
            bytes.to_vec()
        }
        Some(format) => decompress(bytes, format, options.max_decompressed_bytes)?,
    };
    Ok(crate::parse_with_options(&decoded, options.parse_options)?)
}

/// Returns whether `bytes` begins with the decompressed XDR stream marker.
pub fn is_xdr_stream(bytes: &[u8]) -> bool {
    bytes.starts_with(XDR_MARKER)
}

fn decompress(bytes: &[u8], format: Compression, limit: usize) -> Result<Vec<u8>, ReadError> {
    let output = match format {
        Compression::Gzip => decompress_gzip(bytes, limit)?,
        Compression::Xz => decompress_xz(bytes, limit)?,
        Compression::Bzip2 => decompress_bzip2(bytes, limit)?,
        Compression::Zstd => decompress_zstd(bytes, limit)?,
    };
    if output.len() > limit {
        return Err(ReadError::DecompressedSizeLimitExceeded { limit });
    }
    Ok(output)
}

#[cfg(any(feature = "gzip", feature = "xz", feature = "bzip2", feature = "zstd"))]
fn read_limited<R: std::io::Read>(
    reader: R,
    limit: usize,
    format: Compression,
) -> Result<Vec<u8>, ReadError> {
    use std::io::Read as _;

    let mut output = Vec::new();
    reader
        .take(limit.saturating_add(1) as u64)
        .read_to_end(&mut output)
        .map_err(|source| ReadError::Decompression { format, source })?;
    Ok(output)
}

#[cfg(feature = "gzip")]
fn decompress_gzip(bytes: &[u8], limit: usize) -> Result<Vec<u8>, ReadError> {
    read_limited(
        flate2::read::GzDecoder::new(bytes),
        limit,
        Compression::Gzip,
    )
}

#[cfg(not(feature = "gzip"))]
fn decompress_gzip(_: &[u8], _: usize) -> Result<Vec<u8>, ReadError> {
    Err(ReadError::CompressionDisabled {
        format: Compression::Gzip,
    })
}

#[cfg(feature = "xz")]
fn decompress_xz(bytes: &[u8], limit: usize) -> Result<Vec<u8>, ReadError> {
    read_limited(
        lzma_rust2::XzReader::new(bytes, false),
        limit,
        Compression::Xz,
    )
}

#[cfg(not(feature = "xz"))]
fn decompress_xz(_: &[u8], _: usize) -> Result<Vec<u8>, ReadError> {
    Err(ReadError::CompressionDisabled {
        format: Compression::Xz,
    })
}

#[cfg(feature = "bzip2")]
fn decompress_bzip2(bytes: &[u8], limit: usize) -> Result<Vec<u8>, ReadError> {
    read_limited(
        bzip2::read::BzDecoder::new(bytes),
        limit,
        Compression::Bzip2,
    )
}

#[cfg(not(feature = "bzip2"))]
fn decompress_bzip2(_: &[u8], _: usize) -> Result<Vec<u8>, ReadError> {
    Err(ReadError::CompressionDisabled {
        format: Compression::Bzip2,
    })
}

#[cfg(feature = "zstd")]
fn decompress_zstd(bytes: &[u8], limit: usize) -> Result<Vec<u8>, ReadError> {
    let mut source = bytes;
    let decoder = ruzstd::decoding::StreamingDecoder::new(&mut source).map_err(|source| {
        ReadError::Decompression {
            format: Compression::Zstd,
            source: io::Error::new(io::ErrorKind::InvalidData, source.to_string()),
        }
    })?;
    read_limited(decoder, limit, Compression::Zstd)
}

#[cfg(not(feature = "zstd"))]
fn decompress_zstd(_: &[u8], _: usize) -> Result<Vec<u8>, ReadError> {
    Err(ReadError::CompressionDisabled {
        format: Compression::Zstd,
    })
}
