//! Standalone tests for [`rd_helpdb::read_rds_file`] that don't depend on
//! an installed R library.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "rd-helpdb-read-rds-file-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

#[test]
fn reports_invalid_bzip2_payloads_after_delegation() {
    let dir = scratch_dir();
    let path = dir.join("fake.rds");
    // The standalone reader now accepts the bzip2 envelope and reports the
    // malformed payload from the delegated decompressor.
    fs::write(&path, b"BZh91AY&SY\x00\x00\x00\x00").expect("write fake bzip2 file");

    let err = rd_helpdb::read_rds_file(&path).unwrap_err();
    assert!(
        matches!(err, rd_helpdb::Error::RdsFile(_)),
        "expected delegated RDS file error, got {err:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}
