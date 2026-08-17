#[test]
fn raw_nodes_preserve_non_child_payloads_and_metadata() {
    let values = vec![
        RValue::Null,
        RValue::Integer(vec![Some(1), None]),
        RValue::Logical(vec![Some(true), None, Some(false)]),
        RValue::Real(vec![
            Some(1.5),
            None,
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
        ]),
        RValue::Environment(EnvHandle::Global),
    ];
    let expected = vec![
        RawRdValue::Null,
        RawRdValue::Integer(vec![Some(1), None]),
        RawRdValue::Logical(vec![Some(true), None, Some(false)]),
        RawRdValue::Real(vec![
            RawRdReal::Finite(1.5),
            RawRdReal::Na,
            RawRdReal::Nan,
            RawRdReal::PositiveInfinity,
            RawRdReal::NegativeInfinity,
        ]),
        RawRdValue::Environment(RawRdEnvironment::Global),
    ];
    for (value, expected) in values.into_iter().zip(expected) {
        let RdNode::Raw(raw) = lower_single_node(tagged_raw_value(value)) else {
            panic!("scalar must lower to Raw")
        };
        assert_eq!(raw.tag(), Some(r"\opaque"));
        assert!(raw.option().is_none());
        assert!(raw.children().is_empty());
        assert_eq!(raw.payload(), Some(&expected));
        assert_eq!(
            raw.attributes()
                .iter()
                .map(|a| a.name())
                .collect::<Vec<_>>(),
            vec!["class"]
        );
    }

    for value in [
        RValue::Character(vec![
            RStr::Na,
            RStr::new(
                &b"text"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            ),
        ]),
        RValue::List(vec![bare(RValue::Null)]),
    ] {
        let RdNode::Raw(raw) = lower_single_node(tagged_raw_value(value)) else {
            panic!("value must lower to Raw")
        };
        assert_eq!(raw.payload(), None);
        assert!(!raw.children().is_empty());
    }
}

#[test]
fn malformed_scalar_option_survives_as_raw_option_payload() {
    let node = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![
            attribute(
                "Rd_tag",
                RValue::Character(vec![RStr::new(
                    &br"\link"[..],
                    REncoding::Utf8,
                    NativeEncodingSource::Unknown,
                )]),
            ),
            attribute("Rd_option", RValue::Integer(vec![Some(9), None])),
        ]));
    let RdNode::Tagged(tagged) = lower_single_node(node) else {
        panic!("expected tagged option")
    };
    let option = tagged.option().expect("option preserved");
    let RdNode::Raw(raw) = &option[0] else {
        panic!("malformed option should be Raw")
    };
    assert_eq!(
        raw.payload(),
        Some(&RawRdValue::Integer(vec![Some(9), None]))
    );
    assert!(raw.children().is_empty());
}

#[test]
fn paired_ifelse_attributes_lower_without_changing_children() {
    let (root_value, _) = expanded_fragment_fixture().into_parts();
    let RValue::List(mut nodes) = root_value else {
        unreachable!("fixture root is a list")
    };
    let ifelse_pair = nodes.remove(0);
    let ifelse_without_metadata = nodes.remove(0);
    assert!(ifelse_pair.attributes().get("class").is_some());
    assert!(ifelse_pair.attributes().get("Rdfile").is_some());
    assert!(ifelse_pair.attributes().get("srcref").is_some());

    let paired = lower_single_node(ifelse_pair);
    let baseline = lower_single_node(ifelse_without_metadata);
    let (paired_children, baseline_children) = match (paired, baseline) {
        (RdNode::Tagged(paired), RdNode::Tagged(baseline))
            if paired.tag() == &RdTag::IfElse && baseline.tag() == &RdTag::IfElse =>
        {
            (paired.children().to_vec(), baseline.children().to_vec())
        }
        other => panic!("unexpected lowered nodes: {other:?}"),
    };
    assert_eq!(paired_children, baseline_children);
}

#[test]
fn paired_href_attributes_lower_without_changing_children() {
    let (root_value, _) = expanded_fragment_fixture().into_parts();
    let RValue::List(mut nodes) = root_value else {
        unreachable!("fixture root is a list")
    };
    nodes.remove(0);
    nodes.remove(0);
    let href_pair = nodes.remove(0);
    let href_without_metadata = nodes.remove(0);
    assert!(href_pair.attributes().get("class").is_some());
    assert!(href_pair.attributes().get("Rdfile").is_some());

    let paired = lower_single_node(href_pair);
    let baseline = lower_single_node(href_without_metadata);
    let (paired_children, baseline_children) = match (paired, baseline) {
        (RdNode::Tagged(paired), RdNode::Tagged(baseline))
            if paired.tag() == &RdTag::Href && baseline.tag() == &RdTag::Href =>
        {
            (paired.children().to_vec(), baseline.children().to_vec())
        }
        other => panic!("unexpected lowered nodes: {other:?}"),
    };
    assert_eq!(paired_children, baseline_children);
}

#[test]
fn invalid_dynamic_flag_shapes_force_raw_and_preserve_attributes() {
    let template = dynamic_fixture_node(r"\description");
    let dynamic_flag = template
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "dynamicFlag")
        .expect("dynamicFlag attribute");
    let character = template
        .attributes()
        .get("Rd_tag")
        .expect("Rd_tag attribute")
        .value()
        .clone();

    let invalid_attribute_values = [
        RValue::Real(vec![Some(18.0)]),
        RValue::Character(match character {
            RValue::Character(values) => values,
            _ => unreachable!("Rd_tag is character"),
        }),
        RValue::Logical(vec![Some(true)]),
        RValue::Integer(Vec::new()),
        RValue::Integer(vec![Some(1), Some(2)]),
        RValue::Integer(vec![None]),
    ];

    let list_context = NodeContext {
        tag: Some(r"\description"),
        value: &RValue::List(Vec::new()),
        attributes: template.attributes(),
    };
    for value in invalid_attribute_values {
        let invalid_attribute = Attribute::new(
            dynamic_flag.name().clone(),
            RObject::from_parts(value, Default::default()),
        );
        assert_eq!(
            classify_attribute(&list_context, &invalid_attribute),
            AttributeDisposition::Reject
        );
    }

    let non_list_node = with_value(&template, RValue::Character(Vec::new()));
    let document = lower_r_object(&RObject::from_parts(RValue::List(vec![non_list_node]), Default::default()))
    .expect("invalid dynamicFlag is not a lowering error");
    let raw = &document.nodes()[0];
    assert!(matches!(raw, RdNode::Raw(_)));
    assert!(raw_attribute_names(raw).contains(&"dynamicFlag"));

    let text_context = NodeContext {
        tag: Some("TEXT"),
        value: &RValue::Character(Vec::new()),
        attributes: template.attributes(),
    };
    assert_eq!(
        classify_attribute(&text_context, dynamic_flag),
        AttributeDisposition::Reject
    );
}

#[test]
fn malformed_rd_tag_values_are_raw_and_lossless() {
    let tag_template = dynamic_fixture_node(r"\description")
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "Rd_tag")
        .expect("Rd_tag attribute")
        .clone();
    let tag_attribute = |value| attribute_with_value(&tag_template, value);
    let cases = [
        (RValue::Integer(vec![Some(42)]), "integer"),
        (RValue::Character(Vec::new()), "empty character vector"),
        (
            RValue::Character(vec![RStr::Na, RStr::Na]),
            "multi-element character vector",
        ),
        (RValue::Character(vec![RStr::Na]), "NA character scalar"),
    ];

    for (value, label) in cases {
        let node = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![
                tag_attribute(value),
                attribute("class", RValue::Integer(vec![Some(7)])),
            ]));
        let lowered = lower_single_node(node);
        let RdNode::Raw(raw) = lowered else {
            panic!("{label} Rd_tag should force Raw")
        };
        assert_eq!(
            raw.attributes()
                .iter()
                .map(|attribute| attribute.name())
                .collect::<Vec<_>>(),
            vec!["Rd_tag", "class"]
        );
        assert!(matches!(
            raw.attributes()[1].value().value(),
            RawRdValue::Integer(values) if values == &[Some(7)]
        ));
    }

    let mut nested = tag_attribute(RValue::Character(vec![RStr::Na]));
    let (value, _) = nested.value().clone().into_parts();
    nested = Attribute::new(
        nested.name().clone(),
        RObject::from_parts(
            value,
            Attributes::new(vec![attribute("class", RValue::Integer(vec![Some(1)]))]),
        ),
    );
    let lowered = lower_single_node(RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![nested, attribute("class", RValue::Null)])));
    let RdNode::Raw(raw) = lowered else {
        panic!("Rd_tag with attributes should force Raw")
    };
    let tag = raw
        .attributes()
        .iter()
        .find(|attribute| attribute.name() == "Rd_tag")
        .expect("Rd_tag preserved");
    assert_eq!(tag.value().attributes().len(), 1);

    let tag_template = fixture_attribute("Rd_tag");
    let duplicate = lower_single_node(RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![
            tag_attribute(RValue::Character(vec![RStr::new(
                &b"TEXT"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )])),
            tag_attribute(RValue::Character(vec![RStr::new(
                &b"OTHER"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )])),
            attribute("class", RValue::Null),
        ])));
    let RdNode::Raw(raw) = duplicate else {
        panic!("duplicate Rd_tag should force Raw")
    };
    assert_eq!(
        raw.attributes()
            .iter()
            .filter(|attribute| attribute.name() == "Rd_tag")
            .count(),
        2
    );
    assert!(
        raw.attributes()
            .iter()
            .any(|attribute| attribute.name() == "class")
    );

    let tag_value = |value: &str| {
        attribute_with_value(
            &tag_template,
            RValue::Character(vec![RStr::new(
                value.as_bytes(),
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )]),
        )
    };
    for (first, second) in [("TEXT", "RCODE"), ("RCODE", "TEXT")] {
        let lowered = lower_single_node(RObject::from_parts(
            RValue::Character(vec![RStr::new(
                &b"payload"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )]),
            Attributes::new(vec![tag_value(first), tag_value(second)]),
        ));
        let RdNode::Raw(raw) = lowered else {
            panic!("duplicate Rd_tag should force Raw")
        };
        assert_eq!(raw.tag(), None);
        assert_eq!(
            raw.attributes()
                .iter()
                .map(|attribute| attribute.name())
                .collect::<Vec<_>>(),
            vec!["Rd_tag", "Rd_tag"]
        );
        assert_eq!(
            raw.attributes()[0].value().value(),
            &RawRdValue::Character(vec![Some(first.into())])
        );
        assert_eq!(
            raw.attributes()[1].value().value(),
            &RawRdValue::Character(vec![Some(second.into())])
        );
        assert_eq!(raw.children(), &[RdNode::Text("payload".into())]);
    }
}

#[test]
fn malformed_rd_tag_on_list_option_is_raw_and_lossless() {
    let option = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![attribute(
            "Rd_tag",
            RValue::Character(vec![RStr::Na]),
        )]));
    let node = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![
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

    let RdNode::Tagged(link) = lower_single_node(node) else {
        panic!("valid link should lower structurally")
    };
    let [RdNode::Raw(raw)] = link.option().expect("option present") else {
        panic!("malformed tagged option should remain one Raw node")
    };
    assert_eq!(raw.tag(), None);
    assert_eq!(raw.attributes().len(), 1);
    assert_eq!(raw.attributes()[0].name(), "Rd_tag");
    assert!(matches!(
        raw.attributes()[0].value().value(),
        RawRdValue::Character(values) if values == &[None]
    ));
}

#[test]
fn nested_rd_option_on_list_option_is_raw_and_lossless() {
    let option = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![attribute(
            "Rd_option",
            RValue::Character(vec![RStr::new(
                &b"nested"[..],
                REncoding::Utf8,
                NativeEncodingSource::Unknown,
            )]),
        )]));
    let node = RObject::from_parts(RValue::List(Vec::new()), Attributes::new(vec![
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

    let RdNode::Tagged(link) = lower_single_node(node) else {
        panic!("valid link should lower structurally")
    };
    let [RdNode::Raw(raw)] = link.option().expect("outer option present") else {
        panic!("nested option wrapper should remain one Raw node")
    };
    assert_eq!(raw.tag(), None);
    assert_eq!(raw.option(), Some(&[RdNode::Text("nested".into())][..]));
    assert!(raw.attributes().is_empty());
}

#[test]
fn nested_dynamic_flag_attributes_are_rejected_and_preserved() {
    let template = dynamic_fixture_node(r"\description");
    let node = with_nested_metadata_attributes(&template, &["dynamicFlag"]);
    let dynamic_flag = node
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "dynamicFlag")
        .expect("dynamicFlag attribute");
    let context = NodeContext {
        tag: Some(r"\description"),
        value: node.value(),
        attributes: node.attributes(),
    };
    assert_eq!(
        classify_attribute(&context, dynamic_flag),
        AttributeDisposition::Reject
    );

    let lowered = lower_single_node(node);
    assert_raw_metadata_has_nested_attributes(&lowered, &["dynamicFlag"]);
}

#[test]
fn nested_list_class_attributes_are_rejected_and_preserved() {
    let template = fixture("rd_extra_attr_v2.rds");
    let node = with_nested_metadata_attributes(&template, &["class"]);
    let class = node
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "class")
        .expect("class attribute");
    let context = NodeContext {
        tag: Some("LIST"),
        value: &RValue::List(Vec::new()),
        attributes: node.attributes(),
    };
    assert_eq!(
        classify_attribute(&context, class),
        AttributeDisposition::Reject
    );

    let lowered = lower_single_node(RObject::from_parts(
        RValue::List(Vec::new()),
        node.attributes().clone(),
    ));
    assert_raw_metadata_has_nested_attributes(&lowered, &["class"]);
}

#[test]
fn nested_expanded_fragment_metadata_is_rejected_and_preserved() {
    let (root_value, _) = expanded_fragment_fixture().into_parts();
    let RValue::List(mut nodes) = root_value else {
        unreachable!("fixture root is a list")
    };
    let template = nodes.remove(0);

    let class_nested = with_nested_metadata_attributes(&template, &["class"]);
    let class_context = NodeContext {
        tag: Some(r"\ifelse"),
        value: class_nested.value(),
        attributes: class_nested.attributes(),
    };
    let class = class_nested
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "class")
        .expect("class attribute");
    assert_eq!(
        classify_attribute(&class_context, class),
        AttributeDisposition::Reject
    );
    let lowered = lower_single_node(class_nested);
    assert_raw_metadata_has_nested_attributes(&lowered, &["class"]);

    let rdfile_nested = with_nested_metadata_attributes(&template, &["Rdfile"]);
    let rdfile_context = NodeContext {
        tag: Some(r"\ifelse"),
        value: rdfile_nested.value(),
        attributes: rdfile_nested.attributes(),
    };
    let plain_class = rdfile_nested
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "class")
        .expect("class attribute");
    let rdfile = rdfile_nested
        .attributes()
        .iter()
        .find(|attribute| attribute.name().as_str() == "Rdfile")
        .expect("Rdfile attribute");
    assert_eq!(
        classify_attribute(&rdfile_context, plain_class),
        AttributeDisposition::Discard
    );
    assert_eq!(
        classify_attribute(&rdfile_context, rdfile),
        AttributeDisposition::Reject
    );
    let lowered = lower_single_node(rdfile_nested);
    assert_raw_metadata_has_nested_attributes(&lowered, &["Rdfile"]);
}

#[test]
fn valid_dynamic_flag_with_unknown_attribute_stays_raw() {
    let root = fixture("rd_options_v2.rds");
    assert!(root.attributes().get("dynamicFlag").is_some());

    let document = lower_r_object(&RObject::from_parts(RValue::List(vec![root]), Default::default()))
    .expect("lower document");
    let raw = &document.nodes()[0];
    assert!(matches!(raw, RdNode::Raw(_)));
    let names = raw_attribute_names(raw);
    assert!(names.contains(&"dynamicFlag"));
}

#[test]
fn preserves_each_raw_real_category() {
    let value = RValue::Real(vec![
        Some(1.25),
        None,
        Some(f64::NAN),
        Some(f64::INFINITY),
        Some(f64::NEG_INFINITY),
    ]);

    assert_eq!(
        lower_raw_value(&mut LowerContext::new(), None, None, &value).expect("lower real vector"),
        RawRdValue::Real(vec![
            RawRdReal::Finite(1.25),
            RawRdReal::Na,
            RawRdReal::Nan,
            RawRdReal::PositiveInfinity,
            RawRdReal::NegativeInfinity,
        ])
    );
}
