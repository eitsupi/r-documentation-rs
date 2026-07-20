use super::*;
use crate::producer;

#[test]
fn link_views_follow_r_destination_rules() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(2), RdPathSegment::Child(5)]);
    let inspect = |option: Option<&str>, children: Vec<RdNode>| -> String {
        let option = option.map(|value| vec![RdNode::Text(value.into())]);
        let node = RdTagged::new(RdTag::Link, option, children);
        match node.inspect_link(&base).unwrap().destination() {
            RdLinkDestination::DisplayText { .. } => "display".into(),
            RdLinkDestination::Explicit { topic } => format!("={topic}"),
            RdLinkDestination::Package { package, topic } => match topic {
                RdLinkTopic::DisplayText(_) => format!("{package}:display"),
                RdLinkTopic::Explicit(topic) => format!("{package}:{topic}"),
            },
        }
    };
    assert_eq!(inspect(None, vec![]), "display");
    assert_eq!(
        inspect(Some("pkg"), vec![RdNode::Text("x".into())]),
        "pkg:display"
    );
    assert_eq!(
        inspect(
            Some("pkg:topic"),
            vec![RdNode::tagged(RdTag::Code, None, vec![])]
        ),
        "pkg:topic"
    );
    assert_eq!(
        inspect(
            Some("=dest"),
            vec![RdNode::tagged(RdTag::Code, None, vec![])]
        ),
        "=dest"
    );
    assert_eq!(inspect(Some("=a:b"), vec![]), "=a:b");
    assert_eq!(inspect(Some("a:b:c"), vec![]), "a:b:c");
    assert_eq!(inspect(Some("="), vec![]), "=");
    assert_eq!(inspect(Some(":topic"), vec![]), ":topic");
    assert_eq!(inspect(Some("pkg:"), vec![]), "pkg:");

    let bad = RdTagged::new(
        RdTag::Link,
        None,
        vec![RdNode::tagged(RdTag::Code, None, vec![])],
    );
    let error = bad.inspect_link(&base).unwrap_err();
    assert_eq!(error.path(), &base.with_child(0));
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedContent {
            actual: RdNodeKind::Tagged
        }
    ));
    let bad = RdTagged::new(
        RdTag::Link,
        Some(vec![RdNode::Text("pkg".into())]),
        vec![RdNode::tagged(RdTag::Code, None, vec![])],
    );
    let error = bad.inspect_link(&base).unwrap_err();
    assert_eq!(error.path(), &base.with_child(0));
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedContent {
            actual: RdNodeKind::Tagged
        }
    ));
    let malformed = RdTagged::new(
        RdTag::Link,
        Some(vec![RdNode::Text("a".into()), RdNode::Text("b".into())]),
        vec![],
    );
    assert!(matches!(
        malformed.inspect_link(&base).unwrap_err().kind(),
        RdShapeErrorKind::WrongArity { .. }
    ));
    let malformed = RdTagged::new(RdTag::Link, Some(vec![RdNode::group(vec![])]), vec![]);
    assert!(matches!(
        malformed.inspect_link(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedContent {
            actual: RdNodeKind::Group
        }
    ));
}

#[test]
fn href_view_validates_groups_and_paths() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(2), RdPathSegment::Child(5)]);
    let node = RdTagged::new(
        RdTag::Href,
        None,
        vec![
            RdNode::group(vec![RdNode::Verb("URL".into())]),
            RdNode::group(vec![RdNode::tagged(RdTag::Code, None, vec![])]),
        ],
    );
    let href = node.inspect_href(&base).unwrap();
    assert!(matches!(href.url(), [RdNode::Verb(value)] if value == "URL"));
    assert_eq!(href.path(), &base);
    let option = RdTagged::new(RdTag::Href, Some(vec![]), vec![]);
    assert!(matches!(
        option.inspect_href(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let arity = RdTagged::new(RdTag::Href, None, vec![]);
    assert!(matches!(
        arity.inspect_href(&base).unwrap_err().kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(2),
            actual: 0
        }
    ));
    let child = RdTagged::new(
        RdTag::Href,
        None,
        vec![RdNode::Text("bad".into()), RdNode::group(vec![])],
    );
    let error = child.inspect_href(&base).unwrap_err();
    assert_eq!(error.path(), &base.with_child(0));
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            actual: RdNodeKind::Text
        }
    ));
    let wrong = RdTagged::new(RdTag::Link, None, vec![]);
    assert!(matches!(
        wrong.inspect_href(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Href,
            ..
        }
    ));
    assert_eq!(
        wrong.inspect_href(&base).unwrap_err().to_string(),
        r"expected \href node, found tagged node for \link at top-level[2] / child[5]"
    );
    let wrong = RdTagged::new(RdTag::Href, None, vec![]);
    assert!(matches!(
        wrong.inspect_link(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Link,
            ..
        }
    ));
}

#[test]
fn s4_class_links_preserve_class_and_package_contents() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(1)]);
    let node = RdNode::tagged(
        RdTag::LinkS4Class,
        None,
        vec![RdNode::Text("myClass".into())],
    );
    let view = node.inspect_s4_class_link(&base).unwrap().unwrap();
    assert_eq!(view.class_text(), Some("myClass".into()));
    assert_eq!(view.package(), None);
    assert_eq!(view.package_text(), None);
    let empty = RdNode::tagged(RdTag::LinkS4Class, Some(vec![]), vec![]);
    let empty = empty.s4_class_link(&base).unwrap();
    assert_eq!(empty.package(), Some([].as_slice()));
    assert_eq!(empty.package_text(), Some(String::new()));

    let class_child = RdNode::tagged(RdTag::Emph, None, vec![]);
    let option_child = RdNode::tagged(RdTag::Code, None, vec![]);
    let node = RdNode::tagged(
        RdTag::LinkS4Class,
        Some(vec![RdNode::Text("methods".into()), option_child.clone()]),
        vec![RdNode::Text("my".into()), class_child.clone()],
    );
    let view = node.s4_class_link(&base).unwrap();
    assert_eq!(view.class(), node.as_tagged().unwrap().children());
    assert_eq!(view.package(), node.as_tagged().unwrap().option());
    assert_eq!(view.class_text(), None);
    assert_eq!(view.package_text(), None);
    let plain = RdNode::tagged(
        RdTag::LinkS4Class,
        Some(vec![RdNode::Text("methods".into())]),
        vec![RdNode::Text("myClass".into())],
    );
    assert_eq!(
        plain.s4_class_link(&base).unwrap().package_text(),
        Some("methods".into())
    );
    assert_eq!(
        plain.s4_class_link(&base).unwrap().class_text(),
        Some("myClass".into())
    );
}

#[test]
fn s4_class_links_only_reject_matching_raw() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(1)]);
    let raw = RdNode::Raw(producer::raw_node(
        Some(r"\linkS4class".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(matches!(
        raw.inspect_s4_class_link(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw
        }
    ));
    for tag in [RdTag::Link, RdTag::Href] {
        assert_eq!(
            RdNode::tagged(tag, None, vec![])
                .inspect_s4_class_link(&base)
                .unwrap(),
            None
        );
    }
}
