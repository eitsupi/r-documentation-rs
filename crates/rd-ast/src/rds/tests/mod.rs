use super::lower_r_object;
use super::*;
use crate::{RawRdEnvironment, RawRdReal, RawRdValue, RdNode, RdTag};

use std::{fs, io::Read, path::PathBuf, sync::Arc};

use super::attributes::{
    AttributeDisposition, classify_attribute, valid_expanded_fragment_class,
    valid_expanded_fragment_rdfile,
};
use super::context::{LowerContext, NodeContext};
use super::raw::lower_raw_value;
use flate2::read::GzDecoder;
use rd_rds::{Attribute, Attributes, EnvHandle, REncoding, RObject, RStr, RValue, parse};
fn fixture_dir() -> PathBuf {
    // These crate-local copies originate from crates/rd-rds/tests/fixtures/data/ and committed
    // R generator scripts; keep them byte-identical so packaged rd-ast tests run standalone.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
}

fn fixture(name: &str) -> RObject {
    let bytes = fs::read(fixture_dir().join(name)).expect("fixture bytes");
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("fixture gzip stream");
    parse(&decompressed).expect(name)
}

// A small hand-built R serialization containing paired and unpaired
// expanded-fragment roots. The children are positional groups, matching
// the shape found by the corpus probe.
fn expanded_fragment_fixture() -> RObject {
    const RDS: &[u8] = &[
        31, 139, 8, 0, 0, 0, 0, 0, 0, 3, 139, 224, 98, 96, 96, 96, 102, 96, 97, 99, 100, 96, 102,
        5, 50, 25, 88, 67, 67, 220, 116, 45, 128, 12, 97, 32, 102, 1, 202, 129, 104, 38, 40, 159,
        17, 136, 5, 192, 52, 11, 3, 39, 136, 174, 192, 37, 81, 9, 212, 204, 132, 36, 192, 22, 148,
        18, 95, 146, 152, 142, 166, 140, 61, 38, 51, 45, 53, 167, 56, 21, 77, 49, 107, 114, 78, 98,
        113, 49, 154, 90, 166, 160, 20, 76, 51, 211, 50, 115, 82, 209, 212, 241, 164, 167, 230,
        165, 22, 37, 150, 164, 166, 232, 97, 234, 40, 46, 74, 46, 74, 77, 3, 178, 120, 161, 142, 6,
        225, 127, 64, 195, 41, 242, 37, 227, 127, 156, 254, 2, 25, 142, 55, 8, 153, 75, 139, 114,
        112, 72, 177, 230, 36, 38, 165, 230, 224, 176, 130, 53, 38, 3, 236, 19, 144, 36, 19, 186,
        36, 60, 172, 152, 209, 101, 208, 66, 135, 160, 223, 41, 117, 30, 195, 63, 0, 124, 116, 120,
        254, 98, 2, 0, 0,
    ];
    let mut decoder = GzDecoder::new(RDS);
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("expanded fragment gzip stream");
    parse(&decompressed).expect("expanded fragment fixture")
}

fn bare(value: RValue) -> RObject {
    RObject::from_parts(value, Attributes::default())
}

fn bare_with_attribute(value: RValue, name: &str) -> RObject {
    let attribute = fixture("rd_extra_attr_v2.rds")
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == name)
        .cloned()
        .expect("fixture attribute");
    RObject::from_parts(
        value,
        Attributes::new(vec![Attribute::new(
            attribute.name().clone(),
            bare(RValue::Null),
        )]),
    )
}

fn lower_bare(value: RValue) -> RdNode {
    lower_r_object(&bare(RValue::List(vec![bare(value)])))
        .expect("lower bare node")
        .into_nodes()
        .into_iter()
        .next()
        .expect("one node")
}

fn tag_name(node: &RdNode) -> Option<&str> {
    match node {
        RdNode::Tagged(tagged) => Some(tagged.tag().as_rd_tag()),
        RdNode::Raw(raw) => raw.tag(),
        _ => None,
    }
}

fn walk_nodes<'a>(nodes: &'a [RdNode], f: &mut impl FnMut(&'a RdNode)) {
    for node in nodes {
        f(node);
        match node {
            RdNode::Tagged(tagged) => {
                if let Some(option) = tagged.option() {
                    walk_nodes(option, f);
                }
                walk_nodes(tagged.children(), f);
            }
            RdNode::Raw(raw) => {
                if let Some(option) = raw.option() {
                    walk_nodes(option, f);
                }
                walk_nodes(raw.children(), f);
            }
            RdNode::Group(group) => walk_nodes(group.children(), f),
            _ => {}
        }
    }
}

fn find_dynamic_node(object: &RObject, tag: &str) -> Option<RObject> {
    let mut context = LowerContext::new();
    if object.attributes().get("dynamicFlag").is_some()
        && rd_tag_string(&mut context, object)
            .ok()
            .flatten()
            .as_deref()
            == Some(tag)
    {
        return Some(object.clone());
    }

    let RValue::List(children) = object.value() else {
        return None;
    };
    children
        .iter()
        .find_map(|child| find_dynamic_node(child, tag))
}

fn dynamic_fixture_node(tag: &str) -> RObject {
    let root = fixture("rd_options_v2.rds");
    find_dynamic_node(&root, tag).expect("fixture dynamicFlag node")
}

fn with_value(template: &RObject, value: RValue) -> RObject {
    RObject::from_parts(value, template.attributes().clone())
}

fn raw_attribute_names(node: &RdNode) -> Vec<&str> {
    match node {
        RdNode::Raw(raw) => raw
            .attributes()
            .iter()
            .map(|attribute| attribute.name())
            .collect(),
        _ => Vec::new(),
    }
}

fn class_fixture_attribute() -> Attribute {
    let root = fixture("rd_extra_attr_v2.rds");
    root.attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "class")
        .expect("class attribute")
        .clone()
}

fn attribute_with_value(template: &Attribute, value: RValue) -> Attribute {
    Attribute::new(
        template.name().clone(),
        RObject::from_parts(value, Default::default()),
    )
}

fn fixture_attribute(name: &str) -> Attribute {
    fn find(object: &RObject, name: &str) -> Option<Attribute> {
        if let Some(attribute) = object
            .attributes()
            .iter()
            .find(|attribute| attribute.name().as_str() == name)
        {
            return Some(attribute.clone());
        }
        match object.value() {
            RValue::List(objects) => objects.iter().find_map(|object| find(object, name)),
            _ => None,
        }
    }

    [
        "rd_minimal_v2.rds",
        "rd_options_v2.rds",
        "rd_extra_attr_v2.rds",
    ]
    .into_iter()
    .find_map(|fixture_name| find(&fixture(fixture_name), name))
    .expect("fixture attribute")
}

fn invalid_string() -> RStr {
    RStr::Value {
        bytes: Arc::from(&b"\xff"[..]),
        encoding: REncoding::Bytes,
        native_encoding: None,
    }
}

fn attribute(name: &str, value: RValue) -> Attribute {
    attribute_with_value(&fixture_attribute(name), value)
}

fn with_nested_metadata_attributes(template: &RObject, metadata_names: &[&str]) -> RObject {
    let nested_attributes = fixture("rd_options_v2.rds")
        .attributes()
        .get("srcref")
        .expect("fixture srcref attribute")
        .attributes()
        .clone();
    let attributes = template
        .attributes()
        .iter()
        .map(|attribute| {
            let mut attribute = attribute.clone();
            if metadata_names
                .iter()
                .any(|name| *name == attribute.name().as_str())
            {
                let (value, _) = attribute.value().clone().into_parts();
                attribute = Attribute::new(
                    attribute.name().clone(),
                    RObject::from_parts(value, nested_attributes.clone()),
                );
            }
            attribute
        })
        .collect();

    RObject::from_parts(template.value().clone(), Attributes::new(attributes))
}

fn assert_raw_metadata_has_nested_attributes(node: &RdNode, names: &[&str]) {
    let RdNode::Raw(raw) = node else {
        panic!("expected Raw node, got {node:?}");
    };
    for name in names {
        let attribute = raw
            .attributes()
            .iter()
            .find(|attribute| attribute.name() == *name)
            .expect("metadata attribute preserved in Raw");
        assert!(!attribute.value().attributes().is_empty());
    }
}
fn lower_single_node(node: RObject) -> RdNode {
    let document = lower_r_object(&RObject::from_parts(
        RValue::List(vec![node]),
        Default::default(),
    ))
    .expect("lower expanded fragment");
    document
        .into_nodes()
        .into_iter()
        .next()
        .expect("lowered node")
}

fn tagged_raw_value(value: RValue) -> RObject {
    let tag = RStr::Value {
        bytes: Arc::from(&b"\\opaque"[..]),
        encoding: REncoding::Utf8,
        native_encoding: None,
    };
    let mut tag_attribute = fixture_attribute("Rd_tag");
    tag_attribute = Attribute::new(
        tag_attribute.name().clone(),
        bare(RValue::Character(vec![tag])),
    );
    RObject::from_parts(
        value,
        Attributes::new(vec![tag_attribute, fixture_attribute("class")]),
    )
}

mod attributes {
    use super::*;
    include!("attributes.rs");
}
mod errors {
    use super::*;
    include!("errors.rs");
}
mod raw {
    use super::*;
    include!("raw.rs");
}
mod structured {
    use super::*;
    include!("structured.rs");
}
