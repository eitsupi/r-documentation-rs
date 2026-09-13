use super::*;

fn tabular(spec: &str, body: Vec<RdNode>) -> RdTagged {
    RdTagged::new(
        RdTag::Tabular,
        None,
        vec![
            RdNode::group(vec![RdNode::Text(spec.into())]),
            RdNode::group(body),
        ],
    )
}

fn separator(tag: RdTag) -> RdNode {
    RdNode::tagged(tag, None, vec![])
}

#[test]
fn tabular_view_splits_rows_cells_and_preserves_nested_markup() {
    let nested = RdNode::tagged(RdTag::Strong, None, vec![RdNode::Text("bold".into())]);
    let table = tabular(
        "lcr",
        vec![
            RdNode::Text("a".into()),
            separator(RdTag::Tab),
            nested.clone(),
            separator(RdTag::Tab),
            RdNode::Text("c".into()),
            separator(RdTag::Cr),
            RdNode::Text("d".into()),
            separator(RdTag::Tab),
            RdNode::Text("e".into()),
            separator(RdTag::Tab),
            RdNode::Text("f".into()),
        ],
    );
    let base = RdAstPath::new(vec![RdAstPathSegment::TopLevel(4)]);
    let view = table.inspect_tabular(&base).unwrap();
    assert_eq!(
        view.columns(),
        &[
            RdColumnAlign::Left,
            RdColumnAlign::Center,
            RdColumnAlign::Right
        ]
    );
    assert_eq!(view.rows().len(), 2);
    assert_eq!(view.rows()[0].cells()[1].nodes(), &[nested]);
    assert_eq!(
        view.rows()[1].cells()[2].nodes(),
        &[RdNode::Text("f".into())]
    );
    assert!(view.diagnostics().is_empty());
    assert_eq!(
        view.rows()[0].cells()[0].anchor_path(),
        &base.with_child(1).with_child(0)
    );
}

#[test]
fn tabular_view_keeps_empty_cells_and_does_not_make_trailing_row() {
    let table = tabular(
        "lcr",
        vec![
            separator(RdTag::Tab),
            separator(RdTag::Tab),
            RdNode::Text("x".into()),
            separator(RdTag::Cr),
            separator(RdTag::Cr),
        ],
    );
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    assert_eq!(view.rows().len(), 2);
    assert_eq!(view.rows()[0].cells().len(), 3);
    assert!(view.rows()[0].cells()[0].nodes().is_empty());
    assert!(view.rows()[1].cells()[0].nodes().is_empty());
    assert!(
        tabular("l", vec![])
            .inspect_tabular(&RdAstPath::new(vec![]))
            .unwrap()
            .rows()
            .is_empty()
    );
}

#[test]
fn tabular_view_anchors_leading_empty_cell_at_separator() {
    let table = tabular("ll", vec![separator(RdTag::Tab), RdNode::Text("x".into())]);
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    assert_eq!(view.rows().len(), 1);
    assert_eq!(
        view.rows()[0].anchor_path(),
        &RdAstPath::new(vec![RdAstPathSegment::Child(1), RdAstPathSegment::Child(0)])
    );
    assert_eq!(view.rows()[0].cells().len(), 2);
    assert!(view.rows()[0].cells()[0].nodes().is_empty());
    assert_eq!(
        view.rows()[0].cells()[1].nodes(),
        &[RdNode::Text("x".into())]
    );
}

#[test]
fn table_cells_keep_body_ranges_for_empty_and_nonempty_content() {
    let base = RdAstPath::new(vec![RdAstPathSegment::TopLevel(4)]);
    let table = tabular(
        "ll",
        vec![
            separator(RdTag::Tab),
            RdNode::Text("x".into()),
            separator(RdTag::Tab),
            separator(RdTag::Cr),
            RdNode::Text("y".into()),
            separator(RdTag::Tab),
            separator(RdTag::Cr),
        ],
    );
    let view = table.inspect_tabular(&base).unwrap();
    let body_path = base.with_child(1);

    let leading_empty = view.rows()[0].cells()[0].nodes_ref();
    assert_eq!(leading_empty.container_path(), &body_path);
    assert_eq!(leading_empty.sibling_range().range(), 0..0);
    assert!(leading_empty.is_empty());

    let first_content = view.rows()[0].cells()[1].nodes_ref();
    assert_eq!(first_content.container_path(), &body_path);
    assert_eq!(first_content.sibling_range().range(), 1..2);
    assert_eq!(
        first_content.get(0).unwrap().path(),
        &body_path.with_child(1)
    );

    let trailing_empty = view.rows()[1].cells()[1].nodes_ref();
    assert_eq!(trailing_empty.container_path(), &body_path);
    assert_eq!(trailing_empty.sibling_range().range(), 6..6);
    assert!(trailing_empty.is_empty());

    let sliced = view.rows()[1].cells()[0].nodes_ref().slice(0..1).unwrap();
    assert_eq!(sliced.sibling_range().range(), 4..5);
    assert_eq!(sliced.get(0).unwrap().path(), &body_path.with_child(4));
}

#[test]
fn table_rows_keep_absolute_ranges_and_real_empty_anchors() {
    let base = RdAstPath::new(vec![RdAstPathSegment::TopLevel(9)]);
    let table = tabular(
        "lll",
        vec![
            RdNode::Text("a".into()),
            separator(RdTag::Tab),
            RdNode::Text("b".into()),
            separator(RdTag::Tab),
            RdNode::Text("c".into()),
            separator(RdTag::Cr),
            separator(RdTag::Cr),
            RdNode::Text("d".into()),
            separator(RdTag::Tab),
        ],
    );
    let view = table.inspect_tabular(&base).unwrap();
    let body_path = base.with_child(1);

    assert_eq!(view.rows().len(), 3);
    let first = &view.rows()[0];
    assert_eq!(first.nodes_ref().container_path(), &body_path);
    assert_eq!(first.nodes_ref().sibling_range().range(), 0..5);
    assert_eq!(first.nodes_ref().len(), 5);
    assert_eq!(
        first.nodes_ref().get(1).unwrap().path(),
        &body_path.with_child(1)
    );
    assert_eq!(
        first.nodes_ref().get(3).unwrap().path(),
        &body_path.with_child(3)
    );
    assert_eq!(first.cells().len(), 3);

    let empty = &view.rows()[1];
    assert_eq!(empty.nodes_ref().container_path(), &body_path);
    assert_eq!(empty.nodes_ref().sibling_range().range(), 6..6);
    assert_eq!(empty.anchor_path(), &body_path.with_child(6));
    assert_eq!(empty.cells().len(), 1);
    assert!(empty.nodes_ref().is_empty());

    let terminal_tab = &view.rows()[2];
    assert_eq!(terminal_tab.nodes_ref().sibling_range().range(), 7..9);
    assert_eq!(
        terminal_tab.nodes_ref().get(1).unwrap().path(),
        &body_path.with_child(8)
    );
    assert_eq!(terminal_tab.cells().len(), 1);
    assert_eq!(terminal_tab.anchor_path(), &body_path.with_child(7));
    assert_eq!(view.diagnostics().len(), 2);
}

#[test]
fn tabular_view_does_not_create_terminal_empty_cell() {
    let table = tabular("ll", vec![RdNode::Text("a".into()), separator(RdTag::Tab)]);
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    assert_eq!(view.rows().len(), 1);
    assert_eq!(view.rows()[0].cells().len(), 1);
    assert_eq!(
        view.rows()[0].cells()[0].nodes(),
        &[RdNode::Text("a".into())]
    );
    assert_eq!(view.diagnostics().len(), 1);
    assert!(matches!(
        view.diagnostics()[0].kind(),
        RdShapeErrorKind::CountMismatch {
            construct: RdConstruct::TableRow,
            expected: 2,
            actual: 1,
        }
    ));
}

#[test]
fn tabular_view_reports_invalid_spec_widths_and_malformed_separators() {
    let malformed = RdNode::tagged(
        RdTag::Tab,
        Some(vec![RdNode::Text("option".into())]),
        vec![RdNode::Text("child".into())],
    );
    let table = tabular(
        "l cx",
        vec![RdNode::Text("x".into()), malformed, separator(RdTag::Cr)],
    );
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    assert_eq!(
        view.columns(),
        &[RdColumnAlign::Left, RdColumnAlign::Center]
    );
    assert_eq!(
        view.diagnostics()
            .iter()
            .filter(|e| matches!(e.kind(), RdShapeErrorKind::InvalidValue { .. }))
            .count(),
        2
    );
    assert!(
        view.diagnostics()
            .iter()
            .any(|e| matches!(e.kind(), RdShapeErrorKind::UnexpectedOption))
    );
    assert!(view.diagnostics().iter().any(|e| matches!(
        e.kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(0),
            ..
        }
    )));
    assert_eq!(view.rows()[0].cells().len(), 2);
}

#[test]
fn tabular_invalid_spec_reports_canonical_leaf_byte_range() {
    let table = tabular("léc", vec![]);
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    let diagnostic = view
        .diagnostics()
        .iter()
        .find(|error| matches!(error.kind(), RdShapeErrorKind::InvalidValue { .. }))
        .expect("invalid UTF-8 column-spec character diagnostic");

    assert_eq!(
        diagnostic.path().segments(),
        &[RdAstPathSegment::Child(0), RdAstPathSegment::Child(0),]
    );
    assert_eq!(diagnostic.leaf_byte_range(), Some(1..3));
}

#[test]
fn tabular_view_does_not_split_nested_separators_and_reports_widths() {
    let nested_tab = RdNode::tagged(RdTag::Emph, None, vec![separator(RdTag::Tab)]);
    let table = tabular("l", vec![nested_tab, RdNode::Text("tail".into())]);
    let view = table.inspect_tabular(&RdAstPath::new(vec![])).unwrap();
    assert_eq!(view.rows().len(), 1);
    assert_eq!(view.rows()[0].cells().len(), 1);
    assert_eq!(view.rows()[0].cells()[0].nodes().len(), 2);
    assert!(view.diagnostics().is_empty());
}

#[test]
fn tabular_view_container_failures_are_atomic() {
    let base = RdAstPath::new(vec![RdAstPathSegment::TopLevel(2)]);
    let wrong = RdTagged::new(RdTag::Title, None, vec![])
        .inspect_tabular(&base)
        .unwrap_err();
    assert_eq!(
        wrong.to_string(),
        r"expected \tabular node, found tagged node for \title at top-level[2]"
    );
    assert!(matches!(
        wrong.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tabular,
            ..
        }
    ));
    assert!(matches!(
        RdTagged::new(RdTag::Tabular, Some(vec![]), vec![])
            .inspect_tabular(&base)
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let wrong_arity = RdTagged::new(RdTag::Tabular, None, vec![])
        .inspect_tabular(&base)
        .unwrap_err();
    assert!(matches!(
        wrong_arity.kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(2),
            ..
        }
    ));
    let non_group = RdTagged::new(
        RdTag::Tabular,
        None,
        vec![RdNode::Text("l".into()), RdNode::group(vec![])],
    )
    .inspect_tabular(&base)
    .unwrap_err();
    assert!(matches!(
        non_group.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            ..
        }
    ));
    let bad_colspec = RdTagged::new(
        RdTag::Tabular,
        None,
        vec![RdNode::group(vec![]), RdNode::group(vec![])],
    )
    .inspect_tabular(&base)
    .unwrap_err();
    assert!(matches!(
        bad_colspec.kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(1),
            ..
        }
    ));
    let bad_text = RdTagged::new(
        RdTag::Tabular,
        None,
        vec![
            RdNode::group(vec![RdNode::Text("l".into()), RdNode::Text("r".into())]),
            RdNode::group(vec![]),
        ],
    )
    .inspect_tabular(&base)
    .unwrap_err();
    assert!(matches!(
        bad_text.kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(1),
            ..
        }
    ));
}
