#![cfg(feature = "lazyload")]

use std::{fs, path::PathBuf};

use rd_rds::{
    RValue,
    lazyload::{Compression, Error, LazyLoadDb, Options, RecordReference},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/lazyload")
        .join(name)
}

#[test]
fn opens_and_reads_raw_records_from_rds_v2_and_v3() {
    for suffix in ["-v2", ""] {
        let db = LazyLoadDb::open(
            fixture(&format!("raw{suffix}.rdx")),
            fixture(&format!("raw{suffix}.rdb")),
        )
        .unwrap();
        let record = db.read("first").unwrap();
        assert_eq!(db.compression(), Compression::None);
        assert_eq!(record.stored_bytes(), record.decompressed_bytes());
        assert!(matches!(
            rd_rds::parse(record.decompressed_bytes()).unwrap().value(),
            RValue::List(_)
        ));
    }
}

#[test]
fn reads_a_real_installed_package_pair_with_lazyload_only_profile() {
    let db = LazyLoadDb::open(
        fixture("installed/lazyfixture.rdx"),
        fixture("installed/lazyfixture.rdb"),
    )
    .unwrap();
    assert!(db.variable("lazy_fixture").is_some());
    let record = db.read("lazy_fixture_value").unwrap();
    let parsed = rd_rds::parse(record.decompressed_bytes()).unwrap();
    assert!(matches!(
        parsed.value(),
        RValue::Integer(values) if values == &vec![Some(42)]
    ));
}

#[test]
fn opens_and_reads_zlib_records_from_rds_v2_and_v3_with_last_wins_lookup() {
    for suffix in ["-v2", ""] {
        let db = LazyLoadDb::open(
            fixture(&format!("zlib{suffix}.rdx")),
            fixture(&format!("zlib{suffix}.rdb")),
        )
        .unwrap();
        assert_eq!(db.variables().len(), 3);
        assert_eq!(
            db.variables()
                .iter()
                .map(|variable| variable.name())
                .collect::<Vec<_>>(),
            ["duplicate", "other", "duplicate"]
        );
        let first_location = db.variables()[0].location().unwrap();
        let duplicate_location = db.variable("duplicate").unwrap().location().unwrap();
        assert_ne!(first_location, duplicate_location);
        assert_eq!(duplicate_location.offset(), first_location.length());
        let record = db.read("duplicate").unwrap();
        assert_eq!(db.compression(), Compression::Zlib);
        assert_eq!(
            record.stored_bytes().len() as u64,
            duplicate_location.length()
        );
        assert!(!record.decompressed_bytes().is_empty());
        let parsed = rd_rds::parse(record.decompressed_bytes()).unwrap();
        assert!(matches!(parsed.value(), RValue::List(_)));
        assert!(matches!(
            parsed.get_named("value").map(|object| object.value()),
            Some(RValue::Integer(values)) if values == &vec![Some(43)]
        ));
        assert_eq!(record.location(), duplicate_location);
    }
}

#[test]
fn parses_compound_reference_fixture() {
    let db = LazyLoadDb::open(fixture("compound.rdx"), fixture("zlib.rdb")).unwrap();
    assert_eq!(
        db.variable("first").unwrap().location(),
        Some(rd_rds::lazyload::RecordLocation::new(0, 108))
    );
    assert_eq!(
        db.reference("env"),
        Some(&RecordReference::Compound {
            eager_key: Some(rd_rds::lazyload::RecordLocation::new(0, 108)),
            lazy_keys: vec![("line".into(), rd_rds::lazyload::RecordLocation::new(0, 108))],
        })
    );
}

#[test]
fn reports_missing_reference_with_reference_specific_error() {
    let db = LazyLoadDb::open(fixture("compound.rdx"), fixture("zlib.rdb")).unwrap();
    assert!(matches!(
        db.read_reference("missing"),
        Err(Error::UnknownReference { name }) if name == "missing"
    ));
}

#[test]
fn codec_two_and_three_are_explicitly_unsupported() {
    for code in [2, 3] {
        let index = fixture(format!("unsupported-{code}.rdx").as_str());
        let db = LazyLoadDb::open(index, fixture("zlib.rdb")).unwrap();
        assert!(matches!(
            db.read("first"),
            Err(Error::CompressionUnsupported { .. })
        ));
    }
}

#[test]
fn detects_data_replacement_after_open() {
    let temp = tempfile_path("lazyload-replacement");
    fs::create_dir_all(&temp).unwrap();
    fs::copy(fixture("zlib.rdx"), temp.join("pkg.rdx")).unwrap();
    fs::copy(fixture("zlib.rdb"), temp.join("pkg.rdb")).unwrap();
    let db = LazyLoadDb::open(temp.join("pkg.rdx"), temp.join("pkg.rdb")).unwrap();
    let replacement = temp.join("replacement.rdb");
    fs::copy(fixture("raw.rdb"), &replacement).unwrap();
    fs::remove_file(temp.join("pkg.rdb")).unwrap();
    fs::rename(replacement, temp.join("pkg.rdb")).unwrap();
    assert!(matches!(db.read("duplicate"), Err(Error::DataFileChanged)));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn enforces_record_limits_before_allocation() {
    let db = LazyLoadDb::open_with_options(
        fixture("zlib.rdx"),
        fixture("zlib.rdb"),
        Options::default().max_stored_record_bytes(4),
    )
    .unwrap();
    assert!(matches!(
        db.read("duplicate"),
        Err(Error::StoredRecordSizeLimitExceeded { limit: 4 })
    ));

    let db = LazyLoadDb::open_with_options(
        fixture("zlib.rdx"),
        fixture("zlib.rdb"),
        Options::default().max_decompressed_record_bytes(4),
    )
    .unwrap();
    assert!(matches!(
        db.read("duplicate"),
        Err(Error::DecompressedRecordSizeLimitExceeded { limit: 4 })
    ));
}

#[test]
fn rejects_bad_ranges_and_index_limits() {
    let db = LazyLoadDb::open(fixture("bad-range.rdx"), fixture("raw.rdb")).unwrap();
    assert!(matches!(
        db.read("first"),
        Err(Error::RecordOutOfRange { .. })
    ));

    let result = LazyLoadDb::open_with_options(
        fixture("raw.rdx"),
        fixture("raw.rdb"),
        Options::default().max_index_bytes(4),
    );
    assert!(matches!(
        result,
        Err(Error::IndexSizeLimitExceeded { limit: 4 })
    ));

    let result = LazyLoadDb::open(fixture("bad-overflow.rdx"), fixture("raw.rdb"));
    assert!(matches!(result, Err(Error::InvalidVariable { .. })));
}

fn tempfile_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rd-rds-{name}-{}", std::process::id()))
}
