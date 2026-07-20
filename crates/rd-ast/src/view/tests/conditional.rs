use super::*;
use crate::{RdConditionalKind, RdNode, RdPath, RdPathSegment, RdTag, producer};

fn base() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(2), RdPathSegment::Child(5)])
}
fn group(nodes: Vec<RdNode>) -> RdNode {
    RdNode::group(nodes)
}
fn conditional(tag: RdTag, children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(tag, None, children)
}
fn text(value: &str) -> RdNode {
    RdNode::Text(value.into())
}

#[test]
fn if_and_ifelse_preserve_groups_and_project_format() {
    let then_branch = vec![RdNode::tagged(RdTag::Emph, None, vec![text("then")])];
    let node = conditional(
        RdTag::If,
        vec![group(vec![text("html")]), group(then_branch.clone())],
    );
    let view = node.inspect_conditional(&base()).unwrap().unwrap();
    assert_eq!(view.kind(), RdConditionalKind::If);
    assert_eq!(view.format(), "html");
    assert_eq!(view.format_nodes(), &[text("html")]);
    assert_eq!(view.then_branch(), then_branch.as_slice());
    assert_eq!(view.else_branch(), None);
    assert_eq!(node.conditional(&base()), Some(view.clone()));

    let else_branch = vec![RdNode::Raw(producer::raw_node(
        Some("opaque".into()),
        None,
        vec![],
        None,
        vec![],
    ))];
    let node = conditional(
        RdTag::IfElse,
        vec![
            group(vec![text("latex")]),
            group(then_branch),
            group(else_branch.clone()),
        ],
    );
    let view = node.inspect_conditional(&base()).unwrap().unwrap();
    assert_eq!(view.kind(), RdConditionalKind::IfElse);
    assert_eq!(view.else_branch(), Some(else_branch.as_slice()));
}

#[test]
fn format_is_exact_and_only_text() {
    let node = conditional(
        RdTag::If,
        vec![group(vec![text("ht"), text("ml")]), group(vec![])],
    );
    assert_eq!(
        node.inspect_conditional(&base()).unwrap().unwrap().format(),
        "html"
    );
    let node = conditional(RdTag::If, vec![group(vec![text(" html")]), group(vec![])]);
    assert_eq!(
        node.inspect_conditional(&base()).unwrap().unwrap().format(),
        " html"
    );
    for bad in [
        RdNode::Verb("x".into()),
        RdNode::RCode("x".into()),
        RdNode::tagged(RdTag::Emph, None, vec![]),
        RdNode::Raw(producer::raw_node(None, None, vec![], None, vec![])),
    ] {
        let node = conditional(RdTag::If, vec![group(vec![bad]), group(vec![])]);
        let error = node.inspect_conditional(&base()).unwrap_err();
        assert_eq!(error.path(), &base().with_child(0).with_child(0));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedContent { .. }
        ));
        assert!(node.conditional(&base()).is_none());
    }
}

#[test]
fn conditional_validation_order_and_raw_matching() {
    let option = RdNode::tagged(RdTag::If, Some(vec![]), vec![]);
    assert!(matches!(
        option.inspect_conditional(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    for (tag, counts) in [(RdTag::If, [0, 1, 3]), (RdTag::IfElse, [0, 2, 4])] {
        for count in counts {
            let node = conditional(tag.clone(), (0..count).map(|_| group(vec![])).collect());
            assert!(
                matches!(node.inspect_conditional(&base()).unwrap_err().kind(), RdShapeErrorKind::WrongArity { actual, .. } if *actual == count)
            );
            assert!(node.conditional(&base()).is_none());
        }
    }
    for (index, bad) in [(0, text("bad")), (1, RdNode::Verb("bad".into()))] {
        let mut children = vec![group(vec![]), group(vec![])];
        children[index] = bad;
        let node = conditional(RdTag::If, children);
        assert!(matches!(
            node.inspect_conditional(&base()).unwrap_err().kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Group,
                ..
            }
        ));
        assert!(node.conditional(&base()).is_none());
    }
    let raw = RdNode::Raw(producer::raw_node(
        Some(r"\if".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(matches!(
        raw.inspect_conditional(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw
        }
    ));
    let unrelated = RdNode::Raw(producer::raw_node(
        Some(r"\enc".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert_eq!(unrelated.inspect_conditional(&base()).unwrap(), None);
}
