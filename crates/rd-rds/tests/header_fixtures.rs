use std::{fs, io::Read, path::PathBuf};

use flate2::read::GzDecoder;
use rd_rds::{ByteCursor, Header};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}

fn serialization_bytes(bytes: &[u8]) -> Vec<u8> {
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        // serialize() with a refhook writes the bare XDR stream directly;
        // unlike saveRDS(), it has no compression envelope.
        return bytes.to_vec();
    }
    let mut decoder = GzDecoder::new(bytes);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .expect("fixture gzip stream");
    output
}

#[test]
fn parse_all_header_fixtures() {
    let mut fixtures = Vec::new();

    for entry in fs::read_dir(fixture_dir()).expect("fixture directory") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(file_name.ends_with("_v2.rds") || file_name.ends_with("_v3.rds")) {
            continue;
        }
        fixtures.push(path);
    }

    fixtures.sort();
    // Guard against a wrong fixture path making the loop below pass vacuously.
    assert!(!fixtures.is_empty(), "no *_v2.rds/*_v3.rds fixtures found");

    for path in fixtures {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("fixture file name");
        let expected_version = if name.ends_with("_v2.rds") { 2 } else { 3 };
        let bytes = fs::read(&path).expect("fixture bytes");
        let decompressed = serialization_bytes(&bytes);
        let mut cursor = ByteCursor::new(&decompressed);
        let header = Header::parse(&mut cursor).expect("header");

        assert_eq!(header.format_version, expected_version, "{name}");
        if expected_version == 3 {
            assert_eq!(header.native_encoding.as_deref(), Some("UTF-8"), "{name}");
        } else {
            assert!(header.native_encoding.is_none(), "{name}");
        }
    }
}
