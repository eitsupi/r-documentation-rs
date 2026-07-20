//! Same-version serde JSON round-trip and structural-shape coverage.

#![cfg(feature = "serde")]

use rd_ast::{
    RawRdEnvironment, RawRdReal, RawRdValue, RdAttribute, RdDocument, RdGroup, RdNode, RdTag,
    RdTagged, producer,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn round_trip<T>(original: &T) -> Value
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_value(original).expect("serialize");
    let restored: T = serde_json::from_value(json.clone()).expect("deserialize");
    assert_eq!(original, &restored);
    json
}

fn raw_attribute(name: &str, value: RawRdValue) -> RdAttribute {
    producer::raw_attribute(name.to_string(), producer::raw_object(value, Vec::new()))
}

fn raw_node(option: Option<Vec<RdNode>>) -> rd_ast::RawRdNode {
    producer::raw_node(
        Some("\u{005c}unexpected".to_string()),
        option,
        vec![RdNode::Text("child".to_string())],
        None,
        vec![raw_attribute(
            "class",
            RawRdValue::Character(vec![Some("raw".to_string()), None]),
        )],
    )
}

fn raw_payload(value: RawRdValue) -> rd_ast::RawRdNode {
    producer::raw_node(None, None, vec![], Some(value), vec![])
}

fn sample_document() -> RdDocument {
    RdDocument::new(vec![
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("example".to_string())]),
        RdNode::Text("\n".to_string()),
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![
                RdNode::Text("See ".to_string()),
                RdNode::tagged(
                    RdTag::Link,
                    Some(vec![RdNode::Text("base".to_string())]),
                    vec![RdNode::Text("print".to_string())],
                ),
                RdNode::RCode("1 + 1".to_string()),
                RdNode::Verb("literal   text".to_string()),
                RdNode::Comment("% a comment".to_string()),
                RdNode::tagged(RdTag::Unknown(r"\madeUpMacro".to_string()), None, vec![]),
                RdNode::group(vec![
                    RdNode::Text("grouped ".to_string()),
                    RdNode::group(vec![RdNode::Text("content".to_string())]),
                ]),
                RdNode::Raw(producer::raw_node(
                    None,
                    None,
                    vec![RdNode::Text("genuine raw".to_string())],
                    None,
                    vec![producer::raw_attribute(
                        "extra".to_string(),
                        producer::raw_object(
                            RawRdValue::List(vec![producer::raw_object(
                                RawRdValue::Character(vec![
                                    Some("a".to_string()),
                                    Some("b".to_string()),
                                ]),
                                vec![producer::raw_attribute(
                                    "nested".to_string(),
                                    producer::raw_object(RawRdValue::Null, Vec::new()),
                                )],
                            )]),
                            Vec::new(),
                        ),
                    )],
                )),
            ],
        ),
    ])
}

#[test]
fn document_json_round_trip() {
    round_trip(&sample_document());
}

#[test]
fn every_rd_node_variant_round_trips_directly() {
    let cases = [
        RdNode::Text("text".to_string()),
        RdNode::RCode("x <- 1".to_string()),
        RdNode::Verb("verbatim".to_string()),
        RdNode::Comment("% comment".to_string()),
        RdNode::tagged(RdTag::Code, None, vec![RdNode::Text("x".to_string())]),
        RdNode::group(vec![RdNode::Text("group".to_string())]),
        RdNode::Raw(raw_node(None)),
    ];
    for node in &cases {
        round_trip(node);
    }
}

#[test]
fn every_serde_public_type_round_trips_directly() {
    let document = sample_document();
    let tagged = RdTagged::new(RdTag::Title, None, vec![RdNode::Text("title".to_string())]);
    let group = RdGroup::new(vec![RdNode::Text("group".to_string())]);
    let attribute = raw_attribute("value", RawRdValue::Integer(vec![Some(1), None]));
    let object = producer::raw_object(
        RawRdValue::List(vec![producer::raw_object(
            RawRdValue::Null,
            vec![attribute.clone()],
        )]),
        vec![attribute.clone()],
    );
    let raw = producer::raw_node(None, None, vec![], None, vec![attribute.clone()]);

    round_trip(&document); // RdDocument
    round_trip(&RdNode::Text("node".to_string())); // RdNode
    round_trip(&tagged); // RdTagged
    round_trip(&group); // RdGroup
    round_trip(&RdTag::Unknown(r"\custom".to_string())); // RdTag
    round_trip(&raw); // RawRdNode
    round_trip(&attribute); // RdAttribute
    round_trip(&object); // RawRdObject
    round_trip(&RawRdValue::Symbol("name".to_string())); // RawRdValue
    round_trip(&RawRdEnvironment::Other); // RawRdEnvironment
    round_trip(&RawRdReal::Finite(1.25)); // RawRdReal
}

#[test]
fn tag_json_round_trip_including_unknown() {
    for tag in RdTag::KNOWN {
        round_trip(tag);
    }
    let unknown = RdTag::Unknown(r"\madeUpMacro".to_string());
    assert_eq!(round_trip(&unknown), json!({"Unknown": r"\madeUpMacro"}));
}

#[test]
fn raw_values_cover_all_variants_and_vector_shapes() {
    let values = [
        RawRdValue::Null,
        RawRdValue::Integer(vec![]),
        RawRdValue::Integer(vec![Some(1), None, Some(-2)]),
        RawRdValue::Logical(vec![]),
        RawRdValue::Logical(vec![Some(true), None, Some(false)]),
        RawRdValue::Real(vec![]),
        RawRdValue::Real(vec![RawRdReal::Finite(1.25)]),
        RawRdValue::Character(vec![]),
        RawRdValue::Character(vec![Some("a".to_string()), None, Some("b".to_string())]),
        RawRdValue::List(vec![]),
        RawRdValue::List(vec![producer::raw_object(
            RawRdValue::Character(vec![Some("nested".to_string())]),
            vec![raw_attribute("nested", RawRdValue::Null)],
        )]),
        RawRdValue::Symbol("symbol".to_string()),
        RawRdValue::Persisted(vec![]),
        RawRdValue::Persisted(vec![Some("ref".to_string()), None]),
        RawRdValue::Environment(RawRdEnvironment::Global),
    ];
    for value in &values {
        round_trip(value);
    }
}

#[test]
fn real_values_json_round_trip_without_collapsing_special_values() {
    for environment in [
        RawRdEnvironment::Global,
        RawRdEnvironment::Base,
        RawRdEnvironment::Empty,
        RawRdEnvironment::Other,
    ] {
        round_trip(&environment);
    }

    let cases = [
        (
            RawRdReal::Finite(1.25),
            json!({"kind": "finite", "value": 1.25}),
        ),
        (RawRdReal::Na, json!({"kind": "na"})),
        (RawRdReal::Nan, json!({"kind": "nan"})),
        (
            RawRdReal::PositiveInfinity,
            json!({"kind": "positive_infinity"}),
        ),
        (
            RawRdReal::NegativeInfinity,
            json!({"kind": "negative_infinity"}),
        ),
    ];
    for (real, expected) in cases {
        assert_eq!(round_trip(&real), expected);
    }
}

#[test]
fn tagged_and_raw_option_tri_state_is_distinguishable() {
    let tagged = [
        RdNode::tagged(RdTag::Link, None, vec![]),
        RdNode::tagged(RdTag::Link, Some(vec![]), vec![]),
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("option".to_string())]),
            vec![],
        ),
    ];
    let tagged_json: Vec<_> = tagged.iter().map(round_trip).collect();
    assert_ne!(tagged_json[0], tagged_json[1]);
    assert_ne!(tagged_json[1], tagged_json[2]);
    assert_eq!(tagged_json[0]["Tagged"]["option"], Value::Null);
    assert_eq!(tagged_json[1]["Tagged"]["option"], json!([]));

    let raw = [
        RdNode::Raw(raw_node(None)),
        RdNode::Raw(raw_node(Some(vec![]))),
        RdNode::Raw(raw_node(Some(vec![RdNode::Text("option".to_string())]))),
    ];
    let raw_json: Vec<_> = raw.iter().map(round_trip).collect();
    assert_ne!(raw_json[0], raw_json[1]);
    assert_ne!(raw_json[1], raw_json[2]);
    assert_eq!(raw_json[0]["Raw"]["option"], Value::Null);
    assert_eq!(raw_json[1]["Raw"]["option"], json!([]));
}

#[test]
fn groups_round_trip_empty_content_and_nested() {
    for group in [
        RdGroup::new(vec![]),
        RdGroup::new(vec![RdNode::Text("content".to_string())]),
        RdGroup::new(vec![RdNode::group(vec![RdNode::Text(
            "nested".to_string(),
        )])]),
    ] {
        round_trip(&group);
    }
}

#[test]
fn structural_shapes_pin_node_and_raw_layout() {
    assert_eq!(
        round_trip(&RdNode::Group(RdGroup::new(vec![RdNode::Text(
            "child".to_string()
        )]))),
        json!({"Group": {"children": [{"Text": "child"}]}})
    );

    let raw = RdNode::Raw(raw_node(Some(vec![])));
    let value = round_trip(&raw);
    assert_eq!(value["Raw"]["tag"], json!(r"\unexpected"));
    assert_eq!(value["Raw"]["option"], json!([]));
    assert_eq!(value["Raw"]["children"], json!([{"Text": "child"}]));
    assert_eq!(value["Raw"]["payload"], Value::Null);
    assert_eq!(
        value["Raw"]["attributes"][0]["value"]["value"],
        json!({"Character": ["raw", null]})
    );
}

#[test]
fn raw_payload_round_trips_and_is_present_on_the_wire() {
    for value in [
        RawRdValue::Integer(vec![Some(1), None]),
        RawRdValue::Real(vec![
            RawRdReal::Na,
            RawRdReal::Nan,
            RawRdReal::PositiveInfinity,
            RawRdReal::NegativeInfinity,
        ]),
        RawRdValue::Symbol("symbol".into()),
    ] {
        let json = round_trip(&raw_payload(value));
        assert!(json.get("payload").is_some());
        assert_ne!(json["payload"], Value::Null);
    }
}
