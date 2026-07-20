use super::*;
use crate::{RdMethodKind, RdNode, RdPath, RdPathSegment, RdTag, producer};

fn base() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(4), RdPathSegment::Child(2)])
}
fn group(nodes: Vec<RdNode>) -> RdNode {
    RdNode::group(nodes)
}
fn method(tag: RdTag, generic: Vec<RdNode>, qualifier: Vec<RdNode>) -> RdNode {
    RdNode::tagged(tag, None, vec![group(generic), group(qualifier)])
}
fn text(value: &str) -> RdNode {
    RdNode::Text(value.into())
}

#[test]
fn method_kinds_and_exact_scalars() {
    for (tag, kind) in [
        (RdTag::Method, RdMethodKind::Method),
        (RdTag::S3Method, RdMethodKind::S3Method),
        (RdTag::S4Method, RdMethodKind::S4Method),
    ] {
        let node = method(tag, vec![text("print")], vec![text("data.frame")]);
        let view = node.inspect_method(&base()).unwrap().unwrap();
        assert_eq!(view.kind(), kind);
        assert_eq!(view.generic(), "print");
        assert_eq!(view.qualifier(), "data.frame");
    }
    for value in ["[", "+", "%>%", "OldClass,NewClass"] {
        let node = method(RdTag::S4Method, vec![text(value)], vec![text(value)]);
        assert_eq!(
            node.inspect_method(&base()).unwrap().unwrap().generic(),
            value
        );
    }
    let node = method(
        RdTag::Method,
        vec![text("pr"), text("int")],
        vec![text("data."), text("frame")],
    );
    let view = node.inspect_method(&base()).unwrap().unwrap();
    assert_eq!(view.generic(), "print");
    assert_eq!(view.qualifier(), "data.frame");
}

#[test]
fn method_rejects_options_arity_groups_and_non_text() {
    let option = RdNode::tagged(RdTag::Method, Some(vec![]), vec![]);
    assert!(matches!(
        option.inspect_method(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    for count in [0, 1, 3] {
        let node = RdNode::tagged(
            RdTag::Method,
            None,
            (0..count).map(|_| group(vec![])).collect(),
        );
        assert!(
            matches!(node.inspect_method(&base()).unwrap_err().kind(), RdShapeErrorKind::WrongArity { actual, .. } if *actual == count)
        );
    }
    for index in 0..2 {
        let mut children = vec![group(vec![]), group(vec![])];
        children[index] = RdNode::Text("bad".into());
        let error = RdNode::tagged(RdTag::Method, None, children)
            .inspect_method(&base())
            .unwrap_err();
        assert_eq!(error.path(), &base().with_child(index));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Group,
                ..
            }
        ));
    }
    for (index, bad) in [
        (0, RdNode::RCode("x".into())),
        (1, RdNode::Verb("x".into())),
        (0, RdNode::tagged(RdTag::Emph, None, vec![])),
    ] {
        let mut children = vec![group(vec![]), group(vec![])];
        children[index] = group(vec![bad]);
        let error = RdNode::tagged(RdTag::Method, None, children)
            .inspect_method(&base())
            .unwrap_err();
        assert_eq!(error.path(), &base().with_child(index).with_child(0));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedContent { .. }
        ));
    }
}

#[test]
fn method_raw_matching_and_unrelated_nodes() {
    for tag in [r"\method", r"\S3method", r"\S4method"] {
        let raw = RdNode::Raw(producer::raw_node(
            Some(tag.into()),
            None,
            vec![],
            None,
            vec![],
        ));
        assert!(matches!(
            raw.inspect_method(&base()).unwrap_err().kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw
            }
        ));
    }
    let other = RdNode::Raw(producer::raw_node(
        Some(r"\if".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert_eq!(other.inspect_method(&base()).unwrap(), None);
}
