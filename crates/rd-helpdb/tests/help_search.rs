//! Consumer contracts for installed help-search metadata, independent of R.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use rd_helpdb::{Error, HelpSearchIndex, read_rds_file};
use rd_rds::{Attribute, Attributes, RObject, RStr, RValue, Symbol};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data")
        .join(name)
}

fn object(value: RValue) -> RObject {
    RObject::from_parts(value, Attributes::default())
}

fn text(value: &str) -> RStr {
    RStr::new(
        value.as_bytes(),
        rd_rds::REncoding::Utf8,
        rd_rds::NativeEncodingSource::Unknown,
    )
}

fn valid() -> RObject {
    read_rds_file(fixture("help_search_valid_v3.rds")).unwrap()
}

fn replace_matrix_names(root: RObject, matrix_index: usize, names: Vec<RStr>) -> RObject {
    let (RValue::List(mut matrices), attributes) = root.into_parts() else {
        panic!("list")
    };
    let matrix = matrices[matrix_index].clone();
    let (value, matrix_attributes) = matrix.into_parts();
    let matrix_attributes = matrix_attributes
        .iter()
        .filter(|attribute| attribute.name().as_str() != "dimnames")
        .cloned()
        .chain([Attribute::new(
            Symbol::new("dimnames"),
            object(RValue::List(vec![
                object(RValue::Null),
                object(RValue::Character(names)),
            ])),
        )])
        .collect();
    matrices[matrix_index] = RObject::from_parts(value, Attributes::new(matrix_attributes));
    RObject::from_parts(RValue::List(matrices), attributes)
}

fn replace_matrix(root: RObject, matrix_index: usize, matrix: RObject) -> RObject {
    let (RValue::List(mut matrices), attributes) = root.into_parts() else {
        panic!("list")
    };
    matrices[matrix_index] = matrix;
    RObject::from_parts(RValue::List(matrices), attributes)
}

fn without_matrix_dimnames(root: RObject, matrix_index: usize) -> RObject {
    let (RValue::List(mut matrices), attributes) = root.into_parts() else {
        panic!("list")
    };
    let (value, matrix_attributes) = matrices[matrix_index].clone().into_parts();
    let matrix_attributes = matrix_attributes
        .iter()
        .filter(|attribute| attribute.name().as_str() != "dimnames")
        .cloned()
        .collect();
    matrices[matrix_index] = RObject::from_parts(value, Attributes::new(matrix_attributes));
    RObject::from_parts(RValue::List(matrices), attributes)
}

fn replace_first_cell_with_invalid_utf8(root: RObject, matrix_index: usize) -> RObject {
    let (RValue::List(mut matrices), attributes) = root.into_parts() else {
        panic!("list")
    };
    let (value, matrix_attributes) = matrices[matrix_index].clone().into_parts();
    let RValue::Character(mut values) = value else {
        panic!("character matrix")
    };
    values[0] = RStr::new(
        &[0xff],
        rd_rds::REncoding::Utf8,
        rd_rds::NativeEncodingSource::Unknown,
    );
    matrices[matrix_index] = RObject::from_parts(RValue::Character(values), matrix_attributes);
    RObject::from_parts(RValue::List(matrices), attributes)
}

fn named_root(root: RObject) -> RObject {
    let (value, attributes) = root.into_parts();
    let attributes = attributes
        .iter()
        .filter(|attribute| attribute.name().as_str() != "names")
        .cloned()
        .chain([Attribute::new(
            Symbol::new("names"),
            object(RValue::Character(vec![
                text("base"),
                text("aliases"),
                text("keywords"),
                text("concepts"),
            ])),
        )])
        .collect();
    RObject::from_parts(value, Attributes::new(attributes))
}

fn root_with_invalid_names_attribute(root: RObject) -> RObject {
    let (value, attributes) = root.into_parts();
    let attributes = attributes
        .iter()
        .filter(|attribute| attribute.name().as_str() != "names")
        .cloned()
        .chain([Attribute::new(
            Symbol::new("names"),
            object(RValue::Integer(vec![Some(1)])),
        )])
        .collect();
    RObject::from_parts(value, Attributes::new(attributes))
}

#[test]
fn reads_rows_in_stored_order_and_preserves_na_duplicates_and_empty_strings() {
    for version in [2, 3] {
        let root = read_rds_file(fixture(&format!("help_search_valid_v{version}.rds"))).unwrap();
        let index = HelpSearchIndex::from_object(&root).unwrap();
        let base: Vec<_> = index
            .base_entries()
            .map(|entry| {
                [
                    entry.package.as_deref(),
                    entry.lib_path.as_deref(),
                    entry.id.as_deref(),
                    entry.name.as_deref(),
                    entry.title.as_deref(),
                    entry.topic.as_deref(),
                    entry.encoding.as_deref(),
                ]
            })
            .collect();
        assert_eq!(
            base,
            vec![
                [
                    Some("fixturepkg"),
                    Some(""),
                    Some("1"),
                    Some("first"),
                    Some("First title"),
                    Some("first"),
                    Some("UTF-8")
                ],
                [
                    Some("fixturepkg"),
                    Some(""),
                    Some("2"),
                    Some("second"),
                    Some(""),
                    None,
                    Some("UTF-8")
                ],
                [
                    None,
                    Some(""),
                    Some("2"),
                    Some("second"),
                    Some("Second title"),
                    Some("second"),
                    None
                ],
                [
                    Some("fixturepkg"),
                    Some(""),
                    Some(""),
                    Some(""),
                    Some(""),
                    Some(""),
                    Some("")
                ],
            ]
        );
        assert_eq!(
            index
                .aliases()
                .map(|entry| [
                    entry.alias.as_deref(),
                    entry.id.as_deref(),
                    entry.package.as_deref()
                ])
                .collect::<Vec<_>>(),
            vec![
                [Some("first"), Some("1"), Some("fixturepkg")],
                [Some("first"), Some("1"), Some("fixturepkg")],
                [None, Some("2"), Some("fixturepkg")],
                [Some(""), None, Some("")],
            ]
        );
        assert_eq!(
            index
                .keywords()
                .map(|entry| [
                    entry.keyword.as_deref(),
                    entry.id.as_deref(),
                    entry.package.as_deref()
                ])
                .collect::<Vec<_>>(),
            vec![
                [Some("keyword"), Some("1"), Some("fixturepkg")],
                [Some("keyword"), Some("1"), Some("fixturepkg")],
                [None, Some("2"), Some("fixturepkg")],
                [Some(""), None, Some("")],
            ],
        );
        assert_eq!(
            index
                .concepts()
                .map(|entry| [
                    entry.concept.as_deref(),
                    entry.id.as_deref(),
                    entry.package.as_deref()
                ])
                .collect::<Vec<_>>(),
            vec![
                [Some("concept"), Some("1"), Some("fixturepkg")],
                [None, Some("2"), Some("fixturepkg")],
                [Some(""), Some(""), Some("")],
            ]
        );
        assert_eq!(index.aliases().len(), 4);
        assert_eq!(index.keywords().len(), 4);
        assert_eq!(index.concepts().len(), 3);
        assert!(!index.is_empty());
        assert_eq!(HelpSearchIndex::try_from(&root).unwrap(), index);
    }
}

#[test]
fn reads_known_legacy_schema_and_fills_missing_encoding_like_r() {
    let index =
        HelpSearchIndex::from_object(&read_rds_file(fixture("help_search_legacy_v3.rds")).unwrap())
            .unwrap();
    assert_eq!(
        index.base_entries().next().unwrap().name.as_deref(),
        Some("first")
    );
    assert!(
        index
            .base_entries()
            .all(|entry| entry.encoding == Some(String::new()))
    );
    assert_eq!(
        index.aliases().next().unwrap().alias.as_deref(),
        Some("first")
    );

    let index = HelpSearchIndex::from_object(
        &read_rds_file(fixture("help_search_legacy_encoding_v3.rds")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        index.base_entries().next().unwrap().encoding.as_deref(),
        Some("UTF-8")
    );
}

#[test]
fn resolves_reordered_matrix_columns_by_name() {
    let original = HelpSearchIndex::from_object(&valid()).unwrap();
    let reordered = HelpSearchIndex::from_object(
        &read_rds_file(fixture("help_search_reordered_v3.rds")).unwrap(),
    )
    .unwrap();
    assert_eq!(reordered, original);
}

#[test]
fn reads_installed_package_fixture_with_utf8_metadata() {
    let index = HelpSearchIndex::from_object(
        &read_rds_file(fixture("help_search_installed_package_v3.rds")).unwrap(),
    )
    .unwrap();
    assert!(
        index
            .base_entries()
            .any(|entry| entry.title.as_deref() == Some("UTF-8 café topic"))
    );
    assert!(
        index
            .concepts()
            .any(|entry| entry.concept.as_deref() == Some("café"))
    );
}

#[test]
fn decodes_latin1_cells_to_rust_utf8() {
    let index =
        HelpSearchIndex::from_object(&read_rds_file(fixture("help_search_latin1_v3.rds")).unwrap())
            .unwrap();
    assert_eq!(
        index.base_entries().next().unwrap().title.as_deref(),
        Some("café")
    );
}

#[test]
fn accepts_a_valid_empty_index() {
    let index =
        HelpSearchIndex::from_object(&read_rds_file(fixture("help_search_empty_v3.rds")).unwrap())
            .unwrap();
    assert!(index.is_empty());
    assert_eq!(index.base_entries().len(), 0);
    assert_eq!(index.aliases().len(), 0);
    assert_eq!(index.keywords().len(), 0);
    assert_eq!(index.concepts().len(), 0);
}

#[test]
fn rejects_unknown_duplicate_or_missing_column_names_and_wrong_roots() {
    let root = valid();
    let error = HelpSearchIndex::from_object(&replace_matrix_names(
        root.clone(),
        0,
        vec![
            text("Package"),
            text("LibPath"),
            text("ID"),
            text("Name"),
            text("Title"),
            text("Topic"),
            text("Unknown"),
        ],
    ))
    .unwrap_err();
    assert!(
        matches!(error, Error::MalformedIndex(message) if message.contains("unsupported columns"))
    );

    let error = HelpSearchIndex::from_object(&replace_matrix_names(
        root.clone(),
        1,
        vec![text("Alias"), text("ID"), text("Alias")],
    ))
    .unwrap_err();
    assert!(matches!(error, Error::MalformedIndex(message) if message.contains("duplicate")));

    let error = HelpSearchIndex::from_object(&replace_matrix_names(
        root,
        2,
        vec![text("Keyword"), text("ID"), RStr::Na],
    ))
    .unwrap_err();
    assert!(matches!(error, Error::MalformedIndex(message) if message.contains("missing or NA")));

    let error = HelpSearchIndex::from_object(&replace_matrix(
        valid(),
        0,
        object(RValue::Integer(vec![Some(1)])),
    ))
    .unwrap_err();
    assert!(
        matches!(error, Error::MalformedIndex(message) if message.contains("character vector"))
    );

    let wrong_dimensions = RObject::from_parts(
        RValue::Character(vec![]),
        Attributes::new(vec![
            Attribute::new(
                Symbol::new("dim"),
                object(RValue::Integer(vec![Some(1), Some(1)])),
            ),
            Attribute::new(
                Symbol::new("dimnames"),
                object(RValue::List(vec![
                    object(RValue::Null),
                    object(RValue::Character(vec![text("Package")])),
                ])),
            ),
        ]),
    );
    let error =
        HelpSearchIndex::from_object(&replace_matrix(valid(), 0, wrong_dimensions)).unwrap_err();
    assert!(
        matches!(error, Error::MalformedIndex(message) if message.contains("unexpected length"))
    );

    let error = HelpSearchIndex::from_object(&replace_first_cell_with_invalid_utf8(valid(), 0))
        .unwrap_err();
    assert!(
        matches!(error, Error::MalformedIndex(message) if message.contains("invalid string encoding"))
    );

    let error = HelpSearchIndex::from_object(&without_matrix_dimnames(valid(), 0)).unwrap_err();
    assert!(matches!(error, Error::MalformedIndex(message) if message.contains("missing or NA")));

    let matrix = replace_matrix(
        valid(),
        0,
        RObject::from_parts(
            RValue::Character(vec![]),
            Attributes::new(vec![
                Attribute::new(
                    Symbol::new("dim"),
                    object(RValue::Integer(vec![Some(0), Some(8)])),
                ),
                Attribute::new(
                    Symbol::new("dimnames"),
                    object(RValue::List(vec![
                        object(RValue::Null),
                        object(RValue::Character(vec![
                            text("Package"),
                            text("LibPath"),
                            text("ID"),
                            text("Name"),
                            text("Title"),
                            text("Topic"),
                            text("Encoding"),
                            text("Extra"),
                        ])),
                    ])),
                ),
            ]),
        ),
    );
    let error = HelpSearchIndex::from_object(&matrix).unwrap_err();
    assert!(
        matches!(error, Error::MalformedIndex(message) if message.contains("unsupported columns"))
    );

    for root in [
        object(RValue::Null),
        object(RValue::List(vec![])),
        named_root(valid()),
        root_with_invalid_names_attribute(valid()),
    ] {
        assert!(matches!(
            HelpSearchIndex::from_object(&root),
            Err(Error::MalformedIndex(_))
        ));
    }
}

struct Package(PathBuf);

impl Package {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir()
            .join(format!(
                "rd-helpdb-search-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ))
            .join("fixturepkg");
        fs::create_dir_all(path.join("Meta")).unwrap();
        Self(path)
    }

    fn search(&self, name: &str) {
        fs::copy(fixture(name), self.0.join("Meta/hsearch.rds")).unwrap();
    }
}

impl Drop for Package {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

#[test]
fn installed_reader_distinguishes_missing_empty_invalid_and_options() {
    let pkg = Package::new();
    assert!(HelpSearchIndex::read_installed(&pkg.0).unwrap().is_none());
    pkg.search("help_search_empty_v3.rds");
    assert!(
        HelpSearchIndex::read_installed(&pkg.0)
            .unwrap()
            .unwrap()
            .is_empty()
    );
    pkg.search("help_search_valid_v3.rds");
    let options = rd_rds::file::ReadOptions::default().max_compressed_bytes(1);
    assert!(matches!(
        HelpSearchIndex::read_installed_with_options(&pkg.0, &options),
        Err(Error::RdsFile(
            rd_rds::file::ReadError::CompressedSizeLimitExceeded { limit: 1 }
        ))
    ));
    let options = rd_rds::file::ReadOptions::default().max_decompressed_bytes(1);
    assert!(matches!(
        HelpSearchIndex::read_installed_with_options(&pkg.0, &options),
        Err(Error::RdsFile(
            rd_rds::file::ReadError::DecompressedSizeLimitExceeded { limit: 1 }
        ))
    ));
    let options = rd_rds::file::ReadOptions::default()
        .limits(rd_rds::Limits::default().max_total_elements(1));
    assert!(matches!(
        HelpSearchIndex::read_installed_with_options(&pkg.0, &options),
        Err(Error::Rds(rd_rds::Error::TotalElementsLimitExceeded {
            limit: 1,
            ..
        }))
    ));
    fs::write(pkg.0.join("Meta/hsearch.rds"), b"not RDS").unwrap();
    assert!(HelpSearchIndex::read_installed(&pkg.0).is_err());
    fs::remove_file(pkg.0.join("Meta/hsearch.rds")).unwrap();
    fs::create_dir(pkg.0.join("Meta/hsearch.rds")).unwrap();
    assert!(matches!(
        HelpSearchIndex::read_installed(&pkg.0),
        Err(Error::Io { .. })
    ));
}
