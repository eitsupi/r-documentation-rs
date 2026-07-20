use super::*;

fn delimiter_item() -> RdNode {
    RdNode::tagged(RdTag::Item, None, vec![])
}

#[test]
fn lists_split_delimited_items_without_synthesizing_labels() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(4), RdPathSegment::Child(2)]);
    let list = RdTagged::new(
        RdTag::Itemize,
        None,
        vec![
            RdNode::Text("\n".into()),
            RdNode::Text("[leading error]".into()),
            delimiter_item(),
            RdNode::Text("[label] body".into()),
            RdNode::tagged(RdTag::Strong, None, vec![RdNode::Text("nested".into())]),
            delimiter_item(),
            delimiter_item(),
            RdNode::Text("last".into()),
        ],
    );
    let view = list.inspect_list(&base).unwrap();
    let mut items = view.items();
    assert!(matches!(items.next().unwrap(), Err(error) if error.path() == &base.with_child(1)));
    let first = match items.next().unwrap().unwrap() {
        RdListItem::Delimited(item) => item,
        _ => unreachable!(),
    };
    assert!(
        matches!(first.body(), [RdNode::Text(label), RdNode::Tagged(_),] if label == "[label] body")
    );
    assert!(
        matches!(items.next().unwrap(), Ok(RdListItem::Delimited(item)) if item.body().is_empty())
    );
    assert!(
        matches!(items.next().unwrap(), Ok(RdListItem::Delimited(item)) if matches!(item.body(), [RdNode::Text(text)] if text == "last"))
    );
    assert!(items.next().is_none());
}

#[test]
fn enumerate_has_the_same_delimiter_semantics() {
    let list = RdTagged::new(
        RdTag::Enumerate,
        None,
        vec![delimiter_item(), RdNode::Text("one".into())],
    );
    let view = list.inspect_list(&RdPath::new(vec![])).unwrap();
    assert_eq!(view.kind(), RdListKind::Enumerate);
    assert!(
        matches!(view.items().next().unwrap(), Ok(RdListItem::Delimited(item)) if item.body().len() == 1)
    );
}

#[test]
fn describe_items_validate_two_groups_and_keep_scanning() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(3)]);
    let item = |children| RdNode::tagged(RdTag::Item, None, children);
    let list = RdTagged::new(
        RdTag::Describe,
        None,
        vec![
            item(vec![
                RdNode::group(vec![RdNode::Text("a".into())]),
                RdNode::group(vec![RdNode::Text("A".into())]),
            ]),
            item(vec![]),
            item(vec![RdNode::Text("bad".into()), RdNode::group(vec![])]),
            RdNode::Text("content".into()),
            item(vec![RdNode::group(vec![]), RdNode::group(vec![])]),
        ],
    );
    let results: Vec<_> = list.inspect_list(&base).unwrap().items().collect();
    assert!(matches!(&results[0], Ok(RdListItem::Described(item)) if item.label().len() == 1));
    assert!(
        matches!(&results[1], Err(error) if matches!(error.kind(), RdShapeErrorKind::WrongArity { expected: RdArity::Exactly(2), actual: 0 }))
    );
    assert!(matches!(&results[2], Err(error) if error.path() == &base.with_child(2).with_child(0)));
    assert!(matches!(&results[3], Err(error) if error.path() == &base.with_child(3)));
    assert!(matches!(&results[4], Ok(RdListItem::Described(_))));
}

#[test]
fn inspect_list_reports_wrong_tag_and_container_option() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(1)]);
    let wrong = RdTagged::new(RdTag::Title, None, vec![])
        .inspect_list(&base)
        .unwrap_err();
    assert!(matches!(
        wrong.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::List,
            ..
        }
    ));
    assert_eq!(
        wrong.to_string(),
        r"expected list node, found tagged node for \title at top-level[1]"
    );
    let option = RdTagged::new(RdTag::Itemize, Some(vec![]), vec![])
        .inspect_list(&base)
        .unwrap_err();
    assert!(matches!(option.kind(), RdShapeErrorKind::UnexpectedOption));
}
