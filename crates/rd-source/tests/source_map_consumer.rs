use rd_ast::{RdAstPath, RdAstPathSegment};

#[test]
fn positioned_consumers_can_join_paths_to_exact_source_spans() {
    let input = br#"\link[pkg]{topic}"#;
    let parsed = rd_source::parse(input).expect("valid link source");
    let document = parsed.document();
    let root = RdAstPath::new(Vec::new());
    let link = document
        .top_level()
        .get(0)
        .expect("one top-level link node");
    let link_path = RdAstPath::new(vec![RdAstPathSegment::TopLevel(0)]);

    assert_eq!(link.path(), &link_path);
    assert_eq!(
        parsed.source_map().span(&root).unwrap().bytes(),
        0..input.len()
    );
    assert_eq!(
        parsed.source_map().span(link.path()).unwrap().bytes(),
        0..input.len()
    );

    let option = link.option().expect("present link option");
    assert_eq!(option.path(), &link_path.with_option());
    assert_eq!(
        parsed.source_map().span(option.path()).unwrap().bytes(),
        5..10
    );
    let option_value = option.get(0).expect("one option value node");
    assert_eq!(
        parsed
            .source_map()
            .span(option_value.path())
            .unwrap()
            .bytes(),
        6..9
    );

    let topic = link.children().get(0).expect("one link display node");
    assert_eq!(topic.path(), &link_path.with_child(0));
    assert_eq!(
        parsed.source_map().span(topic.path()).unwrap().bytes(),
        11..16
    );
}
