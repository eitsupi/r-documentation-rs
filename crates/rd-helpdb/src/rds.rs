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

use std::{io::Read, path::Path};

use flate2::read::ZlibDecoder;

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
/// decompressed size is checked against the prefix before parsing.
pub fn decode_rdb_record(bytes: &[u8]) -> Result<rd_rds::RObject, Error> {
    if bytes.len() < 4 {
        // Too short to even hold the 4-byte length prefix.
        return Err(Error::RecordSizeMismatch {
            expected: 4,
            actual: bytes.len(),
        });
    }
    let (prefix, payload) = bytes.split_at(4);
    let declared_len =
        u32::from_be_bytes(prefix.try_into().expect("split_at(4) yields 4 bytes")) as usize;

    let mut decoder = ZlibDecoder::new(payload);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|err| {
        Error::MalformedIndex(format!("zlib decompression of .rdb record failed: {err}"))
    })?;
    if decompressed.len() != declared_len {
        return Err(Error::RecordSizeMismatch {
            expected: declared_len,
            actual: decompressed.len(),
        });
    }

    Ok(rd_rds::parse(&decompressed)?)
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
}
