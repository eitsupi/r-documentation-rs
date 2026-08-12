use std::path::PathBuf;

#[cfg(feature = "gzip")]
use rd_rds::Limits;
use rd_rds::file;
#[cfg(all(feature = "gzip", feature = "xz", feature = "bzip2", feature = "zstd"))]
use rd_rds::{RStr, RValue};

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/envelope")
        .join(name);
    std::fs::read(path).expect("envelope fixture")
}

#[cfg(all(feature = "gzip", feature = "xz", feature = "bzip2", feature = "zstd"))]
#[test]
fn all_supported_envelopes_decode_to_the_same_matrix() {
    let names = [
        "packages-small-uncompressed.rds",
        "packages-small-gzip.rds",
        "packages-small-xz.rds",
        "packages-small-bzip2.rds",
        "packages-small-zstd.rds",
    ];
    let values: Vec<_> = names
        .iter()
        .map(|name| file::from_bytes(&fixture(name)).expect("decode envelope"))
        .collect();
    assert!(values.windows(2).all(|pair| pair[0] == pair[1]));

    let matrix = &values[0];
    let RValue::Character(cells) = &matrix.value() else {
        panic!("expected character matrix")
    };
    assert_eq!(cells.len(), 18);
    assert!(matches!(cells[2], RStr::Na));
    assert_eq!(cells[11].as_str().unwrap().unwrap().as_ref(), "NA");

    let RValue::Integer(dim) = &matrix.attributes().get("dim").unwrap().value() else {
        panic!("expected integer dim attribute")
    };
    assert_eq!(dim, &[Some(3), Some(6)]);
    let dimnames = matrix.attributes().get("dimnames").unwrap();
    let RValue::List(names) = &dimnames.value() else {
        panic!("expected dimnames list")
    };
    assert_eq!(names.len(), 2);
    let RValue::Character(row_names) = &names[0].value() else {
        panic!("expected row names")
    };
    let row_names: Vec<_> = row_names
        .iter()
        .map(|value| value.as_str().unwrap().unwrap().into_owned())
        .collect();
    assert_eq!(row_names, ["base", "stats", "tools"]);
    let RValue::Character(column_names) = &names[1].value() else {
        panic!("expected column names")
    };
    assert_eq!(column_names.len(), 6);
    let column_names: Vec<_> = column_names
        .iter()
        .map(|value| value.as_str().unwrap().unwrap().into_owned())
        .collect();
    assert_eq!(
        column_names,
        [
            "Package",
            "Version",
            "Depends",
            "Imports",
            "DownloadURL",
            "Filesize"
        ]
    );
}

#[test]
fn envelope_sniffing_is_strict() {
    assert!(file::is_xdr_stream(&fixture(
        "packages-small-uncompressed.rds"
    )));
    assert!(!file::is_xdr_stream(&fixture("packages-small-gzip.rds")));
    assert!(!file::is_xdr_stream(&fixture("packages-small-xz.rds")));
    assert!(!file::is_xdr_stream(&fixture("packages-small-bzip2.rds")));
    assert!(!file::is_xdr_stream(&fixture("packages-small-zstd.rds")));
    assert!(!file::is_xdr_stream(b"garbage"));

    let error = file::from_bytes(b"A\nhello").unwrap_err();
    assert!(matches!(error, file::ReadError::UnknownEnvelope { magic } if magic == b"A\nhell"));
}

#[test]
fn disk_reads_enforce_compressed_size_limit_before_decoding() {
    let source = fixture("packages-small-uncompressed.rds");
    let path = std::env::temp_dir().join(format!(
        "rd-rds-file-envelope-{}-{}.rds",
        std::process::id(),
        source.len()
    ));
    std::fs::write(&path, &source).expect("write temporary RDS file");

    let error = file::read_with_options(
        &path,
        &file::ReadOptions::default().max_compressed_bytes(source.len() - 1),
    )
    .expect_err("oversized disk input should be rejected");
    assert!(matches!(
        error,
        file::ReadError::CompressedSizeLimitExceeded { limit }
            if limit == source.len() - 1
    ));

    let value = file::read_with_options(
        &path,
        &file::ReadOptions::default().max_compressed_bytes(source.len()),
    )
    .expect("input within the disk cap should decode");
    assert_eq!(value, file::from_bytes(&source).expect("source decode"));

    std::fs::remove_file(path).expect("remove temporary RDS file");
}

#[cfg(feature = "gzip")]
#[test]
fn file_and_decoder_caps_are_enforced() {
    let gzip = fixture("packages-small-gzip.rds");
    assert!(matches!(
        file::from_bytes_with_options(&gzip, &file::ReadOptions::default().max_compressed_bytes(1)),
        Err(file::ReadError::CompressedSizeLimitExceeded { limit: 1 })
    ));
    assert!(matches!(
        file::from_bytes_with_options(
            &gzip,
            &file::ReadOptions::default().max_decompressed_bytes(1)
        ),
        Err(file::ReadError::DecompressedSizeLimitExceeded { limit: 1 })
    ));
    let uncompressed = fixture("packages-small-uncompressed.rds");
    assert!(matches!(
        file::from_bytes_with_options(
            &uncompressed,
            &file::ReadOptions::default().limits(Limits::default().max_vector_len(1))
        ),
        Err(file::ReadError::Decode(
            rd_rds::Error::VectorLengthLimitExceeded { limit: 1, .. }
        ))
    ));
    assert!(matches!(
        file::from_bytes_with_options(
            &uncompressed,
            &file::ReadOptions::default().limits(Limits::default().max_total_elements(1))
        ),
        Err(file::ReadError::Decode(
            rd_rds::Error::TotalElementsLimitExceeded { limit: 1, .. }
        ))
    ));
}

#[cfg(not(feature = "gzip"))]
#[test]
fn gzip_requires_feature() {
    assert!(matches!(
        file::from_bytes(&fixture("packages-small-gzip.rds")),
        Err(file::ReadError::CompressionDisabled {
            format: file::Compression::Gzip
        })
    ));
}

#[cfg(not(feature = "xz"))]
#[test]
fn xz_requires_feature() {
    assert!(matches!(
        file::from_bytes(&fixture("packages-small-xz.rds")),
        Err(file::ReadError::CompressionDisabled {
            format: file::Compression::Xz
        })
    ));
}

#[cfg(not(feature = "bzip2"))]
#[test]
fn bzip2_requires_feature() {
    assert!(matches!(
        file::from_bytes(&fixture("packages-small-bzip2.rds")),
        Err(file::ReadError::CompressionDisabled {
            format: file::Compression::Bzip2
        })
    ));
}

#[cfg(not(feature = "zstd"))]
#[test]
fn zstd_requires_feature() {
    assert!(matches!(
        file::from_bytes(&fixture("packages-small-zstd.rds")),
        Err(file::ReadError::CompressionDisabled {
            format: file::Compression::Zstd
        })
    ));
}

#[cfg(feature = "zstd")]
#[test]
fn corrupted_zstd_payload_reports_decompression_error() {
    let mut corrupted = vec![0x28, 0xb5, 0x2f, 0xfd];
    corrupted.extend_from_slice(b"garbage");
    assert!(matches!(
        file::from_bytes(&corrupted),
        Err(file::ReadError::Decompression {
            format: file::Compression::Zstd,
            ..
        })
    ));
}

#[cfg(feature = "xz")]
#[test]
fn truncated_xz_stream_reports_decompression_error() {
    let source = fixture("packages-small-xz.rds");
    let truncated = &source[..source.len() - 1];

    assert!(matches!(
        file::from_bytes(truncated),
        Err(file::ReadError::Decompression {
            format: file::Compression::Xz,
            ..
        })
    ));
}

#[cfg(feature = "xz")]
#[test]
fn corrupted_xz_payload_reports_decompression_error() {
    let mut corrupted = fixture("packages-small-xz.rds");
    let payload_offset = corrupted.len() / 2;
    corrupted[payload_offset] ^= 0x01;

    assert!(matches!(
        file::from_bytes(&corrupted),
        Err(file::ReadError::Decompression {
            format: file::Compression::Xz,
            ..
        })
    ));
}

#[cfg(feature = "xz")]
#[test]
fn corrupted_xz_header_reports_decompression_error() {
    let mut corrupted = fixture("packages-small-xz.rds");
    corrupted[6] ^= 0x01;

    assert!(matches!(
        file::from_bytes(&corrupted),
        Err(file::ReadError::Decompression {
            format: file::Compression::Xz,
            ..
        })
    ));
}

#[cfg(feature = "xz")]
#[test]
fn xz_decompression_limit_is_enforced() {
    let source = fixture("packages-small-xz.rds");

    assert!(matches!(
        file::from_bytes_with_options(
            &source,
            &file::ReadOptions::default().max_decompressed_bytes(1),
        ),
        Err(file::ReadError::DecompressedSizeLimitExceeded { limit: 1 })
    ));
}
