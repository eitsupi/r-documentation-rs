use super::*;

fn path() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(2), RdPathSegment::Child(1)])
}

#[test]
fn every_inline_tag_maps_and_preserves_body() {
    let cases = [
        (RdTag::Emph, RdInlineSpanKind::Emph),
        (RdTag::Strong, RdInlineSpanKind::Strong),
        (RdTag::Bold, RdInlineSpanKind::Bold),
        (RdTag::Code, RdInlineSpanKind::Code),
        (RdTag::Special, RdInlineSpanKind::Special),
        (RdTag::Verb, RdInlineSpanKind::Verb),
        (RdTag::Url, RdInlineSpanKind::Url),
        (RdTag::Email, RdInlineSpanKind::Email),
        (RdTag::File, RdInlineSpanKind::File),
        (RdTag::Pkg, RdInlineSpanKind::Pkg),
        (RdTag::Samp, RdInlineSpanKind::Samp),
        (RdTag::SQuote, RdInlineSpanKind::SQuote),
        (RdTag::DQuote, RdInlineSpanKind::DQuote),
        (RdTag::Kbd, RdInlineSpanKind::Kbd),
        (RdTag::Var, RdInlineSpanKind::Var),
        (RdTag::Env, RdInlineSpanKind::Env),
        (RdTag::Command, RdInlineSpanKind::Command),
        (RdTag::Option, RdInlineSpanKind::Option),
        (RdTag::Acronym, RdInlineSpanKind::Acronym),
        (RdTag::Abbr, RdInlineSpanKind::Abbr),
        (RdTag::Cite, RdInlineSpanKind::Cite),
        (RdTag::Dfn, RdInlineSpanKind::Dfn),
    ];
    let base = path();
    for (tag, kind) in cases {
        let body = vec![RdNode::Text("body".into()), RdNode::RCode("x".into())];
        let node = RdNode::tagged(tag, None, body.clone());
        let lossy = node.inline_span(&base).unwrap();
        let strict = node.inspect_inline_span(&base).unwrap().unwrap();
        for view in [lossy, strict] {
            assert_eq!(view.path(), &base);
            assert_eq!(view.kind(), kind);
            assert_eq!(view.body(), body.as_slice());
        }
    }
}

#[test]
fn inline_bodies_pass_through_including_empty_and_nested() {
    let base = path();
    let cases = [
        RdNode::tagged(RdTag::Emph, None, vec![RdNode::Text("text".into())]),
        RdNode::tagged(RdTag::Code, None, vec![RdNode::RCode("x()".into())]),
        RdNode::tagged(RdTag::Verb, None, vec![RdNode::Verb("literal".into())]),
        RdNode::tagged(
            RdTag::Cite,
            None,
            vec![RdNode::tagged(RdTag::Emph, None, vec![])],
        ),
        RdNode::tagged(RdTag::Kbd, None, vec![]),
    ];
    for node in cases {
        assert!(node.inline_span(&base).is_some());
        assert!(node.inspect_inline_span(&base).is_ok());
    }
}

#[test]
fn inline_options_and_raw_nodes_are_not_interpreted() {
    let base = path();
    for option in [Some(vec![]), Some(vec![RdNode::Text("option".into())])] {
        let node = RdNode::tagged(RdTag::Emph, option, vec![]);
        assert!(node.inline_span(&base).is_none());
        let error = node.inspect_inline_span(&base).unwrap_err();
        assert_eq!(error.path(), &base);
        assert_eq!(error.tag(), Some(&RdTag::Emph));
        assert!(matches!(error.kind(), RdShapeErrorKind::UnexpectedOption));
    }
    for spelling in [r"\emph", r"\code", r"\verb"] {
        let node = RdNode::Raw(crate::producer::raw_node(
            Some(spelling.into()),
            None,
            vec![RdNode::Text("opaque".into())],
            Some(crate::RawRdValue::Character(vec![Some("payload".into())])),
            vec![],
        ));
        assert!(node.inline_span(&base).is_none());
        let error = node.inspect_inline_span(&base).unwrap_err();
        assert_eq!(error.path(), &base);
        assert_eq!(error.tag(), Some(&RdTag::from_rd_tag(spelling)));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw
            }
        ));
    }
}

#[test]
fn inline_unrelated_and_excluded_nodes_are_ignored() {
    let base = path();
    let nodes = [
        RdNode::tagged(RdTag::Title, None, vec![]),
        RdNode::Text("leaf".into()),
        RdNode::RCode("code".into()),
        RdNode::group(vec![]),
        RdNode::tagged(RdTag::Preformatted, None, vec![]),
        RdNode::tagged(RdTag::Out, None, vec![]),
        RdNode::tagged(RdTag::I, None, vec![]),
        RdNode::tagged(RdTag::Doi, None, vec![]),
        RdNode::tagged(RdTag::CranPkg, None, vec![]),
        RdNode::tagged(RdTag::LinkS4Class, None, vec![]),
        RdNode::tagged(RdTag::Sspace, None, vec![]),
    ];
    for node in nodes {
        assert!(node.inline_span(&base).is_none());
        assert!(node.inspect_inline_span(&base).unwrap().is_none());
    }
}

#[test]
fn text_symbols_preserve_kind_fallback_and_shape_errors() {
    let base = path();
    for (tag, kind, fallback) in [
        (RdTag::R, RdTextSymbolKind::R, "R"),
        (RdTag::Dots, RdTextSymbolKind::Dots, "..."),
        (RdTag::LDots, RdTextSymbolKind::LDots, "..."),
    ] {
        let node = RdNode::tagged(tag, None, vec![]);
        let view = node.text_symbol(&base).unwrap();
        assert_eq!(view.path(), &base);
        assert_eq!(view.kind(), kind);
        assert_eq!(view.fallback_text(), fallback);
        assert_eq!(node.inspect_text_symbol(&base).unwrap().unwrap(), view);
    }
    assert_ne!(RdTextSymbolKind::Dots, RdTextSymbolKind::LDots);
    assert_eq!(
        RdTextSymbolKind::Dots.fallback_text(),
        RdTextSymbolKind::LDots.fallback_text()
    );

    let option = RdNode::tagged(RdTag::R, Some(vec![]), vec![]);
    assert!(option.text_symbol(&base).is_none());
    assert!(matches!(
        option.inspect_text_symbol(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));

    for count in [1, 2] {
        let node = RdNode::tagged(RdTag::Dots, None, vec![RdNode::Text("x".into()); count]);
        assert!(node.text_symbol(&base).is_none());
        let error = node.inspect_text_symbol(&base).unwrap_err();
        assert_eq!(error.path(), &base);
        assert_eq!(error.tag(), Some(&RdTag::Dots));
        assert!(
            matches!(error.kind(), RdShapeErrorKind::WrongArity { expected: RdArity::Exactly(0), actual } if *actual == count)
        );
    }

    let raw = RdNode::Raw(crate::producer::raw_node(
        Some(r"\R".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(raw.text_symbol(&base).is_none());
    assert!(matches!(
        raw.inspect_text_symbol(&base).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw
        }
    ));
    let unrelated = RdNode::Raw(crate::producer::raw_node(
        Some(r"\other".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(unrelated.inspect_text_symbol(&base).unwrap().is_none());
}
