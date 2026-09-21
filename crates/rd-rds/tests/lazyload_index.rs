#![cfg(feature = "lazyload")]

use std::{fs, io::Read, path::PathBuf};

use rd_rds::lazyload::{Compression, Error, LazyLoadDb, LazyLoadIndex, Options, RecordLocation};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/lazyload")
        .join(name)
}

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rd-rds-index-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn opens_indexes_without_data_files_and_matches_database_accessors() {
    let scratch = ScratchDir::new("without-data");
    for (index_name, data_name) in [
        ("raw-v2.rdx", "raw-v2.rdb"),
        ("raw.rdx", "raw.rdb"),
        ("zlib-v2.rdx", "zlib-v2.rdb"),
        ("zlib.rdx", "zlib.rdb"),
        ("compound.rdx", "zlib.rdb"),
        ("installed/lazyfixture.rdx", "installed/lazyfixture.rdb"),
        ("installed/empty.rdx", "installed/empty.rdb"),
    ] {
        let index_path = scratch.0.join("pkg.rdx");
        let missing_data_path = scratch.0.join("pkg.rdb");
        fs::copy(fixture(index_name), &index_path).unwrap();
        let index = LazyLoadIndex::open(&index_path).unwrap();
        assert!(matches!(
            LazyLoadDb::open(&index_path, &missing_data_path),
            Err(Error::Io { path, source })
                if path == missing_data_path && source.kind() == std::io::ErrorKind::NotFound
        ));

        let db = LazyLoadDb::open(fixture(index_name), fixture(data_name)).unwrap();
        assert_eq!(index.compression(), db.compression());
        assert_eq!(index.variables(), db.variables());
        assert_eq!(index.references(), db.references());
        for variable in index.variables() {
            assert_eq!(
                index.variable(variable.name()),
                db.variable(variable.name())
            );
        }
        for (name, _) in index.references() {
            assert_eq!(index.reference(name), db.reference(name));
        }
        assert_eq!(index.variable("missing"), None);
        assert_eq!(index.reference("missing"), None);
    }
}

#[test]
fn preserves_duplicate_variables_and_uses_the_last_for_lookup() {
    let index = LazyLoadIndex::open(fixture("zlib.rdx")).unwrap();
    assert_eq!(
        index
            .variables()
            .iter()
            .map(|v| v.name())
            .collect::<Vec<_>>(),
        ["duplicate", "other", "duplicate"]
    );
    assert_ne!(
        index.variables()[0].location(),
        index.variables()[2].location()
    );
    assert_eq!(index.variable("duplicate"), Some(&index.variables()[2]));
}

#[test]
fn distinguishes_empty_variables_from_a_missing_index() {
    let index = LazyLoadIndex::open(fixture("installed/empty.rdx")).unwrap();
    assert!(index.variables().is_empty());
    assert_eq!(index.variable("missing"), None);
    let scratch = ScratchDir::new("missing");
    let missing = scratch.0.join("missing.rdx");
    assert!(matches!(
        LazyLoadIndex::open(&missing),
        Err(Error::Io { path, source })
            if path == missing && source.kind() == std::io::ErrorKind::NotFound
    ));
}

#[test]
fn enforces_stored_index_limit_and_accepts_the_boundary() {
    let path = fixture("raw.rdx");
    let length = fs::read(&path).unwrap().len();
    LazyLoadIndex::open_with_options(&path, Options::default().max_index_bytes(length)).unwrap();
    assert!(matches!(
        LazyLoadIndex::open_with_options(&path, Options::default().max_index_bytes(length - 1)),
        Err(Error::IndexSizeLimitExceeded { limit }) if limit == length - 1
    ));
}

#[test]
fn enforces_decompressed_index_limit_and_accepts_the_boundary() {
    let path = fixture("installed/lazyfixture.rdx");
    let stored = fs::read(&path).unwrap();
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(stored.as_slice())
        .read_to_end(&mut decoded)
        .unwrap();
    assert!(stored.len() < decoded.len());
    LazyLoadIndex::open_with_options(&path, Options::default().max_index_bytes(decoded.len()))
        .unwrap();
    assert!(matches!(
        LazyLoadIndex::open_with_options(
            &path,
            Options::default().max_index_bytes(decoded.len() - 1)
        ),
        Err(Error::InvalidIndex { .. })
    ));
}

#[test]
fn ignores_record_limits_and_defers_data_range_validation() {
    let index = LazyLoadIndex::open_with_options(
        fixture("bad-range.rdx"),
        Options::default()
            .max_stored_record_bytes(0)
            .max_decompressed_record_bytes(0),
    )
    .unwrap();
    assert_eq!(
        index.variable("first").unwrap().location(),
        Some(RecordLocation::new(1000, 2))
    );
    assert!(matches!(
        LazyLoadIndex::open(fixture("bad-overflow.rdx")),
        Err(Error::InvalidVariable { .. })
    ));
}

#[test]
fn inspects_record_compression_without_requiring_its_codec() {
    for (name, compression) in [
        ("raw.rdx", Compression::None),
        ("zlib.rdx", Compression::Zlib),
        ("unsupported-2.rdx", Compression::Bzip2),
        ("unsupported-3.rdx", Compression::Xz),
    ] {
        let index = LazyLoadIndex::open(fixture(name)).unwrap();
        assert_eq!(index.compression(), compression);
        assert!(!index.variables().is_empty());
    }
}

#[test]
fn rejects_a_truncated_index() {
    let scratch = ScratchDir::new("truncated");
    let path = scratch.0.join("pkg.rdx");
    let mut bytes = fs::read(fixture("raw.rdx")).unwrap();
    bytes.truncate(bytes.len() / 2);
    fs::write(&path, bytes).unwrap();
    assert!(matches!(
        LazyLoadIndex::open(path),
        Err(Error::InvalidIndex { .. })
    ));
}
