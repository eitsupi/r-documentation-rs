use rd_ast::{RawRdValue, RdAstPathSegment, RdDocument, RdNode, RdOptionError, RdTag, producer};

#[test]
fn detached_fixture_uses_a_one_node_document_for_inspection() {
    let document = RdDocument::new(vec![RdNode::tagged(
        RdTag::Link,
        None,
        vec![RdNode::Text("display".into())],
    )]);
    let node = document.top_level().get(0).expect("fixture node");
    let link = node
        .inspect_link()
        .expect("well-formed detached fixture")
        .expect("link tag");
    assert_eq!(link.path().segments(), &[RdAstPathSegment::TopLevel(0)]);
    assert_eq!(
        link.display_ref().get(0).unwrap().node(),
        &RdNode::Text("display".into())
    );
}

#[test]
fn strict_cursor_inspectors_return_none_for_an_unrelated_text_node() {
    let document = RdDocument::new(vec![RdNode::Text("ordinary text".into())]);
    let node = document.top_level().get(0).expect("text node");

    assert!(node.inspect_link().unwrap().is_none());
    assert!(node.inspect_href().unwrap().is_none());
    assert!(node.inspect_s4_class_link().unwrap().is_none());
    assert!(node.inspect_list().unwrap().is_none());
    assert!(node.inspect_tabular().unwrap().is_none());
    assert!(node.inspect_equation().unwrap().is_none());
    assert!(node.inspect_inline_span().unwrap().is_none());
    assert!(node.inspect_text_symbol().unwrap().is_none());
    assert!(node.inspect_conditional().unwrap().is_none());
    assert!(node.inspect_enc().unwrap().is_none());
    assert!(node.inspect_method().unwrap().is_none());
    assert!(node.inspect_figure().unwrap().is_none());
    assert!(node.inspect_example_control().unwrap().is_none());
    assert!(node.inspect_sexpr().unwrap().is_none());
    assert!(node.inspect_rd_opts().unwrap().is_none());
}

#[test]
fn strict_cursor_inspectors_reject_matching_raw_and_ignore_unrelated_raw() {
    let shape_tags = [
        RdTag::Link,
        RdTag::Href,
        RdTag::LinkS4Class,
        RdTag::Itemize,
        RdTag::Tabular,
        RdTag::Eqn,
        RdTag::Emph,
        RdTag::Dots,
        RdTag::If,
        RdTag::Enc,
        RdTag::Method,
        RdTag::Figure,
        RdTag::DontRun,
    ];
    let shape_document = RdDocument::new(
        shape_tags
            .iter()
            .map(|tag| {
                RdNode::Raw(producer::raw_node(
                    Some(tag.as_rd_tag().into()),
                    None,
                    vec![],
                    None,
                    vec![],
                ))
            })
            .collect(),
    );
    let unrelated = RdDocument::new(vec![RdNode::Raw(producer::raw_node(
        Some(r"\unrelated".into()),
        None,
        vec![],
        None,
        vec![],
    ))]);

    macro_rules! assert_shape_raw {
        ($cursor:expr, $tag:expr, $inspector:ident) => {{
            let error = $cursor.$inspector().unwrap_err();
            assert_eq!(error.tag(), Some(&$tag));
            assert_eq!(error.path(), $cursor.path());
        }};
    }

    assert_shape_raw!(
        shape_document.top_level().get(0).unwrap(),
        RdTag::Link,
        inspect_link
    );
    assert_shape_raw!(
        shape_document.top_level().get(1).unwrap(),
        RdTag::Href,
        inspect_href
    );
    assert_shape_raw!(
        shape_document.top_level().get(2).unwrap(),
        RdTag::LinkS4Class,
        inspect_s4_class_link
    );
    assert_shape_raw!(
        shape_document.top_level().get(3).unwrap(),
        RdTag::Itemize,
        inspect_list
    );
    assert_shape_raw!(
        shape_document.top_level().get(4).unwrap(),
        RdTag::Tabular,
        inspect_tabular
    );
    assert_shape_raw!(
        shape_document.top_level().get(5).unwrap(),
        RdTag::Eqn,
        inspect_equation
    );
    assert_shape_raw!(
        shape_document.top_level().get(6).unwrap(),
        RdTag::Emph,
        inspect_inline_span
    );
    assert_shape_raw!(
        shape_document.top_level().get(7).unwrap(),
        RdTag::Dots,
        inspect_text_symbol
    );
    assert_shape_raw!(
        shape_document.top_level().get(8).unwrap(),
        RdTag::If,
        inspect_conditional
    );
    assert_shape_raw!(
        shape_document.top_level().get(9).unwrap(),
        RdTag::Enc,
        inspect_enc
    );
    assert_shape_raw!(
        shape_document.top_level().get(10).unwrap(),
        RdTag::Method,
        inspect_method
    );
    assert_shape_raw!(
        shape_document.top_level().get(11).unwrap(),
        RdTag::Figure,
        inspect_figure
    );
    assert_shape_raw!(
        shape_document.top_level().get(12).unwrap(),
        RdTag::DontRun,
        inspect_example_control
    );

    let unrelated = unrelated.top_level().get(0).unwrap();
    assert!(unrelated.inspect_link().unwrap().is_none());
    assert!(unrelated.inspect_href().unwrap().is_none());
    assert!(unrelated.inspect_s4_class_link().unwrap().is_none());
    assert!(unrelated.inspect_list().unwrap().is_none());
    assert!(unrelated.inspect_tabular().unwrap().is_none());
    assert!(unrelated.inspect_equation().unwrap().is_none());
    assert!(unrelated.inspect_inline_span().unwrap().is_none());
    assert!(unrelated.inspect_text_symbol().unwrap().is_none());
    assert!(unrelated.inspect_conditional().unwrap().is_none());
    assert!(unrelated.inspect_enc().unwrap().is_none());
    assert!(unrelated.inspect_method().unwrap().is_none());
    assert!(unrelated.inspect_figure().unwrap().is_none());
    assert!(unrelated.inspect_example_control().unwrap().is_none());
    assert!(unrelated.inspect_sexpr().unwrap().is_none());
    assert!(unrelated.inspect_rd_opts().unwrap().is_none());

    let option_document = RdDocument::new(vec![
        RdNode::Raw(producer::raw_node(
            Some(RdTag::Sexpr.as_rd_tag().into()),
            None,
            vec![],
            None,
            vec![],
        )),
        RdNode::Raw(producer::raw_node(
            Some(RdTag::RdOpts.as_rd_tag().into()),
            None,
            vec![],
            None,
            vec![],
        )),
    ]);
    let sexpr = option_document.top_level().get(0).unwrap();
    let sexpr_error = sexpr.inspect_sexpr().unwrap_err();
    assert!(matches!(
        &sexpr_error,
        RdOptionError::Shape(error) if error.tag() == Some(&RdTag::Sexpr)
    ));
    assert_eq!(sexpr_error.path(), sexpr.path());
    let rd_opts = option_document.top_level().get(1).unwrap();
    let rd_opts_error = rd_opts.inspect_rd_opts().unwrap_err();
    assert!(matches!(
        &rd_opts_error,
        RdOptionError::Shape(error) if error.tag() == Some(&RdTag::RdOpts)
    ));
    assert_eq!(rd_opts_error.path(), rd_opts.path());
}

#[test]
fn strict_system_macro_reports_missing_following_at_definition_path() {
    let srcfile = producer::raw_attribute(
        "srcfile".into(),
        producer::raw_object(RawRdValue::Persisted(vec![Some("env::1".into())]), vec![]),
    );
    let class = producer::raw_attribute(
        "class".into(),
        producer::raw_object(RawRdValue::Character(vec![Some("srcref".into())]), vec![]),
    );
    let srcref = producer::raw_attribute(
        "srcref".into(),
        producer::raw_object(RawRdValue::Integer(vec![Some(1); 6]), vec![srcfile, class]),
    );
    let definition = RdNode::Raw(producer::raw_node(
        Some("USERMACRO".into()),
        None,
        vec![RdNode::Text(
            r##"\Sexpr[results=rd]{tools:::Rd_expr_doi("#1")}10.1/x"##.into(),
        )],
        None,
        vec![
            srcref,
            producer::raw_attribute(
                "macro".into(),
                producer::raw_object(RawRdValue::Character(vec![Some(r"\doi".into())]), vec![]),
            ),
        ],
    ));
    let document = RdDocument::new(vec![definition]);
    let error = document
        .inspect_system_macro_items()
        .next()
        .expect("definition item")
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        rd_ast::RdShapeErrorKind::MissingFollowing { .. }
    ));
    assert_eq!(error.path().segments(), &[RdAstPathSegment::TopLevel(0)]);
}
