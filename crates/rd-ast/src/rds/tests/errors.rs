use super::*;

#[test]
fn rejects_non_list_roots() {
    let root = fixture("rd_minimal_v2.rds");
    let non_list = root.attributes().get("class").expect("class attr");
    assert!(matches!(non_list.value(), RValue::Character(_)));
    let error = lower_r_object(non_list).unwrap_err();
    assert!(matches!(error, LowerError::RootMustBeList { .. }));
    assert!(error.location().path().segments().is_empty());
    assert_eq!(
        error.to_string(),
        "root Rd object must be a list at document root"
    );
}

#[test]
fn invalid_string_in_text_leaf_reports_node_character_location() {
    let node = RObject::from_parts(RValue::Character(vec![RStr::Na, invalid_string()]), Attributes::new(vec![attribute(
            "Rd_tag",
            RValue::Character(vec![RStr::Value {
                bytes: Arc::from(&b"TEXT"[..]),
                encoding: REncoding::Utf8,
                native_encoding: None,
            }]),
        )]));
    let error = lower_r_object(&bare(RValue::List(vec![node]))).unwrap_err();

    assert_eq!(
        error.location().path().segments(),
        &[
            RdPathSegment::TopLevel(0),
            RdPathSegment::CharacterElement(1),
        ]
    );
    assert_eq!(error.location().tag(), Some("TEXT"));
    assert_eq!(error.location().attribute(), None);
    assert_eq!(error.encoding(), Some(StringEncoding::Bytes));
    assert_eq!(
        error.to_string(),
        "invalid Bytes string encoding at top-level[0] / character[1] (TEXT)"
    );
}

#[test]
fn invalid_string_in_option_reports_option_context() {
    let node = RObject::from_parts(RValue::Null, Attributes::new(vec![
            attribute(
                "Rd_tag",
                RValue::Character(vec![RStr::Value {
                    bytes: Arc::from(&br"\arguments"[..]),
                    encoding: REncoding::Utf8,
                    native_encoding: None,
                }]),
            ),
            attribute("Rd_option", RValue::Character(vec![invalid_string()])),
        ]));
    let error = lower_r_object(&bare(RValue::List(vec![node]))).unwrap_err();

    assert_eq!(
        error.location().path().segments(),
        &[
            RdPathSegment::TopLevel(0),
            RdPathSegment::Option,
            RdPathSegment::CharacterElement(0),
        ]
    );
    assert_eq!(error.location().tag(), Some(r"\arguments"));
    assert_eq!(error.location().attribute(), Some("Rd_option"));
}

#[test]
fn invalid_string_in_raw_attribute_reports_attribute_context() {
    let node = RObject::from_parts(RValue::Null, Attributes::new(vec![
            attribute(
                "Rd_tag",
                RValue::Character(vec![RStr::Value {
                    bytes: Arc::from(&b"TEXT"[..]),
                    encoding: REncoding::Utf8,
                    native_encoding: None,
                }]),
            ),
            attribute("rootextra", RValue::Character(vec![invalid_string()])),
        ]));
    let error = lower_r_object(&bare(RValue::List(vec![node]))).unwrap_err();

    assert_eq!(
        error.location().path().segments(),
        &[
            RdPathSegment::TopLevel(0),
            RdPathSegment::Attribute("rootextra".to_string()),
            RdPathSegment::AttributeValue,
            RdPathSegment::CharacterElement(0),
        ]
    );
    assert_eq!(error.location().tag(), Some("TEXT"));
    assert_eq!(error.location().attribute(), Some("rootextra"));
}

#[test]
fn invalid_string_in_rd_tag_reports_tag_attribute_context() {
    let node = RObject::from_parts(RValue::Null, Attributes::new(vec![attribute(
            "Rd_tag",
            RValue::Character(vec![invalid_string()]),
        )]));
    let error = lower_r_object(&bare(RValue::List(vec![node]))).unwrap_err();

    assert_eq!(
        error.location().path().segments(),
        &[
            RdPathSegment::TopLevel(0),
            RdPathSegment::CharacterElement(0),
        ]
    );
    assert_eq!(error.location().tag(), None);
    assert_eq!(error.location().attribute(), Some("Rd_tag"));
}
