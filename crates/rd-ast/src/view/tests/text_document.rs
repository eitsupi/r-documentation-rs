use super::*;

#[test]
fn strict_views_agree_on_well_formed_shapes_and_ignore_trivia() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(RdTag::Title, None, vec![RdNode::Text("T".into())]),
        RdNode::Comment("% separator".into()),
        RdNode::tagged(RdTag::Alias, None, vec![RdNode::Text("a".into())]),
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![raw_group(vec![]), raw_group(vec![])],
        ),
        RdNode::tagged(
            RdTag::Arguments,
            None,
            vec![
                item(vec![RdNode::Text("x".into())], vec![]),
                RdNode::Text("\n".into()),
                RdNode::Comment("% trivia".into()),
            ],
        ),
    ]);
    assert_eq!(doc.inspect_title().unwrap(), doc.title());
    assert_eq!(
        doc.inspect_aliases()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()[0]
            .text_contents(),
        "a"
    );
    assert_eq!(
        doc.inspect_sections()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        doc.sections().count()
    );
    assert_eq!(
        doc.inspect_arguments()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .len(),
        doc.arguments().count()
    );
}

#[test]
fn strict_views_report_shape_errors_with_paths() {
    let raw = || {
        RdNode::Raw(crate::producer::raw_node(
            Some(r"\title".into()),
            None,
            vec![],
            None,
            vec![],
        ))
    };
    let error = RdDocument::new(vec![RdNode::tagged(RdTag::Title, None, vec![]), raw()])
        .inspect_title()
        .unwrap_err();
    assert_eq!(error.path().segments(), &[RdPathSegment::TopLevel(1)]);
    assert_eq!(error.tag(), Some(&RdTag::Title));
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            actual: RdNodeKind::Raw,
            ..
        }
    ));

    let error = RdDocument::new(vec![
        RdNode::tagged(RdTag::Title, None, vec![]),
        RdNode::tagged(RdTag::Title, None, vec![]),
    ])
    .inspect_title()
    .unwrap_err();
    assert!(
        matches!(error.kind(), RdShapeErrorKind::Duplicate { first_path, .. } if first_path.segments() == [RdPathSegment::TopLevel(0)])
    );

    let error = RdDocument::new(vec![RdNode::tagged(
        RdTag::Section,
        None,
        vec![RdNode::Text("bad".into())],
    )])
    .inspect_sections()
    .next()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.path().segments(), &[RdPathSegment::TopLevel(0)]);
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(2),
            actual: 1
        }
    ));

    let error = RdDocument::new(vec![RdNode::tagged(
        RdTag::Arguments,
        None,
        vec![RdNode::Text("not trivia".into())],
    )])
    .inspect_arguments()
    .unwrap()
    .next()
    .unwrap()
    .unwrap_err();
    assert_eq!(
        error.path().segments(),
        &[RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]
    );
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedContent {
            actual: RdNodeKind::Text
        }
    ));
}

#[test]
fn strict_arguments_report_item_option_and_non_group_child() {
    let option_item = RdNode::tagged(
        RdTag::Item,
        Some(vec![]),
        vec![raw_group(vec![]), raw_group(vec![])],
    );
    let bad_item = RdNode::tagged(
        RdTag::Item,
        None,
        vec![RdNode::Text("bad".into()), raw_group(vec![])],
    );
    let doc = RdDocument::new(vec![RdNode::tagged(
        RdTag::Arguments,
        None,
        vec![option_item, bad_item],
    )]);
    assert!(matches!(
        doc.inspect_arguments()
            .unwrap()
            .next()
            .unwrap()
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let error = doc
        .inspect_arguments()
        .unwrap()
        .nth(1)
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.path().segments(),
        &[
            RdPathSegment::TopLevel(0),
            RdPathSegment::Child(1),
            RdPathSegment::Child(0)
        ]
    );
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            actual: RdNodeKind::Text
        }
    ));
}

#[test]
fn strict_aliases_sections_and_argument_containers_report_errors() {
    let alias = RdDocument::new(vec![RdNode::tagged(RdTag::Alias, Some(vec![]), vec![])]);
    assert!(matches!(
        alias.inspect_aliases().next().unwrap().unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));

    let section = RdDocument::new(vec![RdNode::tagged(RdTag::Section, Some(vec![]), vec![])]);
    assert!(matches!(
        section
            .inspect_sections()
            .next()
            .unwrap()
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let section = RdDocument::new(vec![RdNode::tagged(
        RdTag::Section,
        None,
        vec![RdNode::Text("bad".into()), raw_group(vec![])],
    )]);
    let error = section.inspect_sections().next().unwrap().unwrap_err();
    assert_eq!(
        error.path().segments(),
        &[RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]
    );
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            actual: RdNodeKind::Text
        }
    ));

    let duplicate = RdDocument::new(vec![
        RdNode::tagged(RdTag::Arguments, None, vec![]),
        RdNode::tagged(RdTag::Arguments, None, vec![]),
    ]);
    let error = duplicate.inspect_arguments().err().unwrap();
    assert!(
        matches!(error.kind(), RdShapeErrorKind::Duplicate { first_path, .. } if first_path.segments() == [RdPathSegment::TopLevel(0)])
    );
    let raw = RdDocument::new(vec![RdNode::Raw(crate::producer::raw_node(
        Some(r"\arguments".into()),
        None,
        vec![],
        None,
        vec![],
    ))]);
    assert!(matches!(
        raw.inspect_arguments().err().unwrap().kind(),
        RdShapeErrorKind::UnexpectedNode {
            actual: RdNodeKind::Raw,
            ..
        }
    ));
    let option = RdDocument::new(vec![RdNode::tagged(RdTag::Arguments, Some(vec![]), vec![])]);
    assert!(matches!(
        option.inspect_arguments().err().unwrap().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let arity = RdDocument::new(vec![RdNode::tagged(
        RdTag::Arguments,
        None,
        vec![RdNode::tagged(RdTag::Item, None, vec![])],
    )]);
    assert!(matches!(
        arity
            .inspect_arguments()
            .unwrap()
            .next()
            .unwrap()
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::WrongArity {
            expected: RdArity::Exactly(2),
            actual: 0
        }
    ));
}

#[test]
fn text_contents_flattens_nested_tagged_raw_comment_and_option() {
    let nodes = vec![
        RdNode::Text("See ".to_string()),
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("ignored-option".to_string())]),
            vec![RdNode::Text("print".to_string())],
        ),
        RdNode::Comment("% ignored comment".to_string()),
        RdNode::Text(" and ".to_string()),
        RdNode::Raw(crate::producer::raw_node(
            Some(r"\method".to_string()),
            Some(vec![RdNode::Text("ignored-raw-option".to_string())]),
            vec![
                raw_group(vec![RdNode::Text("generic".to_string())]),
                raw_group(vec![RdNode::Text("class".to_string())]),
            ],
            None,
            Vec::new(),
        )),
        RdNode::RCode(".".to_string()),
    ];

    assert_eq!(
        text_contents(&nodes),
        "See print and genericclass.".to_string()
    );
}

#[test]
fn title_first_wins_on_duplicates() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(RdTag::Title, None, vec![RdNode::Text("first".to_string())]),
        RdNode::tagged(RdTag::Title, None, vec![RdNode::Text("second".to_string())]),
    ]);

    assert_eq!(
        doc.title(),
        Some(vec![RdNode::Text("first".to_string())].as_slice())
    );
}

#[test]
fn title_ignores_raw_node_with_matching_tag_string() {
    let doc = RdDocument::new(vec![RdNode::Raw(crate::producer::raw_node(
        Some(r"\title".to_string()),
        None,
        vec![RdNode::Text("not a real title".to_string())],
        None,
        Vec::new(),
    ))]);

    assert_eq!(doc.title(), None);
}

#[test]
fn description_usage_value_find_their_respective_tags() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![RdNode::Text("desc".to_string())],
        ),
        RdNode::tagged(RdTag::Usage, None, vec![RdNode::RCode("f(x)".to_string())]),
        RdNode::tagged(
            RdTag::Value,
            None,
            vec![RdNode::Text("a value".to_string())],
        ),
    ]);

    assert_eq!(
        doc.description(),
        Some(vec![RdNode::Text("desc".to_string())].as_slice())
    );
    assert_eq!(
        doc.usage(),
        Some(vec![RdNode::RCode("f(x)".to_string())].as_slice())
    );
    assert_eq!(
        doc.value(),
        Some(vec![RdNode::Text("a value".to_string())].as_slice())
    );
    assert_eq!(doc.title(), None);
}

#[test]
fn aliases_yields_text_contents_with_markup_inside() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(RdTag::Alias, None, vec![RdNode::Text("plain".to_string())]),
        RdNode::Text("\n".to_string()),
        RdNode::tagged(
            RdTag::Alias,
            None,
            vec![
                RdNode::Text("with-".to_string()),
                RdNode::tagged(RdTag::Code, None, vec![RdNode::Text("markup".to_string())]),
            ],
        ),
        RdNode::tagged(RdTag::Alias, None, vec![]),
    ]);

    let aliases: Vec<String> = doc.aliases().collect();
    assert_eq!(
        aliases,
        vec![
            "plain".to_string(),
            "with-markup".to_string(),
            String::new(),
        ]
    );
}

#[test]
fn sections_recognizes_custom_section_shape() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![
                raw_group(vec![RdNode::Text("Custom Title".to_string())]),
                raw_group(vec![RdNode::Text("Custom body.".to_string())]),
            ],
        ),
        // Not a section: wrong arity (only one child).
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![raw_group(vec![RdNode::Text("lonely".to_string())])],
        ),
        // Not a section: has an option.
        RdNode::tagged(
            RdTag::Section,
            Some(vec![RdNode::Text("opt".to_string())]),
            vec![
                raw_group(vec![RdNode::Text("t".to_string())]),
                raw_group(vec![RdNode::Text("b".to_string())]),
            ],
        ),
        // Not a section: children aren't Raw.
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![RdNode::Text("t".to_string()), RdNode::Text("b".to_string())],
        ),
    ]);

    let sections: Vec<RdSection<'_>> = doc.sections().collect();
    assert_eq!(sections.len(), 1);
    assert_eq!(text_contents(sections[0].title), "Custom Title".to_string());
    assert_eq!(text_contents(sections[0].body), "Custom body.".to_string());
}

#[test]
fn arguments_pairs_items_and_skips_whitespace_and_malformed_entries() {
    let doc = RdDocument::new(vec![RdNode::tagged(
        RdTag::Arguments,
        None,
        vec![
            item(
                vec![RdNode::Text("x".to_string())],
                vec![RdNode::Text("the first argument".to_string())],
            ),
            // Whitespace TEXT leaf between items -- skipped.
            RdNode::Text("\n".to_string()),
            item(
                vec![RdNode::Text("y".to_string())],
                vec![RdNode::Text("the second argument".to_string())],
            ),
            // Malformed: only one child.
            RdNode::tagged(
                RdTag::Item,
                None,
                vec![raw_group(vec![RdNode::Text("z".to_string())])],
            ),
            // Malformed: has an option (itemize/enumerate-style item).
            RdNode::tagged(
                RdTag::Item,
                Some(vec![RdNode::Text("label".to_string())]),
                vec![
                    raw_group(vec![RdNode::Text("a".to_string())]),
                    raw_group(vec![RdNode::Text("b".to_string())]),
                ],
            ),
        ],
    )]);

    let arguments: Vec<(String, String)> = doc
        .arguments()
        .map(|arg| (text_contents(arg.name), text_contents(arg.description)))
        .collect();
    assert_eq!(
        arguments,
        vec![
            ("x".to_string(), "the first argument".to_string()),
            ("y".to_string(), "the second argument".to_string()),
        ]
    );
}

#[test]
fn arguments_empty_when_no_top_level_arguments_node() {
    let doc = RdDocument::new(vec![RdNode::tagged(
        RdTag::Title,
        None,
        vec![RdNode::Text("t".to_string())],
    )]);

    assert_eq!(doc.arguments().count(), 0);
}

#[test]
fn fixed_section_accessors_cover_the_remaining_vocabulary() {
    let tags = [
        (RdTag::Name, "name"),
        (RdTag::Details, "details"),
        (RdTag::Note, "note"),
        (RdTag::Author, "author"),
        (RdTag::References, "references"),
        (RdTag::SeeAlso, "seealso"),
        (RdTag::Examples, "examples"),
        (RdTag::Format, "format"),
        (RdTag::Source, "source"),
        (RdTag::Encoding, "encoding"),
        (RdTag::DocType, "docType"),
        (RdTag::Rdversion, "Rdversion"),
        (RdTag::Synopsis, "synopsis"),
    ];
    let nodes = tags
        .iter()
        .map(|(tag, text)| RdNode::tagged(tag.clone(), None, vec![RdNode::Text((*text).into())]))
        .collect();
    let doc = RdDocument::new(nodes);
    macro_rules! assert_fixed_accessor {
        ($accessor:ident, $inspect:ident) => {
            assert_eq!(doc.$accessor(), doc.$inspect().unwrap());
            assert!(RdDocument::new(vec![]).$accessor().is_none());
        };
    }
    assert_fixed_accessor!(name, inspect_name);
    assert_fixed_accessor!(details, inspect_details);
    assert_fixed_accessor!(note, inspect_note);
    assert_fixed_accessor!(author, inspect_author);
    assert_fixed_accessor!(references, inspect_references);
    assert_fixed_accessor!(see_also, inspect_see_also);
    assert_fixed_accessor!(examples, inspect_examples);
    assert_fixed_accessor!(format, inspect_format);
    assert_fixed_accessor!(source, inspect_source);
    assert_fixed_accessor!(encoding, inspect_encoding);
    assert_fixed_accessor!(doc_type, inspect_doc_type);
    assert_fixed_accessor!(rd_version, inspect_rd_version);
    assert_fixed_accessor!(synopsis, inspect_synopsis);
    assert_eq!(doc.name(), doc.inspect_name().unwrap());
    assert_eq!(doc.name().map(text_contents), Some("name".into()));
    assert!(RdDocument::new(vec![]).inspect_name().unwrap().is_none());

    let duplicate = RdDocument::new(vec![
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("first".into())]),
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("second".into())]),
    ]);
    assert_eq!(text_contents(duplicate.name().unwrap()), "first");
    assert!(matches!(
        duplicate.inspect_name().unwrap_err().kind(),
        RdShapeErrorKind::Duplicate { first_path, .. }
            if first_path.segments() == [RdPathSegment::TopLevel(0)]
    ));
    let option = RdDocument::new(vec![RdNode::tagged(RdTag::Name, Some(vec![]), vec![])]);
    assert!(matches!(
        option.inspect_name().unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let raw = RdDocument::new(vec![RdNode::Raw(crate::producer::raw_node(
        Some("\\name".into()),
        None,
        vec![],
        None,
        vec![],
    ))]);
    assert!(matches!(
        raw.inspect_name().unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            actual: RdNodeKind::Raw,
            ..
        }
    ));
}

#[test]
fn keywords_and_concepts_are_repeatable_strict_views() {
    let doc = RdDocument::new(vec![
        RdNode::tagged(RdTag::Keyword, None, vec![RdNode::Text("one".into())]),
        RdNode::tagged(RdTag::Concept, None, vec![RdNode::Text("first".into())]),
        RdNode::tagged(RdTag::Keyword, None, vec![RdNode::Text("two".into())]),
        RdNode::tagged(RdTag::Concept, None, vec![RdNode::Text("second".into())]),
    ]);
    assert_eq!(doc.keywords().collect::<Vec<_>>(), ["one", "two"]);
    assert_eq!(
        doc.inspect_keywords()
            .map(|entry| entry.unwrap().text_contents())
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert_eq!(doc.concepts().collect::<Vec<_>>(), ["first", "second"]);
    assert_eq!(doc.inspect_concepts().count(), 2);
    let option = RdDocument::new(vec![RdNode::tagged(RdTag::Keyword, Some(vec![]), vec![])]);
    assert!(matches!(
        option
            .inspect_keywords()
            .next()
            .unwrap()
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    let raw = RdDocument::new(vec![RdNode::Raw(crate::producer::raw_node(
        Some("\\concept".into()),
        None,
        vec![],
        None,
        vec![],
    ))]);
    assert!(matches!(
        raw.inspect_concepts().next().unwrap().unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            actual: RdNodeKind::Raw,
            ..
        }
    ));
}

fn custom_section(title: &str, body: Vec<RdNode>) -> RdNode {
    RdNode::tagged(
        RdTag::Section,
        None,
        vec![raw_group(vec![RdNode::Text(title.into())]), raw_group(body)],
    )
}

fn custom_subsection(title: &str, body: Vec<RdNode>) -> RdNode {
    RdNode::tagged(
        RdTag::Subsection,
        None,
        vec![raw_group(vec![RdNode::Text(title.into())]), raw_group(body)],
    )
}

#[test]
fn section_tree_is_preorder_path_aware_and_ignores_orphans() {
    let doc = RdDocument::new(vec![
        RdNode::Text(" trivia ".into()),
        custom_section(
            "one",
            vec![
                RdNode::Comment("% c".into()),
                RdNode::Text("body".into()),
                custom_subsection(
                    "one-a",
                    vec![custom_subsection(
                        "one-a-i",
                        vec![RdNode::Text("deep".into())],
                    )],
                ),
            ],
        ),
        RdNode::Comment("% between".into()),
        custom_section("two", vec![RdNode::Text("second".into())]),
        custom_subsection("orphan", vec![]),
    ]);
    let visits: Vec<_> = doc.section_tree().collect();
    assert_eq!(
        visits
            .iter()
            .map(|visit| (visit.kind(), visit.nesting(), text_contents(visit.title())))
            .collect::<Vec<_>>(),
        vec![
            (RdSectionKind::Section, 0, "one".into()),
            (RdSectionKind::Subsection, 1, "one-a".into()),
            (RdSectionKind::Subsection, 2, "one-a-i".into()),
            (RdSectionKind::Section, 0, "two".into()),
        ]
    );
    assert_eq!(text_contents(visits[0].body()), "bodyone-aone-a-ideep");
    assert_eq!(text_contents(visits[2].body()), "deep");
    assert_eq!(
        visits
            .iter()
            .map(|visit| visit.path().segments())
            .collect::<Vec<_>>(),
        vec![
            &[RdPathSegment::TopLevel(1)][..],
            &[
                RdPathSegment::TopLevel(1),
                RdPathSegment::Child(1),
                RdPathSegment::Child(2)
            ][..],
            &[
                RdPathSegment::TopLevel(1),
                RdPathSegment::Child(1),
                RdPathSegment::Child(2),
                RdPathSegment::Child(1),
                RdPathSegment::Child(0),
            ][..],
            &[RdPathSegment::TopLevel(3)][..],
        ]
    );
    assert_eq!(doc.sections().count(), 2);
    assert_eq!(doc.inspect_section_tree().count(), 4);
}

#[test]
fn section_tree_skips_malformed_candidate_and_descendants() {
    let malformed = RdNode::tagged(
        RdTag::Section,
        None,
        vec![
            raw_group(vec![RdNode::Text("bad".into())]),
            raw_group(vec![custom_subsection("hidden", vec![])]),
            raw_group(vec![]),
        ],
    );
    let doc = RdDocument::new(vec![
        custom_section("first", vec![]),
        malformed,
        custom_section("last", vec![]),
    ]);
    assert_eq!(
        doc.section_tree()
            .map(|v| text_contents(v.title()))
            .collect::<Vec<_>>(),
        ["first", "last"]
    );
    let results: Vec<_> = doc.inspect_section_tree().collect();
    assert_eq!(results.len(), 3);
    assert_eq!(text_contents(results[0].as_ref().unwrap().title()), "first");
    assert!(
        matches!(results[1], Err(ref error) if matches!(error.kind(), RdShapeErrorKind::WrongArity { .. }))
    );
    assert_eq!(text_contents(results[2].as_ref().unwrap().title()), "last");
}
