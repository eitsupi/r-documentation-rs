use std::{io::Read, path::PathBuf};

use flate2::read::GzDecoder;
use rd_rds::{package::PackagesMatrix, parse};

fn fixture(directory: &str, name: &str) -> Vec<u8> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(directory)
        .join(name)
        .pipe(|path| std::fs::read(path).expect("fixture"))
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}
impl<T> Pipe for T {}

#[cfg(all(feature = "gzip", feature = "xz"))]
#[test]
fn package_envelopes_decode_to_equal_typed_matrices() {
    let gzip =
        rd_rds::file::from_bytes(&fixture("packages", "packages-cran-gzip.rds")).expect("gzip");
    let xz = rd_rds::file::from_bytes(&fixture("packages", "packages-cran-xz.rds")).expect("xz");
    let gzip = PackagesMatrix::from_object(&gzip).expect("typed gzip");
    let xz = PackagesMatrix::from_object(&xz).expect("typed xz");
    assert_eq!(gzip, xz);
    assert_eq!(gzip.len(), 4);
    assert_eq!(gzip.rows().len(), 4);
    assert_eq!(gzip.column_names().len(), 11);
    for name in [
        "Package",
        "Version",
        "Depends",
        "Imports",
        "DownloadURL",
        "Filesize",
    ] {
        assert!(gzip.column(name).is_some(), "column {name}");
    }
    let packages: Vec<_> = gzip
        .rows()
        .map(|row| (row.get("Package"), row.get("Version")))
        .collect();
    assert_eq!(
        packages,
        vec![
            (Some(Some("cli")), Some(Some("4.6.1"))),
            (Some(Some("curl")), Some(Some("4.3.2"))),
            (Some(Some("data.table")), Some(Some("1.17.8"))),
            (Some(Some("examplepkg")), Some(Some("0.1.0")))
        ]
    );
    assert!(gzip.column("missing").is_none());
    assert_eq!(gzip.row(0).unwrap().get("missing"), None);
    assert_eq!(
        gzip.column("Depends").unwrap().get(2),
        Some(Some("R (>= 3.1.0)"))
    );
    assert_eq!(gzip.column("Depends").unwrap().get(3), Some(None));
    assert_eq!(gzip.row(3).unwrap().get("Imports"), Some(Some("NA")));
    assert_eq!(gzip.column("Package").unwrap().len(), gzip.len());
    assert!(!gzip.column("Package").unwrap().is_empty());
}

#[test]
fn typed_view_accepts_decompressed_stream_without_file_layer() {
    let compressed = fixture("packages", "packages-cran-gzip.rds");
    let mut decoder = GzDecoder::new(&compressed[..]);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).expect("decompress");
    let object = parse(&bytes).expect("parse XDR");
    let matrix = PackagesMatrix::from_object(&object).expect("typed matrix");
    assert_eq!(matrix.row(0).unwrap().get("Package"), Some(Some("cli")));
}
