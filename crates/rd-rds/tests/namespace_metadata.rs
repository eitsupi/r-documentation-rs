use std::{collections::BTreeSet, path::PathBuf};

use rd_rds::{
    Attribute, Attributes, NativeEncodingSource, REncoding, RObject, RStr, RValue, Symbol,
    package::{MetadataField, NamespaceExport, NamespaceImport, NamespaceMetadata, S3MethodName},
};

fn fixture(version: &str) -> RObject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/namespace")
        .join(format!("namespace-{version}.rds"));
    rd_rds::file::read(path).expect("namespace fixture")
}

#[cfg(feature = "gzip")]
fn installed_fixture() -> RObject {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/data/namespace-installed/namespace-installed.rds");
    rd_rds::file::read(path).expect("installed namespace fixture")
}

#[test]
fn v2_and_v3_fixtures_have_the_same_owned_metadata() {
    let v2 = NamespaceMetadata::try_from(&fixture("v2")).expect("v2 namespace metadata");
    let v3 = NamespaceMetadata::try_from(&fixture("v3")).expect("v3 namespace metadata");
    assert_eq!(v2, v3);

    assert_eq!(
        v2.declared_exports(),
        &MetadataField::Present(vec![
            NamespaceExport::new("alpha", "alpha"),
            NamespaceExport::new("alpha", "alpha"),
            NamespaceExport::new("beta", "beta"),
        ])
    );
    assert_eq!(v2.export_patterns(), &MetadataField::Present(Vec::new()));
    assert_eq!(
        v2.export_classes(),
        &MetadataField::Present(vec!["Widget".into(), "Widget".into()])
    );
    assert_eq!(
        v2.export_class_patterns(),
        &MetadataField::Present(vec!["^Widget".into()])
    );

    let MetadataField::Present(imports) = v2.imports() else {
        panic!("imports should be present");
    };
    assert_eq!(
        imports,
        &vec![
            NamespaceImport::All {
                package: "base".into(),
                except: vec![],
            },
            NamespaceImport::From {
                package: "stats".into(),
                names: vec![
                    rd_rds::package::ImportedName::new("mean", "mean"),
                    rd_rds::package::ImportedName::new("median", "average"),
                ],
            },
            NamespaceImport::All {
                package: "utils".into(),
                except: vec!["head".into(), "tail".into()],
            },
        ]
    );

    let MetadataField::Present(registrations) = v2.s3_registrations() else {
        panic!("S3 registrations should be present");
    };
    assert_eq!(registrations.len(), 2);
    assert!(matches!(registrations[0].method(), S3MethodName::Implicit));
    assert!(
        matches!(registrations[1].method(), S3MethodName::Explicit(name) if name == "format.widget")
    );
    assert_eq!(registrations[0].generic_package(), Some("base"));
    assert_eq!(registrations[1].generic_package(), Some("utils"));
    assert_eq!(
        v2.s3_generic_evidence(),
        &MetadataField::Present(vec!["print".into(), "format".into()])
    );
}

#[test]
#[cfg(feature = "gzip")]
fn installed_package_metadata_preserves_aliases_and_r_import_shapes() {
    let metadata =
        NamespaceMetadata::try_from(&installed_fixture()).expect("installed namespace metadata");
    assert_eq!(
        metadata.declared_exports(),
        &MetadataField::Present(vec![
            NamespaceExport::new("internal_name", "public_name"),
            NamespaceExport::new("ordinary_name", "ordinary_name"),
        ])
    );
    assert_eq!(
        metadata.imports(),
        &MetadataField::Present(vec![
            NamespaceImport::All {
                package: "utils".into(),
                except: vec!["head".into(), "tail".into()],
            },
            NamespaceImport::From {
                package: "stats".into(),
                names: vec![rd_rds::package::ImportedName::new("median", "mean")],
            },
        ])
    );
    let MetadataField::Present(registrations) = metadata.s3_registrations() else {
        panic!("installed S3 registrations should be present");
    };
    assert_eq!(registrations.len(), 1);
    assert_eq!(registrations[0].generic(), "print");
    assert_eq!(registrations[0].class(), "namespacefixture");
    assert!(matches!(registrations[0].method(), S3MethodName::Implicit));
    assert_eq!(registrations[0].generic_package(), None);
}

#[test]
fn generic_evidence_can_be_used_without_claiming_complete_registrations() {
    let object = named_list(
        &["S3methods"],
        vec![character_matrix(1, 3, &["print", "widget", "print.widget"])],
    );
    let metadata = NamespaceMetadata::from_object(&object).expect("three-column metadata");
    assert!(matches!(
        metadata.s3_registrations(),
        MetadataField::UnsupportedSchema { .. }
    ));
    assert_eq!(
        metadata.s3_generic_evidence(),
        &MetadataField::Present(vec!["print".into()])
    );

    let evidence: BTreeSet<_> = match metadata.s3_generic_evidence() {
        MetadataField::Present(values) => values.iter().cloned().collect(),
        _ => panic!("evidence should be present"),
    };
    assert_eq!(evidence, BTreeSet::from(["print".to_owned()]));
}

#[test]
fn fields_are_independent_and_distinguish_missing_empty_invalid_and_unknown() {
    let object = named_list(
        &["exports", "exports", "exportPatterns", "unknownField"],
        vec![
            character_vector(&["first"]),
            character_vector(&["second"]),
            character_vector(&[]),
            RObject::from_parts(RValue::Null, Attributes::default()),
        ],
    );
    let metadata = NamespaceMetadata::from_object(&object).expect("field-local diagnostics");
    assert!(matches!(
        metadata.declared_exports(),
        MetadataField::Invalid(_)
    ));
    assert_eq!(
        metadata.export_patterns(),
        &MetadataField::Present(Vec::new())
    );
    assert!(matches!(metadata.imports(), MetadataField::Missing));
    assert!(matches!(
        metadata.s3_registrations(),
        MetadataField::Missing
    ));
}

#[test]
fn named_imports_reject_ambiguous_or_unknown_shapes() {
    let unknown = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["package", "mystery"],
            vec![character_vector(&["stats"]), character_vector(&["mean"])],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&unknown).expect("metadata root");
    assert!(matches!(
        metadata.imports(),
        MetadataField::UnsupportedSchema { .. }
    ));

    let duplicate = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["package", "package"],
            vec![character_vector(&["stats"]), character_vector(&["utils"])],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&duplicate).expect("metadata root");
    assert!(matches!(metadata.imports(), MetadataField::Invalid(_)));

    let ambiguous = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["package", "except", "selections"],
            vec![
                character_vector(&["stats"]),
                character_vector(&["mean"]),
                character_vector(&["median"]),
            ],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&ambiguous).expect("metadata root");
    assert!(matches!(metadata.imports(), MetadataField::Invalid(_)));
}

#[test]
fn imports_accept_r_names_for_an_unnamed_package_field() {
    let object = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["", "except"],
            vec![
                character_vector(&["utils"]),
                character_vector(&["head", "tail"]),
            ],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&object).expect("metadata root");
    assert_eq!(
        metadata.imports(),
        &MetadataField::Present(vec![NamespaceImport::All {
            package: "utils".into(),
            except: vec!["head".into(), "tail".into()],
        }])
    );

    let misplaced = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["except", ""],
            vec![character_vector(&["head"]), character_vector(&["utils"])],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&misplaced).expect("metadata root");
    assert!(matches!(
        metadata.imports(),
        MetadataField::UnsupportedSchema { .. }
    ));

    let multiple = named_list(
        &["imports"],
        vec![list_of(vec![named_list(
            &["", ""],
            vec![character_vector(&["utils"]), character_vector(&["stats"])],
        )])],
    );
    let metadata = NamespaceMetadata::from_object(&multiple).expect("metadata root");
    assert!(matches!(
        metadata.imports(),
        MetadataField::UnsupportedSchema { .. }
    ));
}

#[test]
fn exports_preserve_source_and_namespace_aliases() {
    let object = named_list(
        &["exports"],
        vec![named_character_vector(
            &["internal_name", "", "public_name"],
            &["source_name", "ordinary", "internal_name"],
        )],
    );
    let metadata = NamespaceMetadata::from_object(&object).expect("metadata root");
    assert_eq!(
        metadata.declared_exports(),
        &MetadataField::Present(vec![
            NamespaceExport::new("source_name", "internal_name"),
            NamespaceExport::new("ordinary", "ordinary"),
            NamespaceExport::new("internal_name", "public_name"),
        ])
    );
}

#[test]
fn malformed_export_names_are_field_local_invalid() {
    let wrong_type = named_list(
        &["exports"],
        vec![RObject::from_parts(
            RValue::Character(vec![RStr::new(
                b"name",
                REncoding::Native,
                NativeEncodingSource::Unknown,
            )]),
            Attributes::new(vec![Attribute::new(
                Symbol::new("names"),
                RObject::from_parts(RValue::Null, Attributes::default()),
            )]),
        )],
    );
    let metadata = NamespaceMetadata::from_object(&wrong_type).expect("metadata root");
    assert!(matches!(
        metadata.declared_exports(),
        MetadataField::Invalid(_)
    ));

    let invalid_names = named_list(
        &["exports", "exportPatterns"],
        vec![
            RObject::from_parts(
                RValue::Character(vec![
                    RStr::new(b"name", REncoding::Native, NativeEncodingSource::Unknown),
                    RStr::new(b"other", REncoding::Native, NativeEncodingSource::Unknown),
                ]),
                Attributes::new(vec![Attribute::new(
                    Symbol::new("names"),
                    RObject::from_parts(
                        RValue::Character(vec![RStr::Na, RStr::Na]),
                        Attributes::default(),
                    ),
                )]),
            ),
            character_vector(&[]),
        ],
    );
    let metadata = NamespaceMetadata::from_object(&invalid_names).expect("metadata root");
    assert!(matches!(
        metadata.declared_exports(),
        MetadataField::Invalid(_)
    ));
    assert_eq!(
        metadata.export_patterns(),
        &MetadataField::Present(Vec::new())
    );

    let invalid_encoding = named_list(
        &["exports"],
        vec![RObject::from_parts(
            RValue::Character(vec![RStr::new(
                b"source",
                REncoding::Native,
                NativeEncodingSource::Unknown,
            )]),
            Attributes::new(vec![Attribute::new(
                Symbol::new("names"),
                RObject::from_parts(
                    RValue::Character(vec![RStr::new(
                        b"\xff",
                        REncoding::Native,
                        NativeEncodingSource::Unknown,
                    )]),
                    Attributes::default(),
                ),
            )]),
        )],
    );
    let metadata = NamespaceMetadata::from_object(&invalid_encoding).expect("metadata root");
    assert!(matches!(
        metadata.declared_exports(),
        MetadataField::Invalid(_)
    ));
}

#[test]
fn malformed_root_is_a_constructor_error() {
    let scalar = character_vector(&["not a list"]);
    assert!(matches!(
        NamespaceMetadata::from_object(&scalar),
        Err(rd_rds::package::ViewError::UnexpectedType { .. })
    ));

    let unnamed = RObject::from_parts(
        RValue::List(vec![character_vector(&[])]),
        Attributes::default(),
    );
    assert!(matches!(
        NamespaceMetadata::from_object(&unnamed),
        Err(rd_rds::package::ViewError::Missing { path, .. })
            if path == "NamespaceMetadata.names"
    ));

    let mismatched = RObject::from_parts(
        RValue::List(vec![character_vector(&[])]),
        Attributes::new(vec![Attribute::new(
            Symbol::new("names"),
            character_vector(&[]),
        )]),
    );
    assert!(matches!(
        NamespaceMetadata::from_object(&mismatched),
        Err(rd_rds::package::ViewError::UnexpectedLength { path, .. })
            if path == "NamespaceMetadata"
    ));
}

#[test]
fn na_and_invalid_encoding_are_invalid_field_values() {
    let object = named_list(
        &["exports", "exportClasses"],
        vec![
            RObject::from_parts(RValue::Character(vec![RStr::Na]), Attributes::default()),
            RObject::from_parts(
                RValue::Character(vec![RStr::new(
                    b"\xff",
                    REncoding::Native,
                    NativeEncodingSource::Unknown,
                )]),
                Attributes::default(),
            ),
        ],
    );
    let metadata = NamespaceMetadata::from_object(&object).expect("invalid fields are retained");
    assert!(matches!(
        metadata.declared_exports(),
        MetadataField::Invalid(_)
    ));
    assert!(matches!(
        metadata.export_classes(),
        MetadataField::Invalid(_)
    ));
}

fn character_vector(values: &[&str]) -> RObject {
    RObject::from_parts(
        RValue::Character(
            values
                .iter()
                .map(|value| {
                    RStr::new(
                        value.as_bytes(),
                        REncoding::Native,
                        NativeEncodingSource::Unknown,
                    )
                })
                .collect(),
        ),
        Attributes::default(),
    )
}

fn named_character_vector(names: &[&str], values: &[&str]) -> RObject {
    RObject::from_parts(
        RValue::Character(
            values
                .iter()
                .map(|value| {
                    RStr::new(
                        value.as_bytes(),
                        REncoding::Native,
                        NativeEncodingSource::Unknown,
                    )
                })
                .collect(),
        ),
        Attributes::new(vec![Attribute::new(
            Symbol::new("names"),
            character_vector(names),
        )]),
    )
}

fn character_matrix(nrow: i32, ncol: i32, values: &[&str]) -> RObject {
    RObject::from_parts(
        RValue::Character(
            values
                .iter()
                .map(|value| {
                    RStr::new(
                        value.as_bytes(),
                        REncoding::Native,
                        NativeEncodingSource::Unknown,
                    )
                })
                .collect(),
        ),
        Attributes::new(vec![Attribute::new(
            Symbol::new("dim"),
            RObject::from_parts(
                RValue::Integer(vec![Some(nrow), Some(ncol)]),
                Attributes::default(),
            ),
        )]),
    )
}

fn named_list(names: &[&str], values: Vec<RObject>) -> RObject {
    RObject::from_parts(
        RValue::List(values),
        Attributes::new(vec![Attribute::new(
            Symbol::new("names"),
            character_vector(names),
        )]),
    )
}

fn list_of(values: Vec<RObject>) -> RObject {
    RObject::from_parts(RValue::List(values), Attributes::default())
}
