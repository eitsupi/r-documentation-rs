#![cfg(feature = "gzip")]

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rd_rds::{
    file::ReadOptions,
    package::{InstalledMetadataError, NamespaceMetadata, PackageMeta},
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(name)
}

fn temporary_package(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let package_dir = std::env::temp_dir().join(format!("rd-rds-installed-{name}-{nonce}"));
    fs::create_dir_all(package_dir.join("Meta")).expect("create metadata directory");
    package_dir
}

fn copy_fixture(source: &str, package_dir: &Path, artifact: &str) {
    fs::copy(fixture(source), package_dir.join("Meta").join(artifact)).expect("copy fixture");
}

#[test]
fn reads_canonical_installed_metadata_paths() {
    let package_dir = temporary_package("happy");
    copy_fixture(
        "namespace-installed/namespace-installed.rds",
        &package_dir,
        "nsInfo.rds",
    );
    copy_fixture(
        "package-meta/fixturepkg-package.rds",
        &package_dir,
        "package.rds",
    );

    let namespace = NamespaceMetadata::read_installed(&package_dir).expect("namespace metadata");
    assert!(namespace.declared_exports().present().is_some());
    let package = PackageMeta::read_installed(&package_dir).expect("package metadata");
    assert_eq!(package.description_field("Priority"), Some(Some("tools")));
    assert_eq!(
        package
            .built()
            .expect("Built metadata")
            .r_version()
            .to_string(),
        "4.6.1"
    );

    fs::remove_dir_all(package_dir).expect("remove temporary package");
}

#[test]
fn preserves_read_and_decode_error_context() {
    let package_dir = temporary_package("errors");
    let missing = NamespaceMetadata::read_installed(&package_dir).unwrap_err();
    match missing {
        InstalledMetadataError::Read { path, source } => {
            assert_eq!(path, package_dir.join("Meta/nsInfo.rds"));
            assert!(
                matches!(source, rd_rds::file::ReadError::Io { path: io_path, .. } if io_path == path)
            );
        }
        other => panic!("expected a read error, got {other:?}"),
    }

    copy_fixture(
        "namespace-installed/namespace-installed.rds",
        &package_dir,
        "package.rds",
    );
    let malformed = PackageMeta::read_installed(&package_dir).unwrap_err();
    assert!(
        matches!(malformed, InstalledMetadataError::View { ref path, .. } if path == &package_dir.join("Meta/package.rds"))
    );

    fs::remove_dir_all(package_dir).expect("remove temporary package");
}

#[test]
fn explicit_read_options_bound_the_artifact_read() {
    let package_dir = temporary_package("bounds");
    copy_fixture(
        "package-meta/fixturepkg-package.rds",
        &package_dir,
        "package.rds",
    );
    let options = ReadOptions::default().max_compressed_bytes(1);
    let error = PackageMeta::read_installed_with_options(&package_dir, &options).unwrap_err();
    assert!(
        matches!(error, InstalledMetadataError::Read { ref path, source: rd_rds::file::ReadError::CompressedSizeLimitExceeded { limit: 1 } } if path == &package_dir.join("Meta/package.rds"))
    );

    fs::remove_dir_all(package_dir).expect("remove temporary package");
}
