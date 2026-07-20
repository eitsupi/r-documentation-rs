use rd_ast::{RdDocument, RdNode, RdTag};
use rd_writer::{UnserializableKind, WriteError};

fn error(node: RdNode) -> UnserializableKind {
    match rd_writer::write_document(&RdDocument::from(vec![node])).unwrap_err() {
        WriteError::Unserializable { kind, .. } => kind,
        WriteError::Io { .. } => panic!("unexpected I/O error"),
        _ => panic!("unexpected non-exhaustive error variant"),
    }
}

fn document_error(nodes: Vec<RdNode>) -> UnserializableKind {
    match rd_writer::write_document(&RdDocument::from(nodes)).unwrap_err() {
        WriteError::Unserializable { kind, .. } => kind,
        _ => panic!("unexpected error"),
    }
}

#[test]
fn rejects_raw_empty_and_unknown_tags() {
    let raw = rd_ast::producer::raw_node(None, None, vec![], None, vec![]);
    assert_eq!(error(RdNode::Raw(raw)), UnserializableKind::RawNode);
    assert_eq!(
        error(RdNode::Text(String::new())),
        UnserializableKind::UnrepresentableLeaf
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Unknown("USERMACRO".into()),
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::UnknownTag
    );
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Unknown(r"\name".into()),
            None,
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::UnknownTag
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
    let unterminated = RdNode::tagged(
        RdTag::Examples,
        None,
        vec![RdNode::RCode(r#""unterminated"#.into())],
    );
    assert_eq!(
        error(unterminated),
        UnserializableKind::UnterminatedRLikeState
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
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Eqn,
            None,
            vec![RdNode::Verb(r"abc\".into())]
        )),
        UnserializableKind::UnrepresentableEquation
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
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Eqn,
            None,
            vec![RdNode::Verb("{".into())]
        )),
        UnserializableKind::UnrepresentableEquation
    );
    assert_eq!(
        error(RdNode::Text("x\r".into())),
        UnserializableKind::UnrepresentableLeaf
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
    assert_eq!(
        error(RdNode::tagged(
            RdTag::Link,
            Some(vec![RdNode::Text("]".into())]),
            vec![RdNode::Text("x".into())]
        )),
        UnserializableKind::InvalidOptionContent
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
}

#[test]
fn rejects_contextual_tag_shapes() {
    let item = |children| RdNode::tagged(RdTag::Item, None, children);
    let group = |text: &str| RdNode::Group(rd_ast::RdGroup::from(vec![RdNode::Text(text.into())]));
    for parent in [RdTag::Itemize, RdTag::Enumerate] {
        assert!(matches!(
            error(RdNode::tagged(
                parent,
                None,
                vec![item(vec![RdNode::Text("x".into())])]
            )),
            UnserializableKind::InvalidTagShape { .. }
        ));
    }
    for parent in [RdTag::Arguments, RdTag::Value, RdTag::Describe] {
        assert!(matches!(
            error(RdNode::tagged(parent, None, vec![item(vec![])])),
            UnserializableKind::InvalidTagShape { .. }
        ));
    }
    let figure = RdNode::tagged(RdTag::Figure, None, vec![group("f"), group("o")]);
    let examples = RdNode::tagged(RdTag::Examples, None, vec![figure]);
    assert!(matches!(
        error(examples),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert!(matches!(
        error(RdNode::tagged(
            RdTag::Section,
            None,
            vec![RdNode::Text("a".into()); 3]
        )),
        UnserializableKind::InvalidTagShape { .. }
    ));
    assert!(matches!(
        error(RdNode::tagged(RdTag::Title, None, vec![group("a")])),
        UnserializableKind::InvalidTagShape { .. }
    ));
}
