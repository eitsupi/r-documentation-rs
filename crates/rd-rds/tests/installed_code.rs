#![cfg(feature = "lazyload")]

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use rd_rds::package::{
    FormalsInspection, FormalsNotApplicable, FormalsUnavailable, InstalledCodeDb,
    InstalledCodeOptions, StoredKind,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/lazyload")
        .join(name)
}

fn installed_fixture() -> PathBuf {
    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
    let package_dir = std::env::temp_dir()
        .join(format!(
            "rd-rds-installed-code-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ))
        .join("lazyfixture");
    let _ = fs::remove_dir_all(&package_dir);
    fs::create_dir_all(package_dir.join("R")).unwrap();
    fs::copy(
        fixture("installed/lazyfixture.rdx"),
        package_dir.join("R/lazyfixture.rdx"),
    )
    .unwrap();
    fs::copy(
        fixture("installed/lazyfixture.rdb"),
        package_dir.join("R/lazyfixture.rdb"),
    )
    .unwrap();
    package_dir
}

#[test]
fn opens_explicit_installed_shape_and_inspects_unique_bindings() {
    let package_dir = installed_fixture();
    let db = InstalledCodeDb::open(&package_dir).unwrap();
    assert_eq!(db.package_dir(), package_dir);
    assert_eq!(db.index_path(), package_dir.join("R/lazyfixture.rdx"));
    assert_eq!(db.data_path(), package_dir.join("R/lazyfixture.rdb"));
    assert_eq!(db.provenance().package_name(), "lazyfixture");
    assert_eq!(db.compression(), rd_rds::lazyload::Compression::Zlib);
    assert!(!db.stored_bindings().is_empty());
    assert!(
        db.stored_bindings()
            .iter()
            .any(|binding| binding.name() == "lazy_fixture_value")
    );
    assert_eq!(
        db.stored_bindings()
            .iter()
            .map(|binding| binding.name())
            .collect::<Vec<_>>(),
        [
            ".__NAMESPACE__.",
            ".__S3MethodsTable__.",
            ".packageName",
            "lazy_fixture",
            "lazy_fixture_value",
        ]
    );

    let closure = db.inspect_stored_binding("lazy_fixture").unwrap();
    assert_eq!(closure.kind(), StoredKind::Closure);
    let FormalsInspection::Available(formals) = closure.formals() else {
        panic!("expected closure formals");
    };
    assert_eq!(formals.len(), 1);
    assert_eq!(formals[0].name(), "value");
    assert!(matches!(
        formals[0].default(),
        rd_rds::package::DefaultPresence::Present
    ));
    assert!(matches!(
        closure.extent(),
        rd_rds::package::InspectionExtent::ThroughFormals { .. }
    ));
    assert_eq!(
        closure.extent().body_validation(),
        Some(rd_rds::package::BodyValidation::NotValidated)
    );

    let inspection = db.inspect_stored_binding("lazy_fixture_value").unwrap();
    assert_eq!(inspection.kind(), StoredKind::Integer);
    assert!(matches!(
        inspection.formals(),
        FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure)
    ));
    assert!(inspection.extent().body_validation().is_none());

    assert!(matches!(
        db.inspect_stored_binding("missing"),
        Err(rd_rds::package::InstalledCodeError::UnknownStoredBinding { name }) if name == "missing"
    ));
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn missing_index_is_distinct_from_missing_data_file() {
    let package_dir = std::env::temp_dir()
        .join(format!(
            "rd-rds-installed-code-missing-{}",
            std::process::id()
        ))
        .join("missing");
    let _ = fs::remove_dir_all(&package_dir);
    fs::create_dir_all(package_dir.join("R")).unwrap();
    assert!(matches!(
        InstalledCodeDb::open(&package_dir),
        Err(rd_rds::package::InstalledCodeError::NoCodeDatabase { .. })
    ));
    fs::copy(
        fixture("installed/lazyfixture.rdx"),
        package_dir.join("R/missing.rdx"),
    )
    .unwrap();
    assert!(matches!(
        InstalledCodeDb::open(&package_dir),
        Err(rd_rds::package::InstalledCodeError::Open { .. })
    ));
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn duplicate_names_are_reported_without_selecting_last_wins() {
    let package_dir = std::env::temp_dir()
        .join(format!(
            "rd-rds-installed-code-duplicate-{}",
            std::process::id()
        ))
        .join("zlib");
    let _ = fs::remove_dir_all(&package_dir);
    fs::create_dir_all(package_dir.join("R")).unwrap();
    fs::copy(fixture("zlib.rdx"), package_dir.join("R/zlib.rdx")).unwrap();
    fs::copy(fixture("zlib.rdb"), package_dir.join("R/zlib.rdb")).unwrap();
    let db = InstalledCodeDb::open(&package_dir).unwrap();
    assert_eq!(
        db.stored_bindings()
            .iter()
            .map(|binding| binding.name())
            .collect::<Vec<_>>(),
        ["duplicate", "other", "duplicate"]
    );
    assert!(matches!(
        db.inspect_stored_binding("duplicate"),
        Err(rd_rds::package::InstalledCodeError::AmbiguousStoredBinding { count: 2, .. })
    ));
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn replacement_with_changed_metadata_is_reported_and_new_open_gets_a_new_generation() {
    let package_dir = installed_fixture();
    let db = InstalledCodeDb::open(&package_dir).unwrap();
    let old_generation = db.provenance().generation();
    let data_path = package_dir.join("R/lazyfixture.rdb");
    let replacement = package_dir.join("R/replacement.rdb");
    fs::copy(fixture("installed/lazyfixture.rdb"), &replacement).unwrap();
    let mut replacement_file = fs::OpenOptions::new()
        .append(true)
        .open(&replacement)
        .unwrap();
    std::io::Write::write_all(&mut replacement_file, &[0]).unwrap();
    drop(replacement_file);
    fs::remove_file(&data_path).unwrap();
    fs::rename(replacement, &data_path).unwrap();
    assert!(matches!(
        db.inspect_stored_binding("lazy_fixture_value"),
        Err(rd_rds::package::InstalledCodeError::DatabaseChanged { .. })
    ));

    let replacement_db = InstalledCodeDb::open(&package_dir).unwrap();
    assert_ne!(old_generation, replacement_db.provenance().generation());
    let _ = fs::remove_dir_all(package_dir);
}

#[cfg(unix)]
#[test]
fn unix_same_size_replacement_is_reported_by_device_and_inode() {
    let package_dir = installed_fixture();
    let db = InstalledCodeDb::open(&package_dir).unwrap();
    let old_generation = db.provenance().generation();
    let data_path = package_dir.join("R/lazyfixture.rdb");
    let replacement = package_dir.join("R/replacement.rdb");
    fs::copy(fixture("installed/lazyfixture.rdb"), &replacement).unwrap();
    fs::remove_file(&data_path).unwrap();
    fs::rename(replacement, &data_path).unwrap();
    assert!(matches!(
        db.inspect_stored_binding("lazy_fixture_value"),
        Err(rd_rds::package::InstalledCodeError::DatabaseChanged { .. })
    ));

    let replacement_db = InstalledCodeDb::open(&package_dir).unwrap();
    assert_ne!(old_generation, replacement_db.provenance().generation());
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn inspection_and_container_limits_remain_structured() {
    let package_dir = installed_fixture();
    let index_limited = InstalledCodeDb::open_with_options(
        &package_dir,
        InstalledCodeOptions::default().max_index_bytes(1),
    )
    .unwrap_err();
    assert!(matches!(
        index_limited,
        rd_rds::package::InstalledCodeError::Index {
            source: rd_rds::lazyload::Error::IndexSizeLimitExceeded { limit: 1 },
            ..
        }
    ));

    let limited = InstalledCodeDb::open_with_options(
        &package_dir,
        InstalledCodeOptions::default().max_formals(0),
    )
    .unwrap();
    let inspection = limited.inspect_stored_binding("lazy_fixture").unwrap();
    assert!(matches!(
        inspection.formals(),
        FormalsInspection::Unavailable(FormalsUnavailable::Prefix(_))
    ));

    let byte_limited = InstalledCodeDb::open_with_options(
        &package_dir,
        InstalledCodeOptions::default().max_bytes_visited(2),
    )
    .unwrap();
    assert!(matches!(
        byte_limited.inspect_stored_binding("lazy_fixture"),
        Err(rd_rds::package::InstalledCodeError::Inspection { .. })
    ));

    let record_limited = InstalledCodeDb::open_with_options(
        &package_dir,
        InstalledCodeOptions::default().max_decompressed_record_bytes(1),
    )
    .unwrap();
    assert!(matches!(
        record_limited.inspect_stored_binding("lazy_fixture_value"),
        Err(rd_rds::package::InstalledCodeError::Record { .. })
    ));
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn empty_variables_database_is_not_missing() {
    let package_dir = std::env::temp_dir()
        .join(format!(
            "rd-rds-installed-code-empty-{}",
            std::process::id()
        ))
        .join("empty");
    let _ = fs::remove_dir_all(&package_dir);
    fs::create_dir_all(package_dir.join("R")).unwrap();
    fs::copy(
        fixture("installed/empty.rdx"),
        package_dir.join("R/empty.rdx"),
    )
    .unwrap();
    fs::copy(
        fixture("installed/empty.rdb"),
        package_dir.join("R/empty.rdb"),
    )
    .unwrap();
    let db = InstalledCodeDb::open(&package_dir).unwrap();
    assert!(db.stored_bindings().is_empty());
    assert!(matches!(
        db.inspect_stored_binding("anything"),
        Err(rd_rds::package::InstalledCodeError::UnknownStoredBinding { .. })
    ));
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn one_bad_record_does_not_hide_other_records_or_the_index() {
    let package_dir = installed_fixture();
    let data_path = package_dir.join("R/lazyfixture.rdb");
    let low_level =
        rd_rds::lazyload::LazyLoadDb::open(package_dir.join("R/lazyfixture.rdx"), &data_path)
            .unwrap();
    let bad_location = low_level
        .variable("lazy_fixture_value")
        .unwrap()
        .location()
        .unwrap();
    let mut data = fs::read(&data_path).unwrap();
    data[bad_location.offset() as usize] ^= 0xff;
    fs::write(&data_path, data).unwrap();
    let db = InstalledCodeDb::open(&package_dir).unwrap();

    assert!(matches!(
        db.inspect_stored_binding("lazy_fixture_value"),
        Err(rd_rds::package::InstalledCodeError::Record { .. })
    ));
    assert_eq!(db.stored_bindings().len(), 5);
    let closure = db.inspect_stored_binding("lazy_fixture").unwrap();
    assert_eq!(closure.kind(), StoredKind::Closure);
    let _ = fs::remove_dir_all(package_dir);
}
