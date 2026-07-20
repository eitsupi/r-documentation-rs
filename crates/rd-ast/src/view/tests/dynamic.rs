use super::*;

fn test_base() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(4)])
}

fn sexpr(option: Option<Vec<RdNode>>, children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(RdTag::Sexpr, option, children)
}

fn rd_opts(children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(RdTag::RdOpts, None, children)
}

fn sexpr_events<'a>(document: &'a RdDocument) -> Vec<Result<RdResolvedSexpr<'a>, RdOptionError>> {
    document
        .inspect_dynamic_markup()
        .filter_map(|event| match event {
            Ok(RdDynamicMarkupEvent::Sexpr(view)) => Some(Ok(view)),
            Err(error) => Some(Err(error)),
            Ok(RdDynamicMarkupEvent::OptionsChanged { .. }) => None,
        })
        .collect()
}

#[test]
fn sexpr_constructor_view_validates_shape_and_options() {
    let base = test_base();
    let wrong = RdTagged::new(RdTag::Title, None, vec![])
        .inspect_sexpr(&base)
        .unwrap_err();
    assert!(matches!(
        wrong,
        RdOptionError::Shape(ref error)
            if matches!(error.kind(), RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Sexpr,
                actual: RdNodeKind::Tagged,
            })
    ));

    for (children, actual) in [
        (vec![], 0),
        (
            vec![RdNode::RCode("a".into()), RdNode::RCode("b".into())],
            2,
        ),
    ] {
        let error = RdTagged::new(RdTag::Sexpr, None, children)
            .inspect_sexpr(&base)
            .unwrap_err();
        assert!(matches!(
            error,
            RdOptionError::Shape(ref error)
                if matches!(error.kind(), RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(1), actual: n
                } if *n == actual)
        ));
    }

    let error = RdTagged::new(RdTag::Sexpr, None, vec![RdNode::Text("not code".into())])
        .inspect_sexpr(&base)
        .unwrap_err();
    assert!(matches!(
        error,
        RdOptionError::Shape(ref error)
            if error.path().segments()
                == [RdPathSegment::TopLevel(4), RdPathSegment::Child(0)]
                && matches!(error.kind(), RdShapeErrorKind::UnexpectedContent {
                    actual: RdNodeKind::Text
                })
    ));

    let bare_node = RdTagged::new(RdTag::Sexpr, None, vec![RdNode::RCode("x <- 1".into())]);
    let bare = bare_node.inspect_sexpr(&base).unwrap();
    assert_eq!(bare.code(), "x <- 1");
    assert_eq!(bare.options(), None);
    assert_eq!(bare.option_overrides(), RdSexprOptionOverrides::empty());

    let optioned_node = RdTagged::new(
        RdTag::Sexpr,
        Some(vec![RdNode::Text("stage=render,echo=true".into())]),
        vec![RdNode::RCode("x".into())],
    );
    let optioned = optioned_node.inspect_sexpr(&base).unwrap();
    assert_eq!(
        optioned.option_overrides(),
        RdSexprOptionOverrides {
            stage: Some(RdSexprStage::Render),
            echo: Some(true),
            ..RdSexprOptionOverrides::empty()
        }
    );

    let hard = RdTagged::new(
        RdTag::Sexpr,
        Some(vec![RdNode::Text("stage".into())]),
        vec![RdNode::RCode("x".into())],
    )
    .inspect_sexpr(&base)
    .unwrap_err();
    assert_eq!(hard.path(), &base.with_option());

    let soft_node = RdTagged::new(
        RdTag::Sexpr,
        Some(vec![RdNode::Text("unknown=value".into())]),
        vec![RdNode::RCode("x".into())],
    );
    let soft = soft_node.inspect_sexpr(&base).unwrap();
    assert_eq!(soft.options().unwrap().diagnostics().len(), 1);
}

#[test]
fn rd_opts_constructor_view_validates_shape_and_accepts_plain_bodies() {
    let base = test_base();
    let wrong = RdTagged::new(RdTag::Title, None, vec![])
        .inspect_rd_opts(&base)
        .unwrap_err();
    assert!(matches!(
        wrong,
        RdOptionError::Shape(ref error)
            if matches!(error.kind(), RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::RdOpts,
                actual: RdNodeKind::Tagged,
            })
    ));

    let with_option = RdTagged::new(
        RdTag::RdOpts,
        Some(vec![RdNode::Text("echo=true".into())]),
        vec![],
    )
    .inspect_rd_opts(&base)
    .unwrap_err();
    assert!(
        matches!(with_option, RdOptionError::Shape(error) if matches!(
            error.kind(), RdShapeErrorKind::UnexpectedOption
        ))
    );

    for body in [
        vec![RdNode::Verb("stage=render".into())],
        vec![RdNode::Text("stage=build".into())],
        vec![],
    ] {
        let node = RdTagged::new(RdTag::RdOpts, None, body);
        let view = node.inspect_rd_opts(&base).unwrap();
        assert_eq!(view.path(), &base);
    }
    let markup = RdTagged::new(
        RdTag::RdOpts,
        None,
        vec![RdNode::tagged(RdTag::Code, None, vec![])],
    )
    .inspect_rd_opts(&base)
    .unwrap_err();
    assert_eq!(markup.path(), &base.with_child(0));
}

#[test]
fn dynamic_markup_resolves_subsequent_document_options_only() {
    let document = RdDocument::new(vec![
        sexpr(None, vec![RdNode::RCode("before".into())]),
        rd_opts(vec![RdNode::Verb("stage=render".into())]),
        sexpr(
            Some(vec![RdNode::Text("echo=true".into())]),
            vec![RdNode::RCode("between".into())],
        ),
        rd_opts(vec![RdNode::Verb("results=rd".into())]),
        sexpr(None, vec![RdNode::RCode("after".into())]),
    ]);
    let events = sexpr_events(&document);
    assert_eq!(events.len(), 3);
    let effective: Vec<_> = events
        .iter()
        .map(|event| event.as_ref().unwrap().effective_options())
        .collect();
    assert_eq!(effective[0], RdEffectiveSexprOptions::default());
    assert_eq!(effective[1].stage, RdSexprStage::Render);
    assert!(effective[1].echo);
    assert_eq!(effective[1].results, RdSexprResults::Text);
    assert_eq!(effective[2].stage, RdSexprStage::Render);
    assert_eq!(effective[2].results, RdSexprResults::Rd);
    assert!(!effective[2].echo);
}

#[test]
fn dynamic_markup_traverses_nested_nodes_and_preserves_state_on_errors() {
    let document = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![sexpr(None, vec![RdNode::RCode("nested".into())])],
        ),
        rd_opts(vec![RdNode::tagged(RdTag::Code, None, vec![])]),
        sexpr(None, vec![RdNode::RCode("after error".into())]),
        RdNode::Raw(crate::producer::raw_node(
            Some("\\container".into()),
            None,
            vec![sexpr(None, vec![RdNode::RCode("hidden".into())])],
            None,
            vec![],
        )),
    ]);
    let events: Vec<_> = document.inspect_dynamic_markup().collect();
    assert!(matches!(
        &events[0],
        Ok(RdDynamicMarkupEvent::Sexpr(resolved))
            if resolved.view().path().segments()
                == [RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]
    ));
    assert!(matches!(&events[1], Err(error) if error.path().segments()
            == [RdPathSegment::TopLevel(1), RdPathSegment::Child(0)]));
    assert!(matches!(
        &events[2],
        Ok(RdDynamicMarkupEvent::Sexpr(resolved))
            if resolved.effective_options() == RdEffectiveSexprOptions::default()
    ));
    for event in events.iter().filter_map(|event| event.as_ref().ok()) {
        if let RdDynamicMarkupEvent::Sexpr(resolved) = event {
            assert_eq!(
                resolved.state(),
                RdDynamicMarkupState::Unresolved {
                    stage: resolved.effective_options().stage
                }
            );
        }
    }
    assert_eq!(events.len(), 3, "Raw contents must not be traversed");
}

#[test]
fn dynamic_markup_conditionals_fold_options_in_document_order() {
    // Intentional R conformance: this mirrors tools:::processRdSexprs
    // parity, not accidental conditional blindness.
    for (conditional, expected) in [
        (
            RdNode::tagged(
                RdTag::If,
                None,
                vec![
                    RdNode::group(vec![RdNode::Text("html".into())]),
                    RdNode::group(vec![rd_opts(vec![RdNode::Verb("stage=render".into())])]),
                ],
            ),
            (true, false, false),
        ),
        (
            RdNode::tagged(
                RdTag::IfElse,
                None,
                vec![
                    RdNode::group(vec![RdNode::Text("html".into())]),
                    RdNode::group(vec![rd_opts(vec![RdNode::Verb("results=rd".into())])]),
                    RdNode::group(vec![rd_opts(vec![RdNode::Verb("echo=true".into())])]),
                ],
            ),
            (false, true, true),
        ),
    ] {
        let document = RdDocument::new(vec![
            conditional,
            sexpr(None, vec![RdNode::RCode("after conditional".into())]),
        ]);
        let events = sexpr_events(&document);
        assert_eq!(events.len(), 1);
        let effective = events[0].as_ref().unwrap().effective_options();
        assert_eq!(effective.stage == RdSexprStage::Render, expected.0);
        assert_eq!(effective.results == RdSexprResults::Rd, expected.1);
        assert_eq!(effective.echo, expected.2);
    }
}
