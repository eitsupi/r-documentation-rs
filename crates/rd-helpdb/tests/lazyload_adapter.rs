//! End-to-end coverage for the `rd-rds::lazyload` adapter used by
//! `PackageHelpDb`.

use std::{fs, path::PathBuf};

use rd_helpdb::{Error, PackageHelpDb};
use rd_rds::RValue;

fn lazyload_fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rd-rds/tests/fixtures/data/lazyload")
        .join(name)
}

struct ScratchDir(PathBuf);

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn compound_references_do_not_block_topic_access() {
    let root =
        std::env::temp_dir().join(format!("rd-helpdb-lazyload-adapter-{}", std::process::id()));
    let _scratch = ScratchDir(root.clone());
    let pkg_dir = root.join("compound");
    let help_dir = pkg_dir.join("help");
    fs::create_dir_all(&help_dir).expect("create temporary help directory");
    fs::copy(
        lazyload_fixture("compound.rdx"),
        help_dir.join("compound.rdx"),
    )
    .expect("copy compound index fixture");
    fs::copy(lazyload_fixture("zlib.rdb"), help_dir.join("compound.rdb"))
        .expect("copy record fixture");

    let db = PackageHelpDb::open(&pkg_dir).expect("open compound help database");
    let topics: Vec<_> = db.topics().collect();
    assert_eq!(topics, vec!["first"]);

    let topic = db.raw_topic("first").expect("read direct topic");
    assert!(matches!(topic.value(), RValue::List(_)));

    assert_eq!(db.reference_keys().collect::<Vec<_>>(), vec!["env"]);
    assert!(matches!(
        db.reference("env"),
        Err(Error::UnsupportedReference { key }) if key == "env"
    ));
}
