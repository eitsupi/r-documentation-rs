//! Integration coverage for optional-file semantics on `PackageHelpDb`.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use rd_helpdb::{Error, PackageHelpDb};

fn find_stats_package_dir() -> Option<PathBuf> {
    let output = Command::new("Rscript")
        .args(["-e", "cat(find.package('stats'))"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    (!path.trim().is_empty()).then(|| PathBuf::from(path.trim()))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(name)
}

fn scratch_stats_dir() -> PathBuf {
    std::env::temp_dir()
        .join(format!(
            "rd-helpdb-vignette-demo-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
        .join("stats")
}

fn copy_help_database(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.join("help")).expect("create help directory");
    fs::create_dir_all(destination.join("Meta")).expect("create Meta directory");
    for extension in ["rdx", "rdb"] {
        let name = format!("stats.{extension}");
        fs::copy(
            source.join("help").join(&name),
            destination.join("help").join(&name),
        )
        .unwrap_or_else(|error| panic!("copy {name}: {error}"));
    }
}

#[test]
fn optional_indexes_distinguish_missing_empty_and_malformed_files() {
    let Some(source) = find_stats_package_dir() else {
        println!("skipping: Rscript unavailable or 'stats' package not found");
        return;
    };
    let pkg_dir = scratch_stats_dir();
    copy_help_database(&source, &pkg_dir);
    let meta_dir = pkg_dir.join("Meta");
    let vignette_path = meta_dir.join("vignette.rds");
    let demo_path = meta_dir.join("demo.rds");
    let db = PackageHelpDb::open(&pkg_dir).expect("open scratch stats help database");

    assert!(db.vignettes().expect("missing vignette.rds").is_none());
    assert!(db.demos().expect("missing demo.rds").is_none());

    fs::copy(fixture("vignette_empty_v3.rds"), &vignette_path)
        .expect("copy empty vignette fixture");
    fs::copy(fixture("demo_empty_v3.rds"), &demo_path).expect("copy empty demo fixture");
    assert!(
        db.vignettes()
            .expect("empty vignette.rds")
            .expect("vignette index is present")
            .is_empty()
    );
    assert!(
        db.demos()
            .expect("empty demo.rds")
            .expect("demo index is present")
            .is_empty()
    );

    fs::copy(fixture("vignette_missing_column_v3.rds"), &vignette_path)
        .expect("replace vignette fixture");
    fs::copy(fixture("demo_three_columns_v3.rds"), &demo_path).expect("replace demo fixture");
    assert!(matches!(db.vignettes(), Err(Error::MalformedIndex(_))));
    assert!(matches!(db.demos(), Err(Error::MalformedIndex(_))));

    fs::remove_file(&vignette_path).expect("remove vignette fixture");
    fs::create_dir(&vignette_path).expect("create unreadable-as-file path");
    assert!(matches!(db.vignettes(), Err(Error::Io { .. })));

    let _ = fs::remove_dir_all(pkg_dir.parent().expect("scratch package has a parent"));
}
