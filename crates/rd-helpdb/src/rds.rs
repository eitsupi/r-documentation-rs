//! Standalone `.rds` file reading and `.rdb` record decoding.
//!
//! Both entry points ultimately hand a decompressed byte buffer to
//! `rd_rds::parse`; the difference is the compression envelope:
//!
//! - A standalone `.rds` file (`aliases.rds`, `Meta/hsearch.rds`, `<pkg>.rdx`)
//!   is decoded by [`rd_rds::file`], which handles the bounded `"X\n"`, gzip,
//!   xz, bzip2, and zstd envelope layer.
//! - A `.rdb` record is framed as a 4-byte big-endian uncompressed-size
//!   prefix followed by a raw zlib deflate stream (no gzip wrapper).

use std::path::Path;

use crate::Error;

/// Reads a standalone (possibly compressed) `.rds` file into an
/// [`rd_rds::RObject`].
pub fn read_rds_file(path: impl AsRef<Path>) -> Result<rd_rds::RObject, Error> {
    let path = path.as_ref();
    rd_rds::file::read(path).map_err(|error| match error {
        rd_rds::file::ReadError::Io { path, source } => Error::io(path, source),
        rd_rds::file::ReadError::UnknownEnvelope { magic } => Error::UnsupportedCompression {
            path: path.to_path_buf(),
            magic,
        },
        rd_rds::file::ReadError::Decode(error) => Error::Rds(error),
        error => Error::RdsFile(error),
    })
}

/// Decodes a single `.rdb` record: `bytes` is the exact `(offset, length)`
/// slice a `.rdx` index entry points at, i.e. a 4-byte big-endian
/// uncompressed-size prefix followed by a raw zlib deflate stream. The
/// bounded `rd-rds::lazyload` record decoder checks the decompressed size,
/// input size, stream completeness, and trailing bytes before parsing.
pub fn decode_rdb_record(bytes: &[u8]) -> Result<rd_rds::RObject, Error> {
    let decompressed = rd_rds::lazyload::decode_stored_record(
        bytes,
        rd_rds::lazyload::Compression::Zlib,
        rd_rds::lazyload::Options::default(),
    )
    .map_err(|error| map_record_decode_error(error, bytes.len()))?;
    Ok(rd_rds::parse(&decompressed)?)
}

fn map_record_decode_error(error: rd_rds::lazyload::Error, stored_len: usize) -> Error {
    match error {
        rd_rds::lazyload::Error::StoredRecordSizeLimitExceeded { limit } => {
            Error::StoredRecordSizeLimitExceeded { limit }
        }
        rd_rds::lazyload::Error::DecompressedRecordSizeLimitExceeded { limit } => {
            Error::DecompressedRecordSizeLimitExceeded { limit }
        }
        rd_rds::lazyload::Error::RecordLengthPrefixMissing => Error::RecordSizeMismatch {
            expected: 4,
            actual: stored_len,
        },
        rd_rds::lazyload::Error::RecordSizeMismatch { declared, actual } => {
            Error::RecordSizeMismatch {
                expected: declared,
                actual,
            }
        }
        rd_rds::lazyload::Error::CompressionUnsupported { compression } => {
            Error::UnsupportedRecordCompression { compression }
        }
        other => {
            Error::MalformedIndex(format!("zlib decompression of .rdb record failed: {other}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_unknown_envelope_to_existing_error() {
        let path = std::env::temp_dir().join(format!(
            "rd-helpdb-unknown-envelope-{}.rds",
            std::process::id()
        ));
        std::fs::write(&path, b"A\nunknown").expect("write unknown envelope");
        let err = read_rds_file(&path).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert!(matches!(
            err,
            Error::UnsupportedCompression { magic, .. } if magic == b"A\nunkn"
        ));
    }

    #[test]
    fn rejects_short_records() {
        let err = decode_rdb_record(&[0, 1, 2]).unwrap_err();
        assert!(matches!(
            err,
            Error::RecordSizeMismatch {
                expected: 4,
                actual: 3
            }
        ));
    }

    #[test]
    fn compatibility_record_decoder_rejects_trailing_bytes() {
        let mut bytes = include_bytes!("../tests/fixtures/data/rd_minimal_v3.rdbentry").to_vec();
        bytes.push(0);
        let err = decode_rdb_record(&bytes).unwrap_err();
        assert!(matches!(err, Error::MalformedIndex(message) if message.contains("trailing")));
    }

    #[test]
    fn compatibility_record_decoder_maps_declared_size_mismatch() {
        let mut bytes = include_bytes!("../tests/fixtures/data/rd_minimal_v3.rdbentry").to_vec();
        bytes[3] = bytes[3].wrapping_add(1);
        let err = decode_rdb_record(&bytes).unwrap_err();
        assert!(matches!(err, Error::RecordSizeMismatch { .. }));
    }
}
