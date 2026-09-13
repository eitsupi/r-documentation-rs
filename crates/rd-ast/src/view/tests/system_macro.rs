use super::*;
use crate::{RawNodeClassification, RawRdValue, RdAttribute, classify_raw_node, producer};

fn raw_attr(name: &str, value: RawRdValue) -> RdAttribute {
    producer::raw_attribute(name.into(), producer::raw_object(value, vec![]))
}

fn srcref() -> RdAttribute {
    let srcfile = producer::raw_attribute(
        "srcfile".into(),
        producer::raw_object(RawRdValue::Persisted(vec![Some("env::1".into())]), vec![]),
    );
    let class = producer::raw_attribute(
        "class".into(),
        producer::raw_object(RawRdValue::Character(vec![Some("srcref".into())]), vec![]),
    );
    producer::raw_attribute(
        "srcref".into(),
        producer::raw_object(RawRdValue::Integer(vec![Some(1); 6]), vec![srcfile, class]),
    )
}

fn definition_text(name: &str, argument: &str) -> String {
    let mut text = String::new();
    match name {
        r"\doi" => {
            text.push_str(r"\Sexpr");
            text.push('[');
            text.push_str("results=rd");
            text.push(']');
            text.push('{');
            text.push_str("tools:::Rd_expr_doi");
            text.push('(');
            text.push('"');
            text.push_str("#1");
            text.push('"');
            text.push(')');
            text.push('}');
        }
        r"\CRANpkg" => {
            text.push_str(r"\href");
            text.push('{');
            text.push_str("https://CRAN.R-project.org/package=");
            text.push_str("#1");
            text.push('}');
            text.push('{');
            text.push_str(r"\pkg");
            text.push('{');
            text.push_str("#1");
            text.push('}');
            text.push('}');
        }
        r"\sspace" => {
            text.push_str(r"\ifelse");
            text.push('{');
            text.push_str("latex");
            text.push('}');
            text.push('{');
            text.push_str(r"\out");
            text.push('{');
            text.push('~');
            text.push('}');
            text.push('}');
            text.push('{');
            text.push(' ');
            text.push('}');
        }
        _ => text.push_str(argument),
    }
    if !matches!(name, r"\sspace") {
        text.push_str(argument);
    }
    text
}

fn raw_macro(name: &str, argument: &str, extra: Vec<RdAttribute>) -> RdNode {
    let mut attributes = vec![
        srcref(),
        raw_attr("macro", RawRdValue::Character(vec![Some(name.into())])),
    ];
    attributes.extend(extra);
    RdNode::Raw(producer::raw_node(
        Some("USERMACRO".into()),
        None,
        vec![RdNode::Text(definition_text(name, argument))],
        None,
        attributes,
    ))
}

fn curated(tag: RdTag, children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(tag, None, children)
}

fn expansion_doi(id: &str) -> RdNode {
    RdNode::tagged(
        RdTag::Sexpr,
        Some(vec![RdNode::Text("results=rd".into())]),
        vec![RdNode::RCode(format!(r#"tools:::Rd_expr_doi("{}")"#, id))],
    )
}

fn expansion_cran(package: &str) -> RdNode {
    RdNode::tagged(
        RdTag::Href,
        None,
        vec![
            RdNode::group(vec![RdNode::Verb(format!(
                "https://CRAN.R-project.org/package={package}"
            ))]),
            RdNode::group(vec![curated(
                RdTag::Pkg,
                vec![RdNode::Text(package.into())],
            )]),
        ],
    )
}

fn expansion_sspace() -> RdNode {
    // The oracle wraps each \ifelse argument in a positional Group; the
    // corpus differential harness pinned this shape across the R 4.6.1
    // base man pages.
    RdNode::tagged(
        RdTag::IfElse,
        None,
        vec![
            RdNode::group(vec![RdNode::Text("latex".into())]),
            RdNode::group(vec![curated(RdTag::Out, vec![RdNode::Verb("~".into())])]),
            RdNode::group(vec![RdNode::Text(" ".into())]),
        ],
    )
}

#[test]
fn system_macro_module_is_wired() {
    let document = RdDocument::new(vec![RdNode::tagged(
        RdTag::Doi,
        None,
        vec![RdNode::Text("10.1/x".into())],
    )]);
    let item = document.system_macro_items().next().unwrap();
    assert!(matches!(item, RdSystemMacroItem::Macro(m)
        if m.semantic() == RdSystemMacro::Doi { id: "10.1/x" }
            && m.origin() == RdSystemMacroOrigin::CuratedTag && m.consumed() == 1));
}

#[test]
fn curated_profiles_are_recognized() {
    let body = vec![RdNode::Text("abc".into()), RdNode::Comment("% c".into())];
    let document = RdDocument::new(vec![
        curated(RdTag::Doi, vec![RdNode::Text("10.1/x".into())]),
        curated(RdTag::CranPkg, vec![RdNode::Text("stats".into())]),
        curated(RdTag::Sspace, vec![]),
        curated(RdTag::I, body.clone()),
    ]);
    let items: Vec<_> = document.system_macro_items().collect();
    assert!(matches!(&items[0], RdSystemMacroItem::Macro(m)
        if m.semantic() == RdSystemMacro::Doi { id: "10.1/x" } && m.consumed() == 1));
    if let RdSystemMacroItem::Macro(item) = &items[0] {
        assert_eq!(
            item.anchor_path().segments(),
            &[RdAstPathSegment::TopLevel(0)]
        );
        assert_eq!(item.source_nodes().sibling_range().range(), 0..1);
    }
    assert!(matches!(&items[1], RdSystemMacroItem::Macro(m)
        if m.semantic() == RdSystemMacro::CranPkg { package: "stats" }));
    assert!(matches!(&items[2], RdSystemMacroItem::Macro(m)
        if m.semantic() == RdSystemMacro::Sspace));
    assert!(matches!(&items[3], RdSystemMacroItem::Macro(m)
        if matches!(m.semantic(), RdSystemMacro::I { body: b } if b == body)));
    assert!(
        items
            .iter()
            .all(|item| matches!(item, RdSystemMacroItem::Macro(m)
        if m.origin() == RdSystemMacroOrigin::CuratedTag))
    );
}

#[test]
fn raw_profiles_collapse_and_keep_expansion_hidden() {
    let doi = raw_macro(r"\doi", "10.1/x", vec![]);
    let cran = raw_macro(r"\CRANpkg", "stats", vec![]);
    let space = raw_macro(r"\sspace", "", vec![]);
    let document = RdDocument::new(vec![
        doi,
        expansion_doi("10.1/x"),
        cran,
        expansion_cran("stats"),
        space,
        expansion_sspace(),
    ]);
    assert!(matches!(
        document.nodes()[0].as_raw().map(classify_raw_node),
        Some(RawNodeClassification::ExpectedUserMacroDefinition)
    ));
    let items: Vec<_> = document.system_macro_items().collect();
    assert_eq!(items.len(), 3);
    assert!(
        items
            .iter()
            .all(|item| matches!(item, RdSystemMacroItem::Macro(m)
        if m.origin() == RdSystemMacroOrigin::UserMacroExpansion && m.consumed() == 2))
    );
    if let RdSystemMacroItem::Macro(item) = &items[0] {
        assert_eq!(item.source_nodes().sibling_range().range(), 0..2);
    }
}

#[test]
fn system_macro_views_keep_absolute_positions_after_slicing() {
    let document = RdDocument::new(vec![
        RdNode::Text("prefix".into()),
        curated(RdTag::Sspace, vec![]),
    ]);
    let items: Vec<_> = document
        .top_level()
        .slice(1..2)
        .unwrap()
        .system_macro_items()
        .collect();
    let RdSystemMacroItem::Macro(item) = &items[0] else {
        unreachable!()
    };
    assert_eq!(
        item.anchor_path().segments(),
        &[RdAstPathSegment::TopLevel(1)]
    );
    assert_eq!(item.source_nodes().sibling_range().range(), 1..2);
}

#[test]
fn sliced_system_macro_views_keep_absolute_two_node_ranges() {
    let document = RdDocument::new(vec![
        RdNode::Text("prefix".into()),
        raw_macro(r"\doi", "10.1/x", vec![]),
        expansion_doi("10.1/x"),
        RdNode::Text("suffix".into()),
    ]);
    let items: Vec<_> = document
        .top_level()
        .slice(1..3)
        .unwrap()
        .system_macro_items()
        .collect();
    let RdSystemMacroItem::Macro(item) = &items[0] else {
        unreachable!()
    };
    assert_eq!(
        item.anchor_path().segments(),
        &[RdAstPathSegment::TopLevel(1)]
    );
    assert_eq!(item.source_nodes().sibling_range().range(), 1..3);
    assert_eq!(
        item.source_nodes().get(0).unwrap().path().segments(),
        &[RdAstPathSegment::TopLevel(1)]
    );
    assert_eq!(
        item.source_nodes().get(1).unwrap().path().segments(),
        &[RdAstPathSegment::TopLevel(2)]
    );
}

#[test]
fn invalid_expansion_reports_expansion_path_without_consuming_it() {
    let document = RdDocument::new(vec![
        raw_macro(r"\doi", "10.1/x", vec![]),
        expansion_doi("different"),
    ]);
    let mut strict = document.top_level().inspect_system_macro_items();
    let error = strict.next().unwrap().unwrap_err();
    assert_eq!(error.path().segments(), &[RdAstPathSegment::TopLevel(1)]);
    assert!(matches!(
        strict.next().unwrap(),
        Ok(RdSystemMacroItem::Node { path, .. })
            if path.segments() == [RdAstPathSegment::TopLevel(1)]
    ));
}

#[test]
fn sliced_invalid_expansion_reports_absolute_path_without_consuming_it() {
    let document = RdDocument::new(vec![
        RdNode::Text("prefix".into()),
        raw_macro(r"\doi", "10.1/x", vec![]),
        expansion_doi("different"),
        RdNode::Text("suffix".into()),
    ]);
    let mut strict = document
        .top_level()
        .slice(1..3)
        .unwrap()
        .inspect_system_macro_items();
    let error = strict.next().unwrap().unwrap_err();
    assert_eq!(error.path().segments(), &[RdAstPathSegment::TopLevel(2)]);
    assert!(matches!(
        strict.next().unwrap(),
        Ok(RdSystemMacroItem::Node { path, .. })
            if path.segments() == [RdAstPathSegment::TopLevel(2)]
    ));
}

#[test]
fn invalid_raw_and_i_are_not_hidden() {
    let document = RdDocument::new(vec![
        raw_macro(r"\doi", "wrong", vec![]),
        expansion_doi("x"),
        raw_macro(r"\I", "anything", vec![]),
    ]);
    let lossy: Vec<_> = document.system_macro_items().collect();
    assert!(
        lossy
            .iter()
            .all(|item| matches!(item, RdSystemMacroItem::Node { .. }))
    );
    let mut strict = document.inspect_system_macro_items();
    assert!(strict.next().unwrap().is_err());
    assert!(matches!(
        strict.next().unwrap(),
        Ok(RdSystemMacroItem::Node { .. })
    ));
    assert!(matches!(strict.next().unwrap(), Err(error)
        if matches!(error.kind(), RdShapeErrorKind::UnsupportedRepresentation { .. })));
}

#[test]
fn malformed_curated_and_nested_paths_are_reported() {
    let parent = curated(
        RdTag::Description,
        vec![curated(
            RdTag::Doi,
            vec![RdNode::Text("x".into()), RdNode::Text("y".into())],
        )],
    );
    let document = RdDocument::new(vec![parent]);
    let parent_path = RdAstPath::new(vec![RdAstPathSegment::TopLevel(0)]);
    let description = document.node_at(&parent_path).unwrap();
    let mut strict = description.children().inspect_system_macro_items();
    let error = strict.next().unwrap().unwrap_err();
    assert_eq!(
        error.path().segments(),
        &[RdAstPathSegment::TopLevel(0), RdAstPathSegment::Child(0)]
    );
    let mut nested = description.children().system_macro_items();
    assert!(
        matches!(nested.next().unwrap(), RdSystemMacroItem::Node { path, .. }
        if path.segments() == [RdAstPathSegment::TopLevel(0), RdAstPathSegment::Child(0)])
    );
}

#[test]
fn curated_spelling_raw_is_a_strict_error() {
    let shadowing = RdNode::Raw(producer::raw_node(
        Some(r"\doi".into()),
        None,
        vec![RdNode::Text("10.1/x".into())],
        None,
        vec![],
    ));
    let document = RdDocument::new(vec![shadowing, RdNode::Text("after".into())]);
    assert!(
        document
            .system_macro_items()
            .all(|item| matches!(item, RdSystemMacroItem::Node { .. }))
    );
    let mut strict = document.inspect_system_macro_items();
    assert!(matches!(strict.next().unwrap(), Err(error)
        if matches!(error.kind(), RdShapeErrorKind::UnexpectedNode {
            expected: crate::RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw,
        }) && error.tag() == Some(&RdTag::Doi)));
    assert!(matches!(
        strict.next().unwrap(),
        Ok(RdSystemMacroItem::Node { .. })
    ));
}

#[test]
fn unknown_macro_and_guard_failure_remain_opaque() {
    let unknown = raw_macro(r"\other", "definition", vec![]);
    let extra = raw_macro(
        r"\doi",
        "definition",
        vec![raw_attr("extra", RawRdValue::Null)],
    );
    let document = RdDocument::new(vec![unknown, extra]);
    assert!(
        document
            .system_macro_items()
            .all(|item| matches!(item, RdSystemMacroItem::Node { .. }))
    );
    assert!(
        document
            .inspect_system_macro_items()
            .all(|item| item.is_ok())
    );
}
