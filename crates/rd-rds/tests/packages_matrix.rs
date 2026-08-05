use std::{io::Read, path::PathBuf};

use flate2::read::GzDecoder;
#[cfg(feature = "gzip")]
use rd_rds::REncoding;
use rd_rds::{RValue, package::PackagesMatrix, parse};

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

#[cfg(feature = "xz")]
#[test]
fn cran_v2_xz_matrix_preserves_schema_and_cells() {
    let object = rd_rds::file::from_bytes(&fixture("packages", "packages-cran-v2-xz.rds"))
        .expect("CRAN v2 xz");
    let matrix = PackagesMatrix::from_object(&object).expect("typed CRAN v2 matrix");
    let columns = [
        "Package",
        "Version",
        "Priority",
        "Depends",
        "Imports",
        "LinkingTo",
        "Suggests",
        "Enhances",
        "License",
        "License_is_FOSS",
        "License_restricts_use",
        "OS_type",
        "Archs",
        "MD5sum",
        "NeedsCompilation",
        "Path",
        "Published",
    ];
    assert_eq!(matrix.column_names().collect::<Vec<_>>(), columns);
    assert_eq!(matrix.len(), 4);
    assert_eq!(matrix.rows().len(), 4);
    assert_eq!(matrix.column("Package").unwrap().len(), 4);
    assert_eq!(
        matrix.column("Depends").unwrap().get(0),
        Some(Some("R (>= 4.0)"))
    );
    assert_eq!(
        matrix.row(0).unwrap().get("Depends"),
        Some(Some("R (>= 4.0)"))
    );
    assert_eq!(matrix.column("Depends").unwrap().get(1), Some(None));
    assert_eq!(matrix.row(2).unwrap().get("Priority"), Some(None));
    assert_eq!(matrix.row(3).unwrap().get("Priority"), Some(Some("NA")));
    assert_eq!(
        matrix.row(1).unwrap().get("Suggests"),
        Some(Some(
            r"knitr,
  rmarkdown"
        ))
    );
    assert_eq!(
        matrix
            .rows()
            .map(|row| row.get("Package"))
            .collect::<Vec<_>>(),
        vec![
            Some(Some("alpha")),
            Some(Some("beta")),
            Some(Some("gamma")),
            Some(Some("delta")),
        ]
    );
}

#[cfg(feature = "gzip")]
#[test]
fn archive_v3_gzip_matrix_preserves_schema_order_and_utf8() {
    let object = rd_rds::file::from_bytes(&fixture("packages", "packages-archive-v3-gzip.rds"))
        .expect("archive v3 gzip");

    let RValue::List(dimnames) = object.attributes().get("dimnames").unwrap().value() else {
        panic!("expected dimnames list")
    };
    // Real CRAN archive indexes have NULL row names, so this exercises the accepted path.
    assert!(matches!(dimnames[0].value(), RValue::Null));
    let matrix = PackagesMatrix::from_object(&object).expect("typed archive matrix");
    let columns = [
        "Package",
        "Version",
        "Priority",
        "Depends",
        "Imports",
        "LinkingTo",
        "Suggests",
        "Enhances",
        "License",
        "License_is_FOSS",
        "License_restricts_use",
        "OS_type",
        "Archs",
        "MD5sum",
        "NeedsCompilation",
    ];
    assert_eq!(matrix.column_names().collect::<Vec<_>>(), columns);
    assert_eq!(matrix.len(), 3);
    assert_eq!(matrix.row(0).unwrap().get("Version"), Some(Some("1.10.0")));
    assert_eq!(
        matrix.column("Version").unwrap().get(0),
        Some(Some("1.10.0"))
    );
    assert_eq!(matrix.row(1).unwrap().get("Version"), Some(Some("1.2.0")));
    assert_eq!(matrix.row(2).unwrap().get("Version"), Some(Some("1.2.1")));
    assert_eq!(matrix.row(0).unwrap().get("Suggests"), Some(Some("José")));
    assert_eq!(matrix.row(1).unwrap().get("Suggests"), Some(Some("NA")));
    assert_eq!(matrix.row(1).unwrap().get("LinkingTo"), Some(None));
    assert_eq!(
        matrix.row(1).unwrap().get("Enhances"),
        Some(Some(
            r"helper,
  another"
        ))
    );

    let RValue::Character(values) = object.value() else {
        panic!("expected character matrix")
    };
    let utf8_cell = &values[6 * 3];
    assert_eq!(utf8_cell.as_str().unwrap().unwrap().as_ref(), "José");
    assert_eq!(utf8_cell.encoding(), Some(REncoding::Utf8));
    assert_eq!(
        values
            .iter()
            .filter(|value| value.encoding() == Some(REncoding::Utf8))
            .count(),
        1
    );
}

#[test]
fn na_string_row_name_is_decoded_from_handwritten_format_v2_stream() {
    // XDR header: marker, format 2, writer R 4.6.1, minimum reader R 3.5.0.
    // Root character vector with dim and dimnames attributes.
    let bytes = [
        b'X', b'\n', 0, 0, 0, 2, 0, 4, 6, 1, 0, 3, 5, 0,
        // STRSXP root with attributes, followed by two cells: "value" and NA_STRING.
        0, 0, 2, 16, 0, 0, 0, 2, 0, 0, 0, 9, 0, 0, 0, 5, b'v', b'a', b'l', b'u', b'e', 0, 0, 0, 9,
        0xff, 0xff, 0xff, 0xff, // Attribute pairlist: dim = c(2, 1).
        0, 0, 4, 2, 0, 0, 0, 1, 0, 0, 0, 9, 0, 0, 0, 3, b'd', b'i', b'm', 0, 0, 0, 13, 0, 0, 0, 2,
        0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 4, 2,
        // Attribute pairlist: dimnames = list(c(NA, "row2"), "column").
        0, 0, 0, 1, 0, 0, 0, 9, 0, 0, 0, 8, b'd', b'i', b'm', b'n', b'a', b'm', b'e', b's', 0, 0, 0,
        19, 0, 0, 0, 2, 0, 0, 0, 16, 0, 0, 0, 2, 0, 0, 0, 9, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 9, 0,
        0, 0, 4, b'r', b'o', b'w', b'2', 0, 0, 0, 16, 0, 0, 0, 1, 0, 0, 0, 9, 0, 0, 0, 6, b'c',
        b'o', b'l', b'u', b'm', b'n',
        // End of dimnames pairlist and end of root attributes.
        0, 0, 0, 0,
    ];
    let object = parse(&bytes).expect("handwritten format v2 matrix");
    let RValue::List(dimnames) = object.attributes().get("dimnames").unwrap().value() else {
        panic!("expected dimnames list")
    };
    let RValue::Character(row_names) = dimnames[0].value() else {
        panic!("expected character row names")
    };
    assert!(matches!(row_names[0], rd_rds::RStr::Na));
    let matrix = rd_rds::matrix::CharacterMatrix::from_object(&object).expect("character matrix");
    assert_eq!(matrix.nrow(), 2);
    assert_eq!(matrix.ncol(), 1);
    assert_eq!(matrix.get(0, 0), Some(Some("value")));
    assert_eq!(matrix.get(1, 0), Some(None));
    assert_eq!(matrix.row_name(0), None);
    assert_eq!(matrix.row_name(1), Some("row2"));
    assert_eq!(matrix.column_name(0), Some("column"));
}
