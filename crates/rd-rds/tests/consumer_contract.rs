#![cfg(feature = "lazyload")]

use std::{
    collections::BTreeSet,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use rd_rds::package::{
    DefaultPresence, FormalsInspection, FormalsNotApplicable, FormalsUnavailable, InstalledCodeDb,
    InstalledCodeError, InstalledCodeOptions, MetadataField, NamespaceMetadata, StoredKind,
};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(path)
}

fn package_fixture(database: &str) -> PathBuf {
    static NEXT_PACKAGE: AtomicUsize = AtomicUsize::new(0);
    let package_name = database
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".rdx"))
        .unwrap_or(database);
    let package_dir = std::env::temp_dir()
        .join(format!(
            "rd-rds-consumer-contract-{}-{}",
            std::process::id(),
            NEXT_PACKAGE.fetch_add(1, Ordering::Relaxed)
        ))
        .join(package_name);
    let _ = fs::remove_dir_all(&package_dir);
    fs::create_dir_all(package_dir.join("R")).unwrap();
    fs::create_dir_all(package_dir.join("Meta")).unwrap();
    fs::copy(
        fixture("namespace-installed/namespace-installed.rds"),
        package_dir.join("Meta/nsInfo.rds"),
    )
    .unwrap();
    let index = fixture(database);
    let data = index.with_extension("rdb");
    fs::copy(&index, package_dir.join(format!("R/{package_name}.rdx"))).unwrap();
    fs::copy(&data, package_dir.join(format!("R/{package_name}.rdb"))).unwrap();
    package_dir
}

fn mini_roxygen_s3_evidence(metadata: &NamespaceMetadata) -> BTreeSet<String> {
    match metadata.s3_generic_evidence() {
        MetadataField::Present(values) => values.iter().cloned().collect(),
        MetadataField::Missing
        | MetadataField::Invalid(_)
        | MetadataField::UnsupportedSchema { .. }
        | _ => BTreeSet::new(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ConsumerSignature {
    Available(Vec<(String, DefaultPresence)>),
    NotApplicable(&'static str),
    Unavailable(&'static str),
}

fn oak_signature(db: &InstalledCodeDb, name: &str) -> ConsumerSignature {
    let inspection = db.inspect_stored_binding(name).unwrap();
    match inspection.formals() {
        FormalsInspection::Available(formals) => ConsumerSignature::Available(
            formals
                .iter()
                .map(|formal| (formal.name().to_owned(), formal.default()))
                .collect(),
        ),
        FormalsInspection::NotApplicable(FormalsNotApplicable::BuiltIn) => {
            ConsumerSignature::NotApplicable("built-in")
        }
        FormalsInspection::NotApplicable(FormalsNotApplicable::Special) => {
            ConsumerSignature::NotApplicable("special")
        }
        FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure) => {
            ConsumerSignature::NotApplicable("non-closure")
        }
        FormalsInspection::Unavailable(FormalsUnavailable::PromiseNotEvaluated) => {
            ConsumerSignature::Unavailable("promise-not-evaluated")
        }
        FormalsInspection::Unavailable(FormalsUnavailable::PersistentReferenceUnresolved) => {
            ConsumerSignature::Unavailable("persistent-reference-unresolved")
        }
        FormalsInspection::Unavailable(FormalsUnavailable::Prefix(_)) => {
            ConsumerSignature::Unavailable("prefix-unavailable")
        }
        _ => ConsumerSignature::Unavailable("unknown"),
    }
}

fn arf_stored_bindings(db: &InstalledCodeDb) -> Vec<String> {
    db.stored_bindings()
        .iter()
        .map(|binding| binding.name().to_owned())
        .collect()
}

#[test]
fn consumer_policies_keep_namespace_code_and_runtime_domains_distinct() {
    let package_dir = package_fixture("lazyload/installed/lazyfixture.rdx");
    let namespace = NamespaceMetadata::from_object(
        &rd_rds::file::read(package_dir.join("Meta/nsInfo.rds")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        mini_roxygen_s3_evidence(&namespace),
        BTreeSet::from(["print".to_owned()])
    );

    let db = InstalledCodeDb::open(&package_dir).unwrap();
    let bindings = arf_stored_bindings(&db);
    assert_eq!(bindings.len(), 5);
    assert!(bindings.iter().any(|name| name == "lazy_fixture"));
    assert!(bindings.iter().any(|name| name == "lazy_fixture_value"));
    assert_eq!(
        oak_signature(&db, "lazy_fixture"),
        ConsumerSignature::Available(vec![("value".to_owned(), DefaultPresence::Present,)])
    );
    assert_eq!(
        oak_signature(&db, "lazy_fixture_value"),
        ConsumerSignature::NotApplicable("non-closure")
    );
    assert_eq!(
        db.inspect_stored_binding("lazy_fixture").unwrap().kind(),
        StoredKind::Closure
    );
    assert_eq!(
        db.inspect_stored_binding("lazy_fixture_value")
            .unwrap()
            .kind(),
        StoredKind::Integer
    );
    let bounded_db = InstalledCodeDb::open_with_options(
        &package_dir,
        InstalledCodeOptions::default().max_formals(0),
    )
    .unwrap();
    assert_eq!(
        oak_signature(&bounded_db, "lazy_fixture"),
        ConsumerSignature::Unavailable("prefix-unavailable")
    );
    let _ = fs::remove_dir_all(package_dir);
}

#[test]
fn consumer_handles_missing_empty_unknown_and_ambiguous_databases() {
    let missing_dir = package_fixture("lazyload/installed/empty.rdx");
    fs::remove_file(missing_dir.join("R/empty.rdx")).unwrap();
    assert!(matches!(
        InstalledCodeDb::open(&missing_dir),
        Err(InstalledCodeError::NoCodeDatabase { .. })
    ));
    let _ = fs::remove_dir_all(&missing_dir);

    let empty_dir = package_fixture("lazyload/installed/empty.rdx");
    let empty_db = InstalledCodeDb::open(&empty_dir).unwrap();
    assert!(empty_db.stored_bindings().is_empty());
    assert!(matches!(
        empty_db.inspect_stored_binding("missing"),
        Err(InstalledCodeError::UnknownStoredBinding { .. })
    ));
    let _ = fs::remove_dir_all(&empty_dir);

    let duplicate_dir = package_fixture("lazyload/zlib.rdx");
    let duplicate_db = InstalledCodeDb::open(&duplicate_dir).unwrap();
    assert!(matches!(
        duplicate_db.inspect_stored_binding("duplicate"),
        Err(InstalledCodeError::AmbiguousStoredBinding { count: 2, .. })
    ));
    let _ = fs::remove_dir_all(duplicate_dir);
}
