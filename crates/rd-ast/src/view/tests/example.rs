use super::*;

fn base_path() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(3)])
}

#[test]
fn all_example_control_tags_have_lossy_and_strict_views() {
    let cases = [
        (RdTag::DontRun, RdExampleControlKind::DontRun),
        (RdTag::DontTest, RdExampleControlKind::DontTest),
        (RdTag::DontShow, RdExampleControlKind::DontShow),
        (RdTag::DontDiff, RdExampleControlKind::DontDiff),
        (RdTag::TestOnly, RdExampleControlKind::TestOnly),
    ];
    let path = base_path();
    for (tag, kind) in cases {
        let children = vec![RdNode::RCode("x()".into()), RdNode::Comment("% c".into())];
        let node = RdNode::tagged(tag, None, children.clone());
        let lossy = node.example_control(&path).unwrap();
        assert_eq!(lossy.path(), &path);
        assert_eq!(lossy.kind(), kind);
        assert_eq!(lossy.body(), children.as_slice());
        let strict = node.inspect_example_control(&path).unwrap().unwrap();
        assert_eq!(strict.path(), &path);
        assert_eq!(strict.kind(), kind);
        assert_eq!(strict.body(), children.as_slice());
    }
    assert_ne!(
        RdExampleControlKind::DontShow,
        RdExampleControlKind::TestOnly
    );
}

#[test]
fn example_control_preserves_leaf_kinds_nested_markup_and_empty_bodies() {
    let path = base_path();
    let verb_body = vec![RdNode::Verb("shown".into())];
    let verb = RdNode::tagged(RdTag::DontRun, None, verb_body.clone());
    assert_eq!(
        verb.example_control(&path).unwrap().body(),
        verb_body.as_slice()
    );

    let nested_body = vec![
        RdNode::RCode("before(".into()),
        RdNode::tagged(RdTag::Link, None, vec![RdNode::Text("topic".into())]),
        RdNode::Comment("% retained".into()),
        RdNode::RCode(")".into()),
    ];
    let nested = RdNode::tagged(RdTag::DontTest, None, nested_body.clone());
    assert_eq!(
        nested
            .inspect_example_control(&path)
            .unwrap()
            .unwrap()
            .body(),
        nested_body.as_slice()
    );

    let code_body = vec![RdNode::RCode("code".into())];
    let code = RdNode::tagged(RdTag::DontShow, None, code_body.clone());
    assert_eq!(
        code.example_control(&path).unwrap().body(),
        code_body.as_slice()
    );

    let empty = RdNode::tagged(RdTag::TestOnly, None, vec![]);
    assert!(empty.example_control(&path).unwrap().body().is_empty());
    assert!(
        empty
            .inspect_example_control(&path)
            .unwrap()
            .unwrap()
            .body()
            .is_empty()
    );
}

#[test]
fn example_control_options_are_lossy_none_and_strict_errors() {
    let path = base_path();
    for option in [Some(vec![]), Some(vec![RdNode::Text("opt".into())])] {
        let node = RdNode::tagged(RdTag::DontDiff, option, vec![]);
        assert!(node.example_control(&path).is_none());
        let error = node.inspect_example_control(&path).unwrap_err();
        assert_eq!(error.path(), &path);
        assert_eq!(error.tag(), Some(&RdTag::DontDiff));
        assert!(matches!(error.kind(), RdShapeErrorKind::UnexpectedOption));
    }
}

#[test]
fn unrelated_nodes_and_raw_nodes_follow_example_control_boundary() {
    let path = base_path();
    for node in [
        RdNode::RCode("leaf".into()),
        RdNode::Comment("% leaf".into()),
        RdNode::tagged(RdTag::Code, None, vec![]),
    ] {
        assert!(node.example_control(&path).is_none());
        assert!(node.inspect_example_control(&path).unwrap().is_none());
    }

    for spelling in [r"\dontrun", r"\testonly"] {
        let node = RdNode::Raw(crate::producer::raw_node(
            Some(spelling.into()),
            None,
            vec![RdNode::RCode("opaque".into())],
            Some(crate::RawRdValue::Character(vec![Some("payload".into())])),
            vec![],
        ));
        assert!(node.example_control(&path).is_none());
        let error = node.inspect_example_control(&path).unwrap_err();
        assert_eq!(error.path(), &path);
        assert_eq!(error.tag(), Some(&RdTag::from_rd_tag(spelling)));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw
            }
        ));
    }

    for raw in [None, Some(r"\other")] {
        let node = RdNode::Raw(crate::producer::raw_node(
            raw.map(str::to_owned),
            None,
            vec![],
            None,
            vec![],
        ));
        assert!(node.example_control(&path).is_none());
        assert!(node.inspect_example_control(&path).unwrap().is_none());
    }
}

#[test]
fn raw_children_are_preserved_and_errors_propagate_full_paths() {
    let path = RdPath::new(vec![RdPathSegment::TopLevel(7), RdPathSegment::Child(2)]);
    let raw_child = RdNode::Raw(crate::producer::raw_node(
        Some(r"\opaque".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    let wrapper = RdNode::tagged(RdTag::DontRun, None, vec![raw_child.clone()]);
    let view = wrapper.inspect_example_control(&path).unwrap().unwrap();
    assert_eq!(view.body(), &[raw_child]);

    let option = RdNode::tagged(RdTag::DontRun, Some(vec![]), vec![]);
    assert_eq!(
        option.inspect_example_control(&path).unwrap_err().path(),
        &path
    );
    let raw = RdNode::Raw(crate::producer::raw_node(
        Some(r"\dontrun".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert_eq!(
        raw.inspect_example_control(&path).unwrap_err().path(),
        &path
    );
}
