//! Integration tests for `PackageHelpDb`'s `help/aliases.rds` caching and
//! last-wins duplicate-alias resolution.
//!
//! `PackageHelpDb::open` requires a real, valid `.rdx`/`.rdb` pair, and
//! there's no synthetic fixture for that in this repo (every existing test
//! that needs one locates a real installed package via `Rscript`, the same
//! pattern `utils_integration.rs` and `oracle_read_topic.rs` use). These
//! tests reuse the real `utils` package's `.rdx`/`.rdb` files -- unrelated
//! to `aliases.rds` -- by copying them into a scratch directory and
//! swapping in our own `help/aliases.rds` (or omitting it entirely).
//!
//! If `Rscript` isn't available, both tests print a skip message and
//! return rather than failing.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use rd_helpdb::{Error, PackageHelpDb};

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

/// A fresh scratch directory named `<tag>/utils`, so its basename matches
/// the `utils.rdx`/`utils.rdb` files copied into it (`PackageHelpDb::open`
/// derives the package name -- and therefore the expected `.rdx`/`.rdb`
/// file names -- from the directory basename).
fn scratch_utils_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!(
            "rd-helpdb-alias-index-{tag}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
        .join("utils");
    fs::create_dir_all(dir.join("help")).expect("create scratch pkg help dir");
    dir
}

/// Copies the real `utils` package's `.rdx`/`.rdb` files (needed only so
/// `PackageHelpDb::open` succeeds; their content is irrelevant to these
/// alias tests) into `dst_pkg_dir/help`.
fn copy_rdx_rdb(src_pkg_dir: &Path, dst_pkg_dir: &Path) {
    for ext in ["rdx", "rdb"] {
        let file_name = format!("utils.{ext}");
        fs::copy(
            src_pkg_dir.join("help").join(&file_name),
            dst_pkg_dir.join("help").join(&file_name),
        )
        .unwrap_or_else(|err| panic!("copy {file_name}: {err}"));
    }
}

fn dup_aliases_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data/aliases_vector_dup_v3.rds")
}

#[test]
fn resolve_alias_last_wins_for_duplicate_aliases() {
    let Some(utils_dir) = find_utils_package_dir() else {
        println!("skipping: Rscript unavailable or 'utils' package not found");
        return;
    };

    let pkg_dir = scratch_utils_dir("dup");
    copy_rdx_rdb(&utils_dir, &pkg_dir);
    fs::copy(dup_aliases_fixture_path(), pkg_dir.join("help/aliases.rds"))
        .expect("copy aliases_vector_dup fixture");

    let db = PackageHelpDb::open(&pkg_dir)
        .unwrap_or_else(|err| panic!("open synthetic pkg at {}: {err}", pkg_dir.display()));

    let aliases = db.aliases().expect("aliases()");
    assert_eq!(
        aliases,
        vec![
            ("shared".to_string(), "first-topic".to_string()),
            ("unique".to_string(), "unique-topic".to_string()),
            ("shared".to_string(), "second-topic".to_string()),
        ],
        "aliases() should return every entry, including duplicates, in on-disk order"
    );

    let resolved = db
        .resolve_alias("shared")
        .expect("resolve_alias('shared')")
        .expect("'shared' should resolve");
    assert_eq!(
        resolved, "second-topic",
        "duplicate alias should resolve to its LAST occurrence"
    );

    let unique = db
        .resolve_alias("unique")
        .expect("resolve_alias('unique')")
        .expect("'unique' should resolve");
    assert_eq!(unique, "unique-topic");

    let _ = fs::remove_dir_all(pkg_dir.parent().expect("pkg_dir has a parent"));
}

#[test]
fn aliases_and_resolve_alias_error_without_panicking_when_file_is_missing() {
    let Some(utils_dir) = find_utils_package_dir() else {
        println!("skipping: Rscript unavailable or 'utils' package not found");
        return;
    };

    let pkg_dir = scratch_utils_dir("missing");
    copy_rdx_rdb(&utils_dir, &pkg_dir);
    // Deliberately don't write help/aliases.rds.

    let db = PackageHelpDb::open(&pkg_dir)
        .unwrap_or_else(|err| panic!("open synthetic pkg at {}: {err}", pkg_dir.display()));

    let err = db.aliases().unwrap_err();
    assert!(
        matches!(err, Error::Io { .. }),
        "expected an Io error for a missing aliases.rds, got {err:?}"
    );

    let err = db.resolve_alias("anything").unwrap_err();
    assert!(
        matches!(err, Error::Io { .. }),
        "expected an Io error for a missing aliases.rds, got {err:?}"
    );

    // A failed parse must not be cached: retrying should keep failing
    // cleanly (not panic, e.g. on an unwrap of a poisoned/empty cache).
    let err = db.aliases().unwrap_err();
    assert!(
        matches!(err, Error::Io { .. }),
        "expected a retried aliases() call to still error cleanly, got {err:?}"
    );

    let _ = fs::remove_dir_all(pkg_dir.parent().expect("pkg_dir has a parent"));
}
