use super::*;
use crate::{RdFigureSecondArgument, producer};

fn base() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(2)])
}
fn group(nodes: Vec<RdNode>) -> RdNode {
    RdNode::group(nodes)
}
fn figure(children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(RdTag::Figure, None, children)
}
fn verb(value: &str) -> RdNode {
    RdNode::Verb(value.into())
}

#[test]
fn figures_project_filename_and_second_argument() {
    let node = figure(vec![group(vec![verb("img."), verb("png")])]);
    let view = node.inspect_figure(&base()).unwrap().unwrap();
    assert_eq!(view.file(), "img.png");
    assert_eq!(view.second(), None);
    assert_eq!(node.figure(&base()), Some(view));

    for (text, expected) in [
        ("R logo", "R logo"),
        ("options:nospace", "options:nospace"),
        ("options are shown here", "options are shown here"),
    ] {
        let node = figure(vec![group(vec![verb("f")]), group(vec![verb(text)])]);
        let view = node.figure(&base()).unwrap();
        assert_eq!(view.second().unwrap().alt_text(), Some(expected));
        assert_eq!(view.second().unwrap().option_attributes(), None);
    }
    let options_node = figure(vec![
        group(vec![verb("f")]),
        group(vec![verb("options:  a"), verb(" b ")]),
    ]);
    let options = options_node.figure(&base()).unwrap();
    assert_eq!(options.second().unwrap().option_attributes(), Some("a b "));
    let tab_node = figure(vec![
        group(vec![verb("f")]),
        group(vec![verb("options:\talt='x'")]),
    ]);
    let tab = tab_node.figure(&base()).unwrap();
    assert_eq!(tab.second().unwrap().option_attributes(), Some("alt='x'"));
    assert!(matches!(
        options.second(),
        Some(RdFigureSecondArgument::Options { .. })
    ));
}

#[test]
fn figures_validate_shape_and_matching_raw() {
    let cases = [
        figure(vec![]),
        figure(vec![group(vec![]), group(vec![]), group(vec![])]),
        RdNode::tagged(RdTag::Figure, Some(vec![]), vec![]),
    ];
    for node in cases {
        assert!(node.figure(&base()).is_none());
    }
    let bad_group = figure(vec![RdNode::Text("f".into())]);
    assert!(matches!(
        bad_group.inspect_figure(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            ..
        }
    ));
    let bad_leaf = figure(vec![group(vec![RdNode::Text("f".into())])]);
    assert_eq!(
        bad_leaf.inspect_figure(&base()).unwrap_err().path(),
        &base().with_child(0).with_child(0)
    );
    let bad_nested = figure(vec![group(vec![RdNode::tagged(RdTag::Emph, None, vec![])])]);
    assert!(matches!(
        bad_nested.inspect_figure(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedContent {
            actual: RdNodeKind::Tagged
        }
    ));
    let raw = RdNode::Raw(producer::raw_node(
        Some(r"\figure".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(matches!(
        raw.inspect_figure(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw
        }
    ));
    let unrelated = RdNode::Raw(producer::raw_node(
        Some(r"\link".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert_eq!(unrelated.inspect_figure(&base()).unwrap(), None);
}

#[test]
fn lifecycle_filename_is_verbatim() {
    let node = figure(vec![group(vec![verb("lifecycle-stable.svg")])]);
    assert_eq!(node.figure(&base()).unwrap().file(), "lifecycle-stable.svg");
}
