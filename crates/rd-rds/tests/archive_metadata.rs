#[cfg(feature = "gzip")]
use std::path::PathBuf;

#[cfg(feature = "gzip")]
use rd_rds::RValue;

#[cfg(feature = "gzip")]
fn fixture() -> rd_rds::RObject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/archive_metadata/archive-metadata-v3-gzip.rds");
    rd_rds::file::read(path).expect("archive metadata fixture")
}

#[cfg(feature = "gzip")]
fn strings(value: &rd_rds::RObject) -> Vec<String> {
    let RValue::Character(values) = value.value() else {
        panic!("expected character vector")
    };
    values
        .iter()
        .map(|value| value.as_str().unwrap().unwrap().into_owned())
        .collect()
}

#[cfg(feature = "gzip")]
#[test]
fn reads_archive_metadata_through_generic_list_and_attributes() {
    let object = fixture();
    let RValue::List(packages) = object.value() else {
        panic!("expected named list")
    };
    assert_eq!(
        strings(object.attributes().get("names").unwrap()),
        ["alpha", "beta"]
    );
    assert_eq!(packages.len(), 2);

    let expected_columns = [
        "size", "isdir", "mode", "mtime", "ctime", "atime", "uid", "gid", "uname", "grname",
    ];
    for package in packages {
        assert!(
            strings(package.attributes().get("class").unwrap())
                .iter()
                .any(|class| class == "data.frame")
        );
        assert_eq!(
            strings(package.attributes().get("names").unwrap()),
            expected_columns
        );
    }

    let alpha = &packages[0];
    let RValue::List(alpha_columns) = alpha.value() else {
        panic!("expected alpha data frame columns")
    };
    let mode = &alpha_columns[2];
    assert_eq!(
        strings(mode.attributes().get("class").unwrap()),
        ["octmode"]
    );
    assert!(matches!(mode.value(), RValue::Integer(_)));
    let mtime = &alpha_columns[3];
    assert_eq!(
        strings(mtime.attributes().get("class").unwrap()),
        ["POSIXct", "POSIXt"]
    );
    assert!(matches!(mtime.value(), RValue::Real(_)));

    let row_names = alpha.attributes().get("row.names").unwrap();
    assert_eq!(
        strings(row_names),
        ["alpha/alpha_0.1.0.tar.gz", "alpha/alpha_0.2.0.tar.gz"]
    );
    assert_eq!(
        strings(packages[1].attributes().get("row.names").unwrap()),
        ["beta/beta_1.0.0.tar.gz"]
    );
}
