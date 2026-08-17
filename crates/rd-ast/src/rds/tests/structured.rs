use super::*;

#[test]
fn lowers_untagged_list_to_group() {
    assert!(matches!(lower_bare(RValue::List(vec![])), RdNode::Group(_)));
}

#[test]
fn nested_positional_groups_preserve_order_and_content() {
    let node = lower_bare(RValue::List(vec![
        bare(RValue::Character(vec![RStr::Na])),
        bare(RValue::List(vec![bare(RValue::Character(vec![RStr::Na]))])),
    ]));
    let RdNode::Group(group) = node else {
        panic!("expected group")
    };
    assert_eq!(group.children().len(), 2);
    assert!(matches!(group.children()[1], RdNode::Group(_)));
}

#[test]
fn untagged_list_with_only_srcref_is_group() {
    let node = lower_r_object(&bare(RValue::List(vec![bare_with_attribute(
        RValue::List(vec![]),
        "srcref",
    )])))
    .expect("lower node")
    .into_nodes()
    .pop()
    .unwrap();
    assert!(matches!(node, RdNode::Group(_)));
}

#[test]
fn untagged_list_with_unknown_attribute_is_raw() {
    let node = lower_r_object(&bare(RValue::List(vec![bare_with_attribute(
        RValue::List(vec![]),
        "rootextra",
    )])))
    .expect("lower node")
    .into_nodes()
    .pop()
    .unwrap();
    assert!(matches!(node, RdNode::Raw(raw) if raw.tag().is_none()));
}

#[test]
fn untagged_non_list_value_is_raw() {
    let node = lower_bare(RValue::Null);
    assert!(
        matches!(node, RdNode::Raw(raw) if raw.tag().is_none() && raw.children().is_empty() && raw.payload() == Some(&RawRdValue::Null))
    );
}

#[test]
fn lowers_minimal_rd_fixture() {
    for version in [2, 3] {
        let root = fixture(&format!("rd_minimal_v{version}.rds"));
        let doc = lower_r_object(&root).expect("lower document");
        let mut tags = Vec::new();
        walk_nodes(doc.nodes(), &mut |node| {
            if let Some(tag) = tag_name(node) {
                tags.push(tag.to_string());
            }
        });

        assert!(tags.contains(&r"\name".to_string()), r"missing \name");
        assert!(tags.contains(&r"\title".to_string()), r"missing \title");
        assert!(
            tags.contains(&r"\description".to_string()),
            r"missing \description"
        );
    }
}

/// The seealso fixture is parsed from a real `.Rd` file on disk, so
/// every node carries a genuine `srcref` whose value itself carries a
/// nested `srcfile` attribute (an inline environment here; real
/// installed-package help databases persist it as a `PERSISTSXP`
/// instead, which the `rd-helpdb` oracle tests cover against real
/// packages). Per the module documentation's attribute policy, that entire
/// srcref value is discarded by name without inspection, so these
/// sections must lower to structured [`RdNode::Tagged`] nodes, not
/// fall back to Raw.
#[test]
fn lowers_seealso_fixture_structurally_despite_srcref_with_srcfile() {
    for version in [2, 3] {
        let root = fixture(&format!("rd_seealso_v{version}.rds"));
        let doc = lower_r_object(&root).expect("lower document");
        let mut tagged = Vec::new();
        walk_nodes(doc.nodes(), &mut |node| {
            if let RdNode::Tagged(tagged_node) = node {
                tagged.push(tagged_node.tag().clone());
            }
        });

        assert!(tagged.contains(&RdTag::SeeAlso));
        assert!(tagged.contains(&RdTag::Examples));
        assert!(tagged.contains(&RdTag::Usage));
    }
}

#[test]
fn lowers_character_options_as_text() {
    for version in [2, 3] {
        let root = fixture(&format!("rd_options_v{version}.rds"));
        let doc = lower_r_object(&root).expect("lower document");
        let mut options = Vec::new();
        walk_nodes(doc.nodes(), &mut |node| match node {
            RdNode::Tagged(tagged) if matches!(tagged.tag(), RdTag::Link | RdTag::Sexpr) => {
                options.push(tagged.option().map(|nodes| nodes.to_vec()));
            }
            RdNode::Raw(raw) if matches!(raw.tag(), Some(r"\link" | r"\Sexpr")) => {
                options.push(raw.option().map(ToOwned::to_owned));
            }
            _ => {}
        });

        assert_eq!(
            options,
            vec![
                Some(vec![RdNode::Text("stats".to_string())]),
                Some(vec![RdNode::Text("results=rd".to_string())]),
            ]
        );
    }
}

#[test]
fn lowers_tagged_list_options_as_tagged_nodes() {
    let option = RObject::from_parts(RValue::List(vec![RObject::from_parts(RValue::Character(vec![RStr::new(
                &b"pkg"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )]), Attributes::new(vec![attribute(
                "Rd_tag",
                RValue::Character(vec![RStr::new(
                    &b"TEXT"[..],
                    REncoding::Utf8,
                    NativeEncodingSource::Unknown,
                )]),
            )]))]), Attributes::new(vec![attribute(
            "Rd_tag",
            RValue::Character(vec![RStr::new(
                &br"\emph"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )]),
        )]));
    let link = RObject::from_parts(RValue::List(vec![bare(RValue::Character(vec![RStr::new(
            &b"target"[..],
            REncoding::Utf8,
            NativeEncodingSource::Unknown,
        )]))]), Attributes::new(vec![
            attribute(
                "Rd_tag",
                RValue::Character(vec![RStr::new(
                    &br"\link"[..],
                    REncoding::Utf8,
                    NativeEncodingSource::Unknown,
                )]),
            ),
            Attribute::new(rd_rds::Symbol::from("Rd_option"), option),
        ]));

    let document = lower_r_object(&bare(RValue::List(vec![link]))).expect("lower document");
    let RdNode::Tagged(link) = &document.nodes()[0] else {
        panic!("expected tagged link")
    };
    let [RdNode::Tagged(emph)] = link.option().expect("option present") else {
        panic!("expected one tagged option node")
    };
    assert_eq!(emph.tag(), &RdTag::Emph);
    assert_eq!(emph.children(), &[RdNode::Text("pkg".to_string())]);
}

/// The arguments fixture (parsed with srcref on every node) must now
/// lower structurally: `\arguments` is a [`RdNode::Tagged`] node whose
/// direct children include five Tagged `\item`s, each pairing two
/// positional groups, which must now lower to `Group` and leave no Raw
/// nodes in this fixture.
#[test]
fn lowers_arguments_fixture_structurally_discarding_srcref() {
    for version in [2, 3] {
        let root = fixture(&format!("rd_arguments_v{version}.rds"));
        let doc = lower_r_object(&root).expect("lower document");

        // srcref-only metadata never forces Raw, including on positional
        // groups.
        walk_nodes(doc.nodes(), &mut |node| {
            if let RdNode::Raw(raw) = node {
                assert_eq!(
                    raw.tag(),
                    None,
                    "unexpected Raw fallback for a tagged node: {raw:?}"
                );
            }
        });

        let arguments = doc
            .nodes()
            .iter()
            .find_map(|node| match node {
                RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Arguments => {
                    Some(tagged.children())
                }
                _ => None,
            })
            .expect(r"\arguments lowers to Tagged");

        let items: Vec<&[RdNode]> = arguments
            .iter()
            .filter_map(|node| match node {
                RdNode::Tagged(tagged)
                    if tagged.tag() == &RdTag::Item && tagged.option().is_none() =>
                {
                    Some(tagged.children())
                }
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), 5, r"expected five Tagged \item entries");
        for children in items {
            assert!(
                matches!(children, [RdNode::Group(_), RdNode::Group(_)]),
                "expected two untagged positional groups, got {children:?}"
            );
        }
    }
}
