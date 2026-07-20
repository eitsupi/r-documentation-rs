//! Integration test against a REAL installed `utils` package help database.
//!
//! Locates `utils` via `Rscript -e "cat(find.package('utils'))"`. If
//! `Rscript` isn't available (or errors), the test prints a skip message
//! and returns rather than failing -- the same graceful-skip pattern used
//! by `crates/rd-rds/tests/oracle_fetchrddb.rs`.

use std::{path::PathBuf, process::Command};

use rd_helpdb::{Error, PackageHelpDb};
use rd_rds::RValue;

fn find_utils_package_dir() -> Option<PathBuf> {
    let output = Command::new("Rscript")
        .args(["-e", "cat(find.package('utils'))"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let path = stdout.trim();
    if path.is_empty() {
        return None;
    }
    Some(PathBuf::from(path))
}

fn class_strings(root: &rd_rds::RObject) -> Vec<String> {
    root.class()
        .expect("root has a class attribute")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("class value is not NA")
                .expect("class value decodes")
                .into_owned()
        })
        .collect()
}

#[test]
fn utils_help_db_integration() {
    let Some(pkg_dir) = find_utils_package_dir() else {
        println!("skipping: Rscript unavailable or 'utils' package not found");
        return;
    };

    let db = PackageHelpDb::open(&pkg_dir)
        .unwrap_or_else(|err| panic!("open utils help db at {}: {err}", pkg_dir.display()));
    assert_eq!(db.package(), "utils");

    let topics: Vec<&str> = db.topics().collect();
    assert!(!topics.is_empty(), "utils has no topics");
    assert!(
        topics.contains(&"person"),
        "utils topics missing 'person': {topics:?}"
    );
    assert!(
        topics.contains(&"flush.console"),
        "utils topics missing 'flush.console': {topics:?}"
    );
    println!("utils topics: {} entries", topics.len());

    let person = db.raw_topic("person").expect("raw_topic('person')");
    assert_eq!(class_strings(&person), vec!["Rd".to_string()]);
    match &person.value() {
        RValue::List(items) => assert!(!items.is_empty(), "person Rd root list is empty"),
        other => panic!("expected person root to decode as a List, got {other:?}"),
    }
    println!("raw_topic('person'): class=Rd, decoded ok");

    let aliases = db.aliases().expect("aliases()");
    assert!(!aliases.is_empty(), "aliases.rds is empty");
    println!("aliases(): {} entries", aliases.len());

    let head_topic = db
        .resolve_alias("head")
        .expect("resolve_alias('head')")
        .expect("'head' alias should resolve to a topic in utils");
    println!("resolve_alias('head') -> {head_topic:?}");

    let missing = db
        .resolve_alias("this-alias-definitely-does-not-exist-xyz")
        .expect("resolve_alias(nonsense)");
    assert!(missing.is_none(), "nonsense alias unexpectedly resolved");

    // Exercise the .rdx references map through the same record-fetch path
    // as raw_topic, just a different key space. Real reference records
    // describe environments as plain R lists -- just assert it decodes.
    let reference_keys: Vec<&str> = db.reference_keys().collect();
    if let Some(&key) = reference_keys.first() {
        let reference = db
            .reference(key)
            .unwrap_or_else(|err| panic!("reference({key:?}) failed: {err}"));
        println!("reference({key:?}) decoded ok: {:?}", reference.value());
    } else {
        println!("utils .rdx has no references entries; skipping reference() check");
    }

    let _search_index = db.search_index().expect("search_index()");
    println!("search_index() decoded ok");

    let err = db.raw_topic("no-such-topic-xyz").unwrap_err();
    assert!(
        matches!(err, Error::UnknownTopic { .. }),
        "expected UnknownTopic, got {err:?}"
    );
}
