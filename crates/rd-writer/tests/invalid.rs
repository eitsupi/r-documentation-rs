use rd_ast::{RdDocument, RdNode, RdPath, RdPathSegment, RdTag};
use rd_writer::{UnserializableKind, WriteError};

fn write_error(node: RdNode) -> WriteError {
    rd_writer::write_document(&RdDocument::from(vec![node])).unwrap_err()
}

fn document_write_error(nodes: Vec<RdNode>) -> WriteError {
    rd_writer::write_document(&RdDocument::from(nodes)).unwrap_err()
}

fn error(node: RdNode) -> UnserializableKind {
    match write_error(node) {
        WriteError::Unserializable { kind, .. } => kind,
        WriteError::Io { .. } => panic!("unexpected I/O error"),
        _ => panic!("unexpected non-exhaustive error variant"),
    }
}

fn document_error(nodes: Vec<RdNode>) -> UnserializableKind {
    match document_write_error(nodes) {
        WriteError::Unserializable { kind, .. } => kind,
        _ => panic!("unexpected error"),
    }
}

fn assert_path(node: RdNode, expected: RdPath) {
    let error = write_error(node);
    assert!(matches!(error, WriteError::Unserializable { .. }));
    assert_eq!(error.ast_path(), Some(&expected), "error: {error:?}");
}

fn assert_document_path(nodes: Vec<RdNode>, expected: RdPath) {
    let error = document_write_error(nodes);
    assert!(matches!(error, WriteError::Unserializable { .. }));
    assert_eq!(error.ast_path(), Some(&expected));
}

fn assert_kind_path(node: RdNode, expected_kind: UnserializableKind, expected_path: RdPath) {
    match write_error(node) {
        WriteError::Unserializable { kind, path } => {
            assert_eq!(kind, expected_kind);
            assert_eq!(path, expected_path);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn assert_document_kind_path(
    nodes: Vec<RdNode>,
    expected_kind: UnserializableKind,
    expected_path: RdPath,
) {
    match document_write_error(nodes) {
        WriteError::Unserializable { kind, path } => {
            assert_eq!(kind, expected_kind);
            assert_eq!(path, expected_path);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

fn path(segments: Vec<RdPathSegment>) -> RdPath {
    RdPath::new(segments)
}

fn group(children: Vec<RdNode>) -> RdNode {
    RdNode::Group(children.into())
}

fn raw() -> RdNode {
    RdNode::Raw(rd_ast::producer::raw_node(None, None, vec![], None, vec![]))
}

#[test]
fn rejects_raw_empty_and_unknown_tags() {
    let raw_value = rd_ast::producer::raw_node(None, None, vec![], None, vec![]);
    assert_eq!(error(RdNode::Raw(raw_value)), UnserializableKind::RawNode);
    assert_document_path(vec![raw()], path(vec![RdPathSegment::TopLevel(0)]));
    assert_eq!(
        error(RdNode::Text(String::new())),
        UnserializableKind::UnrepresentableLeaf
    );
    assert_path(
        RdNode::Text(String::new()),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Unknown("USERMACRO".into()),
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::UnknownTag
    );
    assert_path(
        RdNode::tagged(
            RdTag::Unknown("USERMACRO".into()),
            None,
            vec![RdNode::Text("x".into())],
        ),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Unknown(r"\name".into()),
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::UnknownTag
    );
    assert_path(
        RdNode::tagged(
            RdTag::Unknown(r"\name".into()),
            None,
            vec![RdNode::Text("x".into())],
        ),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
}

#[test]
fn rejects_bare_groups_and_document_lists() {
    assert_eq!(
        error(RdNode::Group(rd_ast::RdGroup::from(vec![RdNode::Text(
            "x".into()
        )]))),
        UnserializableKind::BareGroup
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::List,
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::ListInDocument
    );
}

#[test]
fn rejects_illegal_contexts_and_unclosed_r_like_frames() {
    let nested_name = RdNode::tagged(
        RdTag::Description,
        None,
        vec![RdNode::tagged(
            RdTag::Name,
            None,
            vec![RdNode::Verb("x".into())],
        )],
    );
    assert_eq!(
        error(nested_name),
        UnserializableKind::TagNotAllowedInContext
    );
    assert_path(
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![RdNode::tagged(
                RdTag::Name,
                None,
                vec![RdNode::Verb("x".into())],
            )],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    let section_in_latex = RdNode::tagged(
        RdTag::Title,
        None,
        vec![RdNode::tagged(
            RdTag::Name,
            None,
            vec![RdNode::Verb("x".into())],
        )],
    );
    assert_eq!(
        error(section_in_latex),
        UnserializableKind::TagNotAllowedInContext
    );
    assert_path(
        RdNode::tagged(
            RdTag::Title,
            None,
            vec![RdNode::tagged(
                RdTag::Name,
                None,
                vec![RdNode::Verb("x".into())],
            )],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    let structured_in_verbatim = RdNode::tagged(
        RdTag::Verb,
        None,
        vec![RdNode::tagged(
            RdTag::Emph,
            None,
            vec![RdNode::Text("x".into())],
        )],
    );
    assert_eq!(
        error(structured_in_verbatim),
        UnserializableKind::TagNotAllowedInContext
    );
    assert_path(
        RdNode::tagged(
            RdTag::Verb,
            None,
            vec![RdNode::tagged(
                RdTag::Emph,
                None,
                vec![RdNode::Text("x".into())],
            )],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    let tab_in_r_like = RdNode::tagged(
        RdTag::Examples,
        None,
        vec![
            RdNode::tagged(RdTag::Tab, None, vec![]),
            RdNode::RCode("\n".into()),
        ],
    );
    assert_eq!(
        error(tab_in_r_like),
        UnserializableKind::TagNotAllowedInContext
    );
    assert_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![
                RdNode::tagged(RdTag::Tab, None, vec![]),
                RdNode::RCode("\n".into()),
            ],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    let unterminated = RdNode::tagged(
        RdTag::Examples,
        None,
        vec![RdNode::RCode(r#""unterminated"#.into())],
    );
    assert_eq!(
        error(unterminated),
        UnserializableKind::UnterminatedRLikeState
    );
    assert_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![RdNode::RCode(r#""unterminated"#.into())],
        ),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
    let quoted_tag = RdNode::tagged(
        RdTag::Examples,
        None,
        vec![
            RdNode::RCode(r#"""#.into()),
            RdNode::tagged(RdTag::Code, None, vec![RdNode::RCode("x".into())]),
            RdNode::RCode("\"\n".into()),
        ],
    );
    assert_eq!(
        error(quoted_tag),
        UnserializableKind::TagNotAllowedInContext
    );
    assert_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![
                RdNode::RCode(r#"""#.into()),
                RdNode::tagged(RdTag::Code, None, vec![RdNode::RCode("x".into())]),
                RdNode::RCode("\"\n".into()),
            ],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(1)]),
    );
}

#[test]
fn rejects_noncanonical_conditional_targets_and_equations() {
    let group = |nodes| RdNode::Group(rd_ast::RdGroup::from(nodes));
    let conditional = RdNode::tagged(
        RdTag::IfDef,
        None,
        vec![group(vec![RdNode::Text("pkg".into())]), group(vec![])],
    );
    assert!(matches!(
        error(conditional),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![group(vec![RdNode::Text("pkg".into())]), group(vec![])],
        ),
        path(vec![
            RdPathSegment::TopLevel(0),
            RdPathSegment::Child(0),
            RdPathSegment::Child(0),
        ]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Eqn,
            None,
            vec![RdNode::Verb(r"abc\".into())]
        )),
        UnserializableKind::UnrepresentableEquation
    );
    assert_path(
        RdNode::tagged(RdTag::Eqn, None, vec![RdNode::Verb(r"abc\".into())]),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
}

#[test]
fn rejects_wrong_leaf_kind_and_equation() {
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Name,
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::UnexpectedNodeKind
    );
    assert_path(
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("x".into())]),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Eqn,
            None,
            vec![RdNode::Verb("{".into())]
        )),
        UnserializableKind::UnrepresentableEquation
    );
    assert_path(
        RdNode::tagged(RdTag::Eqn, None, vec![RdNode::Verb("{".into())]),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    assert_eq!(
        error(RdNode::Text("x\r".into())),
        UnserializableKind::UnrepresentableLeaf
    );
    assert_path(
        RdNode::Text("x\r".into()),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
}

#[test]
fn rejects_comment_and_option_terminator() {
    assert_eq!(
        document_error(vec![
            RdNode::Comment("% comment".into()),
            RdNode::Text("x".into()),
        ]),
        UnserializableKind::InvalidComment
    );
    assert_document_path(
        vec![
            RdNode::Comment("% comment".into()),
            RdNode::Text("x".into()),
        ],
        path(vec![RdPathSegment::TopLevel(0)]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("]".into())]),
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::InvalidOptionContent
    );
    assert_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("]".into())]),
            vec![RdNode::Text("x".into())],
        ),
        path(vec![
            RdPathSegment::TopLevel(0),
            RdPathSegment::Option,
            RdPathSegment::Child(0),
        ]),
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Title,
            Some(vec![]),
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::InvalidTagShape {
            tag: r"\title".into()
        }
    );
    assert_path(
        RdNode::tagged(RdTag::Title, Some(vec![]), vec![RdNode::Text("x".into())]),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Option]),
    );
}

#[test]
fn rejects_conditional_option_at_option_path() {
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            Some(vec![]),
            vec![
                group(vec![RdNode::Text("pkg\n".into())]),
                group(vec![RdNode::Text("body\n".into())]),
            ],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "#ifdef".into(),
        },
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Option]),
    );
}

#[test]
fn reports_first_invalid_option_sibling() {
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("valid".into()), RdNode::Text("]".into())]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidOptionContent,
        path(vec![
            RdPathSegment::TopLevel(0),
            RdPathSegment::Option,
            RdPathSegment::Child(1),
        ]),
    );
}

#[test]
fn reports_nested_tag_option_before_children_in_invalid_option() {
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::tagged(
                RdTag::Link,
                Some(vec![RdNode::Text("]".into())]),
                vec![RdNode::Text("]".into())],
            )]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidOptionContent,
        path(vec![
            RdPathSegment::TopLevel(0),
            RdPathSegment::Option,
            RdPathSegment::Child(0),
            RdPathSegment::Option,
            RdPathSegment::Child(0),
        ]),
    );
}

#[test]
fn rejects_contextual_tag_shapes() {
    let item = |children| RdNode::tagged(RdTag::Item, None, children);
    let group = |text: &str| RdNode::Group(rd_ast::RdGroup::from(vec![RdNode::Text(text.into())]));
    for parent in [RdTag::Itemize, RdTag::Enumerate] {
        assert!(matches!(
            error(RdNode::tagged(
                parent.clone(),
                None,
                vec![item(vec![RdNode::Text("x".into())])]
            )),
            UnserializableKind::InvalidTagShape { .. }
        ));
        assert_path(
            RdNode::tagged(parent, None, vec![item(vec![RdNode::Text("x".into())])]),
            path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
        );
    }
    for parent in [RdTag::Arguments, RdTag::Value, RdTag::Describe] {
        assert!(matches!(
            error(RdNode::tagged(parent.clone(), None, vec![item(vec![])])),
            UnserializableKind::InvalidTagShape { .. }
        ));
        assert_path(
            RdNode::tagged(parent, None, vec![item(vec![])]),
            path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
        );
    }
    let figure = RdNode::tagged(RdTag::Figure, None, vec![group("f"), group("o")]);
    let examples = RdNode::tagged(RdTag::Examples, None, vec![figure]);
    assert!(matches!(
        error(examples),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![RdNode::tagged(
                RdTag::Figure,
                None,
                vec![
                    RdNode::group(vec![RdNode::Text("f".into())]),
                    RdNode::group(vec![RdNode::Text("o".into())]),
                ],
            )],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
    assert!(matches!(
        error(RdNode::tagged(
            RdTag::Section,
            None,
            vec![RdNode::Text("a".into()); 3]
        )),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert_path(
        RdNode::tagged(RdTag::Section, None, vec![RdNode::Text("a".into()); 3]),
        path(vec![RdPathSegment::TopLevel(0)]),
    );
    assert!(matches!(
        error(RdNode::tagged(RdTag::Title, None, vec![group("a")])),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert_path(
        RdNode::tagged(
            RdTag::Title,
            None,
            vec![RdNode::group(vec![RdNode::Text("a".into())])],
        ),
        path(vec![RdPathSegment::TopLevel(0), RdPathSegment::Child(0)]),
    );
}

#[test]
fn reports_canonical_paths_for_invalid_shapes() {
    let top = |index| path(vec![RdPathSegment::TopLevel(index)]);
    let child = |index| RdPathSegment::Child(index);

    assert_document_kind_path(
        vec![RdNode::Text("ok\n".into()), raw()],
        UnserializableKind::RawNode,
        top(1),
    );

    assert_kind_path(
        RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("x".into())]),
        UnserializableKind::UnexpectedNodeKind,
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![RdNode::RCode(r#""unterminated"#.into())],
        ),
        UnserializableKind::UnterminatedRLikeState,
        top(0),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![group(vec![raw()]), group(vec![])],
        ),
        UnserializableKind::RawNode,
        path(vec![top(0).segments()[0].clone(), child(0), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Figure,
            None,
            vec![group(vec![RdNode::Text("f".into())])],
        ),
        UnserializableKind::UnexpectedNodeKind,
        path(vec![top(0).segments()[0].clone(), child(0), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Title,
            None,
            vec![RdNode::tagged(RdTag::List, None, vec![raw()])],
        ),
        UnserializableKind::RawNode,
        path(vec![top(0).segments()[0].clone(), child(0), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![RdNode::Text("pkg\n".into()), group(vec![])],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "#ifdef".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![group(vec![]), group(vec![RdNode::Text("body\n".into())])],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "#ifdef".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![
                group(vec![RdNode::Text("pkg".into())]),
                group(vec![RdNode::Text("body\n".into())]),
            ],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "#ifdef".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![
                group(vec![RdNode::Text("pkg\n".into())]),
                group(vec![raw()]),
            ],
        ),
        UnserializableKind::RawNode,
        path(vec![top(0).segments()[0].clone(), child(1), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::IfDef,
            None,
            vec![
                group(vec![RdNode::Text("pkg\n".into())]),
                group(vec![RdNode::Text("body".into())]),
            ],
        ),
        UnserializableKind::ConditionalNotAtLineStart,
        path(vec![top(0).segments()[0].clone(), child(1)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text(String::new())]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::UnrepresentableLeaf,
        path(vec![
            top(0).segments()[0].clone(),
            RdPathSegment::Option,
            child(0),
        ]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("]".into())]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidOptionContent,
        path(vec![
            top(0).segments()[0].clone(),
            RdPathSegment::Option,
            child(0),
        ]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::tagged(
                RdTag::Emph,
                None,
                vec![RdNode::Text("]".into())],
            )]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidOptionContent,
        path(vec![
            top(0).segments()[0].clone(),
            RdPathSegment::Option,
            child(0),
            child(0),
        ]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::tagged(
                RdTag::Link,
                Some(vec![RdNode::Text("]".into())]),
                vec![RdNode::Text("x".into())],
            )]),
            vec![RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidOptionContent,
        path(vec![
            top(0).segments()[0].clone(),
            RdPathSegment::Option,
            child(0),
            RdPathSegment::Option,
            child(0),
        ]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![group(vec![]), RdNode::Text("x".into())],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\section".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(1)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Section,
            None,
            vec![RdNode::Text("a".into()), RdNode::Text("b".into())],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\section".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Title,
            None,
            vec![group(vec![RdNode::Text("x".into())])],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\title".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Arguments,
            None,
            vec![RdNode::tagged(
                RdTag::Item,
                None,
                vec![group(vec![]), group(vec![raw()])],
            )],
        ),
        UnserializableKind::RawNode,
        path(vec![
            top(0).segments()[0].clone(),
            child(0),
            child(1),
            child(0),
        ]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Arguments,
            None,
            vec![RdNode::tagged(
                RdTag::Item,
                None,
                vec![group(vec![]), RdNode::Text("x".into())],
            )],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\item".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0), child(1)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Arguments,
            None,
            vec![RdNode::tagged(RdTag::Item, None, vec![])],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\item".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
    assert_kind_path(
        RdNode::tagged(
            RdTag::Examples,
            None,
            vec![RdNode::tagged(
                RdTag::Figure,
                None,
                vec![
                    group(vec![RdNode::Text("f".into())]),
                    group(vec![RdNode::Text("o".into())]),
                ],
            )],
        ),
        UnserializableKind::InvalidTagShape {
            tag: "\\figure".into(),
        },
        path(vec![top(0).segments()[0].clone(), child(0)]),
    );
}

#[test]
fn ast_path_is_only_available_for_unserializable_errors() {
    assert!(write_error(raw()).ast_path().is_some());
    assert!(
        WriteError::Io {
            source: std::io::Error::other("test"),
        }
        .ast_path()
        .is_none()
    );
    assert!(
        WriteError::Verification {
            reason: "test".into(),
        }
        .ast_path()
        .is_none()
    );
}
