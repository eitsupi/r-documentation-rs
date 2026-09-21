//! Consumer contracts for installed help-topic metadata, independent of R.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use rd_helpdb::{Error, HelpTopicIndex, HelpTopicText, PackageHelpDb, read_rds_file};
use rd_rds::{
    Attribute, Attributes, NativeEncodingSource, REncoding, RObject, RStr, RValue, Symbol,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(name)
}

fn metadata() -> RObject {
    read_rds_file(fixture("help_topics_metadata_v3.rds")).unwrap()
}

fn object(value: RValue) -> RObject {
    RObject::from_parts(value, Attributes::default())
}

fn text(value: &str) -> RStr {
    RStr::new(
        value.as_bytes(),
        REncoding::Utf8,
        NativeEncodingSource::Unknown,
    )
}

fn replace_column(root: RObject, name: &str, value: Option<RObject>) -> RObject {
    let mut names = root.names().unwrap().to_vec();
    let position = names
        .iter()
        .position(|s| s.as_str().unwrap().unwrap() == name)
        .unwrap();
    let (RValue::List(mut columns), attributes) = root.into_parts() else {
        panic!("list")
    };
    if let Some(value) = value {
        columns[position] = value;
    } else {
        columns.remove(position);
        names.remove(position);
    }
    let attributes = attributes
        .iter()
        .map(|attr| {
            if attr.name().as_str() == "names" {
                Attribute::new(
                    Symbol::new("names"),
                    object(RValue::Character(names.clone())),
                )
            } else {
                attr.clone()
            }
        })
        .collect();
    RObject::from_parts(RValue::List(columns), Attributes::new(attributes))
}

#[test]
fn retains_rows_alias_groups_na_and_source_names_in_both_formats() {
    for version in [2, 3] {
        let root = read_rds_file(fixture(&format!("help_topics_metadata_v{version}.rds"))).unwrap();
        let index = HelpTopicIndex::from_object(&root).unwrap();
        let entries: Vec<_> = index.entries().collect();
        assert_eq!(entries.len(), 4);
        assert_eq!(index.len(), 4);
        assert!(!index.is_empty());
        assert_eq!(
            entries[0].aliases,
            [
                Some("shared".into()),
                Some("first".into()),
                None,
                Some("first".into())
            ]
        );
        assert_eq!(entries[0].title.as_str(), Some("First topic title"));
        assert_eq!(entries[0].file.as_str(), Some("first-topic.Rd"));
        assert_eq!(entries[0].topic_key(), Some("first-topic"));
        assert_eq!(entries[1].title, HelpTopicText::Na);
        assert_eq!(entries[2].file, HelpTopicText::Na);
        assert_eq!(entries[2].topic_key(), None);
        assert_eq!(entries[3].title.as_str(), Some(""));
        assert!(entries[3].aliases.is_empty());
        assert_eq!(entries[3].topic_key(), Some("nested.Rd"));
        assert_eq!(HelpTopicIndex::try_from(&root).unwrap(), index);
    }
}

#[test]
fn alias_lookup_uses_the_first_row_without_discarding_duplicates() {
    let index = HelpTopicIndex::from_object(&metadata()).unwrap();
    assert_eq!(
        index.find_alias("shared").unwrap().topic_key(),
        Some("first-topic")
    );
    assert_eq!(
        index.find_alias("second").unwrap().topic_key(),
        Some("second-topic")
    );
    assert!(index.find_alias("unknown").is_none());
    assert_eq!(
        index
            .entries()
            .filter(|entry| entry.aliases.iter().any(|a| a.as_deref() == Some("shared")))
            .count(),
        2
    );

    let root = replace_column(
        metadata(),
        "Title",
        Some(object(RValue::Character(vec![
            RStr::Na,
            text("Later title"),
            RStr::Na,
            RStr::Na,
        ]))),
    );
    let index = HelpTopicIndex::from_object(&root).unwrap();
    assert_eq!(index.find_alias("shared").unwrap().title, HelpTopicText::Na);
    assert!(index.find_alias("Shared").is_none());
}

#[test]
fn missing_optional_columns_and_empty_metadata_are_explicit() {
    for version in [2, 3] {
        let root =
            read_rds_file(fixture(&format!("help_topics_aliases_only_v{version}.rds"))).unwrap();
        let index = HelpTopicIndex::from_object(&root).unwrap();
        let entry = index.find_alias("first").unwrap();
        assert_eq!(entry.title, HelpTopicText::Missing);
        assert_eq!(entry.file, HelpTopicText::Missing);
        assert_eq!(entry.topic_key(), None);
        let root = read_rds_file(fixture(&format!("help_topics_empty_v{version}.rds"))).unwrap();
        let index = HelpTopicIndex::from_object(&root).unwrap();
        assert!(index.is_empty());
        assert_eq!(index.entries().len(), 0);
    }
}

#[test]
fn malformed_optional_fields_do_not_destroy_other_metadata() {
    let root = replace_column(
        metadata(),
        "File",
        Some(object(RValue::Integer(vec![Some(1); 4]))),
    );
    let index = HelpTopicIndex::from_object(&root).unwrap();
    let entry = index.find_alias("first").unwrap();
    assert!(matches!(entry.file, HelpTopicText::Invalid(_)));
    assert_eq!(entry.title.as_str(), Some("First topic title"));
    assert_eq!(entry.topic_key(), None);

    let root = replace_column(
        metadata(),
        "Title",
        Some(object(RValue::Character(vec![text("Survives")]))),
    );
    let index = HelpTopicIndex::from_object(&root).unwrap();
    assert_eq!(
        index.find_alias("first").unwrap().title.as_str(),
        Some("Survives")
    );
    assert!(matches!(
        index.find_alias("second").unwrap().title,
        HelpTopicText::Invalid(_)
    ));
    assert_eq!(
        index.find_alias("second").unwrap().topic_key(),
        Some("second-topic")
    );

    let bad_string = RStr::new(&[0xff], REncoding::Utf8, NativeEncodingSource::Unknown);
    let root = replace_column(
        metadata(),
        "Title",
        Some(object(RValue::Character(vec![
            bad_string,
            text("Good"),
            RStr::Na,
            text(""),
        ]))),
    );
    let index = HelpTopicIndex::from_object(&root).unwrap();
    assert!(matches!(
        index.find_alias("first").unwrap().title,
        HelpTopicText::Invalid(_)
    ));
    assert_eq!(
        index.find_alias("second").unwrap().title.as_str(),
        Some("Good")
    );

    let root = replace_column(
        metadata(),
        "Title",
        Some(object(RValue::Character(vec![text("extra"); 5]))),
    );
    let index = HelpTopicIndex::from_object(&root).unwrap();
    assert!(
        index
            .entries()
            .all(|entry| matches!(entry.title, HelpTopicText::Invalid(_)))
    );
    assert_eq!(
        index.find_alias("first").unwrap().topic_key(),
        Some("first-topic")
    );
}

#[test]
fn rejects_invalid_root_names_and_required_alias_schema() {
    let (value, _) = metadata().into_parts();
    for root in [
        object(RValue::Null),
        object(value),
        replace_column(metadata(), "Aliases", None),
        replace_column(
            metadata(),
            "Aliases",
            Some(object(RValue::Character(vec![]))),
        ),
        replace_column(
            metadata(),
            "Aliases",
            Some(object(RValue::List(vec![object(RValue::Null); 4]))),
        ),
        replace_column(metadata(), "Aliases", Some(object(RValue::List(vec![])))),
    ] {
        assert!(matches!(
            HelpTopicIndex::from_object(&root),
            Err(Error::MalformedIndex(_))
        ));
    }
    let (value, attributes) = metadata().into_parts();
    for names in [vec![text("Title"); 4], vec![RStr::Na; 4], vec![]] {
        let attrs = attributes
            .iter()
            .filter(|a| a.name().as_str() != "names")
            .cloned()
            .chain([Attribute::new(
                Symbol::new("names"),
                object(RValue::Character(names)),
            )])
            .collect();
        assert!(matches!(
            HelpTopicIndex::from_object(&RObject::from_parts(
                value.clone(),
                Attributes::new(attrs)
            )),
            Err(Error::MalformedIndex(_))
        ));
    }
}

#[test]
fn topic_keys_match_r_basename_and_rd_suffix_rules() {
    let index = HelpTopicIndex::from_object(&metadata()).unwrap();
    let mut entry = index.entries().next().unwrap().clone();
    for (file, key) in [
        ("topic.Rd", "topic"),
        ("topic.rd", "topic"),
        ("dir/topic.Rd", "topic"),
        ("dir/topic.rd", "topic"),
        ("/dir//topic.rd", "topic"),
        ("dir/topic.Rd/", "topic"),
        ("dir/.", "."),
        ("dir/..", ".."),
        ("nested.Rd.Rd", "nested.Rd"),
        ("nested.rd.rd", "nested.rd"),
        ("topic.RD", "topic.RD"),
        ("topic.rD", "topic.rD"),
        ("no-extension", "no-extension"),
        ("", ""),
        ("/", ""),
    ] {
        entry.file = HelpTopicText::Text(file.into());
        assert_eq!(entry.topic_key(), Some(key), "source file: {file:?}");
        assert_eq!(entry.file.as_str(), Some(file));
    }
}

struct Package(PathBuf);

impl Package {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "rd-helpdb-metadata-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
            .join("fixturepkg");
        fs::create_dir_all(path.join("Meta")).unwrap();
        Self(path)
    }

    fn metadata(&self, name: &str) {
        fs::copy(fixture(name), self.0.join("Meta/Rd.rds")).unwrap();
    }

    fn help(&self) {
        fs::create_dir_all(self.0.join("help")).unwrap();
        for extension in ["rdx", "rdb"] {
            fs::copy(
                fixture(&format!("help_topics.{extension}")),
                self.0.join(format!("help/fixturepkg.{extension}")),
            )
            .unwrap();
        }
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

#[test]
fn installed_reader_distinguishes_missing_empty_invalid_and_io_errors() {
    let pkg = Package::new();
    assert!(HelpTopicIndex::read_installed(&pkg.0).unwrap().is_none());
    pkg.metadata("help_topics_empty_v3.rds");
    assert!(
        HelpTopicIndex::read_installed(&pkg.0)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    pkg.metadata("aliases_vector_dup_v3.rds");
    assert!(matches!(
        HelpTopicIndex::read_installed(&pkg.0),
        Err(Error::MalformedIndex(_))
    ));
    fs::write(pkg.0.join("Meta/Rd.rds"), b"not RDS").unwrap();
    assert!(HelpTopicIndex::read_installed(&pkg.0).is_err());
    fs::remove_file(pkg.0.join("Meta/Rd.rds")).unwrap();
    fs::create_dir(pkg.0.join("Meta/Rd.rds")).unwrap();
    assert!(matches!(
        HelpTopicIndex::read_installed(&pkg.0),
        Err(Error::Io { .. })
    ));
}

#[test]
fn installed_reader_enforces_explicit_file_limits() {
    let pkg = Package::new();
    pkg.metadata("help_topics_metadata_v3.rds");
    let options = rd_rds::file::ReadOptions::default().max_compressed_bytes(1);
    assert!(matches!(
        HelpTopicIndex::read_installed_with_options(&pkg.0, &options),
        Err(Error::RdsFile(
            rd_rds::file::ReadError::CompressedSizeLimitExceeded { limit: 1 }
        ))
    ));
}

// Exercise the same consumer code as the runnable example.
#[allow(dead_code)]
#[path = "../examples/help_with_fallback.rs"]
mod consumer;

#[test]
fn consumer_preserves_titles_without_help_and_after_topic_decode_failure() {
    let pkg = Package::new();
    pkg.metadata("help_topics_metadata_v3.rds");
    assert!(!pkg.0.join("help").exists());
    let help = consumer::lookup(&pkg.0, "shared").unwrap().unwrap();
    assert_eq!(help.title.as_deref(), Some("First topic title"));
    assert!(help.document.is_none());
    assert!(consumer::lookup(&pkg.0, "unknown").unwrap().is_none());

    pkg.help();
    fs::copy(
        fixture("aliases_vector_dup_v3.rds"),
        pkg.0.join("help/aliases.rds"),
    )
    .unwrap();
    let db = PackageHelpDb::open(&pkg.0).unwrap();
    assert_eq!(db.resolve_alias("shared").unwrap(), Some("second-topic"));
    let help = consumer::lookup(&pkg.0, "shared").unwrap().unwrap();
    assert_eq!(help.title.as_deref(), Some("First topic title"));
    assert!(help.document.is_some());
    let help = consumer::lookup(&pkg.0, "title-only").unwrap().unwrap();
    assert_eq!(help.title.as_deref(), Some("Title without a page"));
    assert!(help.document.is_none());

    // Preserve the stored range but make the selected RDS record undecodable.
    let path = pkg.0.join("help/fixturepkg.rdb");
    let length = fs::metadata(&path).unwrap().len() as usize;
    fs::write(&path, vec![0; length]).unwrap();
    let db = PackageHelpDb::open(&pkg.0).unwrap();
    assert!(db.raw_topic("first-topic").is_err());
    let help = consumer::lookup(&pkg.0, "shared").unwrap().unwrap();
    assert_eq!(help.title.as_deref(), Some("First topic title"));
    assert!(help.document.is_none());
}
