use rd_ast::{
    RawRdValue, RdDocument, RdDynamicMarkupEvent, RdDynamicMarkupState, RdEffectiveSexprOptions,
    RdGenerationHeader, RdGenerator, RdNode, RdOptionError, RdOptionList, RdOptionPairErrorKind,
    RdOptionValueKind, RdPath, RdPathSegment, RdSectionKind, RdSexprOptionKey,
    RdSexprOptionOverrides, RdSexprResults, RdSexprStage, RdStripWhite, RdTag, producer,
};

fn document_from_public_producer_api() -> RdDocument {
    RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("base".to_string())]),
            vec![RdNode::Text("print".to_string())],
        ),
        RdNode::group(vec![RdNode::Text("group".to_string())]),
        RdNode::Raw(producer::raw_node(
            Some("opaque".to_string()),
            None,
            vec![RdNode::Text("raw child".to_string())],
            None,
            vec![producer::raw_attribute(
                "metadata".to_string(),
                producer::raw_object(
                    RawRdValue::Character(vec![Some("value".to_string())]),
                    Vec::new(),
                ),
            )],
        )),
    ])
}

#[test]
fn external_producer_and_consumer_api() {
    let document = document_from_public_producer_api();
    let nodes = document.nodes();

    let tagged = nodes[0].as_tagged().expect("tagged node");
    assert_eq!(tagged.tag(), &RdTag::Link);
    assert_eq!(
        tagged.option(),
        Some(&[RdNode::Text("base".to_string())][..])
    );
    assert_eq!(tagged.children(), &[RdNode::Text("print".to_string())]);

    let group = nodes[1].as_group().expect("group node");
    assert_eq!(group.children(), &[RdNode::Text("group".to_string())]);

    let raw = nodes[2].as_raw().expect("raw node");
    assert_eq!(raw.tag(), Some("opaque"));
    assert_eq!(raw.option(), None);
    assert_eq!(raw.children(), &[RdNode::Text("raw child".to_string())]);
    assert_eq!(raw.payload(), None);
    let attribute = &raw.attributes()[0];
    assert_eq!(attribute.name(), "metadata");
    assert_eq!(
        attribute.value().value(),
        &RawRdValue::Character(vec![Some("value".to_string())])
    );

    for node in document.nodes() {
        match node {
            RdNode::Tagged(_) | RdNode::Group(_) | RdNode::Raw(_) => {}
            RdNode::Text(_) | RdNode::RCode(_) | RdNode::Verb(_) | RdNode::Comment(_) => {}
            _ => {} // Future node variants are intentionally ignored by this proof.
        }
    }

    let (tag, option, children) = document
        .into_nodes()
        .remove(0)
        .into_tagged()
        .expect("owned tagged node")
        .into_parts();
    assert_eq!(tag, RdTag::Link);
    assert_eq!(option, Some(vec![RdNode::Text("base".to_string())]));
    assert_eq!(children, vec![RdNode::Text("print".to_string())]);

    let children = RdNode::group(vec![RdNode::Text("owned".to_string())])
        .into_group()
        .expect("owned group")
        .into_children();
    assert_eq!(children, vec![RdNode::Text("owned".to_string())]);

    let raw = producer::raw_node(
        Some("opaque".into()),
        None,
        vec![],
        Some(RawRdValue::Integer(vec![Some(7), None])),
        vec![],
    );
    assert_eq!(
        raw.payload(),
        Some(&RawRdValue::Integer(vec![Some(7), None]))
    );
    let (tag, option, children, payload, attributes) = raw.into_parts();
    assert_eq!(tag.as_deref(), Some("opaque"));
    assert_eq!(option, None);
    assert!(children.is_empty());
    assert_eq!(payload, Some(RawRdValue::Integer(vec![Some(7), None])));
    assert!(attributes.is_empty());
}

#[test]
fn shape_errors_are_public_standard_errors() -> Result<(), Box<dyn std::error::Error>> {
    let error = RdDocument::new(vec![RdNode::tagged(
        RdTag::Title,
        Some(vec![RdNode::Text("unexpected option".into())]),
        vec![],
    )])
    .inspect_title()
    .unwrap_err();
    let expected = error.to_string();
    let boxed: Box<dyn std::error::Error> = Box::new(error);
    assert_eq!(boxed.to_string(), expected);
    Ok(())
}

#[test]
fn new_document_views_are_public() {
    let section = RdNode::tagged(
        RdTag::Section,
        None,
        vec![
            RdNode::group(vec![RdNode::Text("Title".into())]),
            RdNode::group(vec![RdNode::Text("Body".into())]),
        ],
    );
    let document = RdDocument::new(vec![
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("name".into())]),
        RdNode::tagged(RdTag::Keyword, None, vec![RdNode::Text("kw".into())]),
        section,
    ]);
    assert_eq!(
        document.name().map(rd_ast::text_contents),
        Some("name".into())
    );
    assert_eq!(document.keywords().collect::<Vec<_>>(), vec!["kw"]);
    let visit = document.section_tree().next().unwrap();
    assert_eq!(visit.kind(), RdSectionKind::Section);
    assert_eq!(
        document
            .inspect_section_tree()
            .next()
            .unwrap()
            .unwrap()
            .nesting(),
        0
    );
    assert!(document.inspect_name().unwrap().is_some());
}

#[test]
fn system_macro_view_is_public() {
    let document = RdDocument::new(vec![RdNode::tagged(RdTag::Sspace, None, vec![])]);
    let item = document.system_macro_items().next().unwrap();
    assert!(matches!(item, rd_ast::RdSystemMacroItem::Macro(m)
        if m.semantic() == rd_ast::RdSystemMacro::Sspace));
}

#[test]
fn lifecycle_view_is_public() {
    let document = RdDocument::new(vec![RdNode::tagged(
        RdTag::Description,
        None,
        vec![RdNode::tagged(
            RdTag::Figure,
            None,
            vec![RdNode::group(vec![RdNode::Verb(
                "lifecycle-stable.svg".into(),
            )])],
        )],
    )]);
    let badges = document.lifecycle_badges();
    let badge = badges.first().expect("lifecycle badge");
    assert_eq!(badge.stage(), &rd_ast::RdLifecycleStage::Stable);
}

#[test]
fn generation_header_view_is_public() {
    let document = RdDocument::new(vec![RdNode::Comment(
        "% Generated by roxygen2: do not edit by hand".into(),
    )]);
    let header: Option<RdGenerationHeader<'_>> = document.generation_header();
    let header = header.expect("roxygen header");
    assert!(header.is_generated());
    assert_eq!(header.generator(), Some(&RdGenerator::Roxygen2));
    assert!(!header.has_sources());
    assert!(header.source_files().is_empty());
}

#[test]
fn option_parser_public_api() -> Result<(), Box<dyn std::error::Error>> {
    let path = RdPath::new(vec![RdPathSegment::TopLevel(0)]);
    let nodes = [RdNode::Text("echo=true,results=rd".into())];
    let parsed = RdOptionList::parse(&nodes, path.clone())?;
    assert_eq!(parsed.path(), &path);
    assert_eq!(parsed.pairs()[0].index(), 0);
    assert_eq!(parsed.pairs()[0].key(), "echo");
    assert_eq!(parsed.pairs()[0].value(), "true");
    assert_eq!(parsed.pairs()[0].known_key(), Some(RdSexprOptionKey::Echo));
    assert!(parsed.diagnostics().is_empty());
    let overrides: RdSexprOptionOverrides = parsed.typed();
    assert_eq!(overrides.echo, Some(true));
    assert_eq!(overrides.results, Some(RdSexprResults::Rd));
    let mut effective = RdEffectiveSexprOptions::default();
    overrides.apply_to(&mut effective);
    assert!(effective.echo);
    assert_eq!(effective.results, RdSexprResults::Rd);
    assert_eq!(RdSexprStage::Install, RdSexprStage::Install);
    assert_eq!(RdStripWhite::Trim, RdStripWhite::Trim);
    let _ = RdOptionPairErrorKind::EmptyPair;
    let _ = RdOptionValueKind::Boolean;
    let shape: RdOptionError =
        RdOptionList::parse(&[RdNode::Comment("%".into())], path).unwrap_err();
    assert!(shape.path().to_string().contains("child[0]"));
    Ok(())
}

#[test]
fn dynamic_markup_public_api() {
    let document = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::RdOpts,
            None,
            vec![RdNode::Verb("stage=render".into())],
        ),
        RdNode::tagged(RdTag::Sexpr, None, vec![RdNode::RCode("1 + 1".into())]),
    ]);
    let events: Vec<_> = document.inspect_dynamic_markup().collect();
    assert!(matches!(
        &events[0],
        Ok(RdDynamicMarkupEvent::OptionsChanged { effective, .. })
            if effective.stage == RdSexprStage::Render
    ));
    assert!(matches!(
        &events[1],
        Ok(RdDynamicMarkupEvent::Sexpr(resolved))
            if resolved.view().code() == "1 + 1"
                && resolved.state() == RdDynamicMarkupState::Unresolved { stage: RdSexprStage::Render }
    ));
}
