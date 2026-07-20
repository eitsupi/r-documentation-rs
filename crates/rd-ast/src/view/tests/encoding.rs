use super::*;
use crate::{RdEnc, RdNode, RdPath, RdPathSegment, RdTag, producer};

fn base() -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(1), RdPathSegment::Child(3)])
}
fn group(nodes: Vec<RdNode>) -> RdNode {
    RdNode::group(nodes)
}
fn enc(children: Vec<RdNode>) -> RdNode {
    RdNode::tagged(RdTag::Enc, None, children)
}

#[test]
fn enc_preserves_both_sides() {
    let encoded = vec![RdNode::tagged(
        RdTag::Emph,
        None,
        vec![RdNode::Text("é".into())],
    )];
    let ascii = vec![RdNode::Text("a".into()), RdNode::Text("scii".into())];
    let node = enc(vec![group(encoded.clone()), group(ascii.clone())]);
    let view: RdEnc<'_> = node.inspect_enc(&base()).unwrap().unwrap();
    assert_eq!(view.encoded(), encoded.as_slice());
    assert_eq!(view.ascii(), ascii.as_slice());
    assert_eq!(node.enc(&base()), Some(view));
}

#[test]
fn enc_validates_only_option_arity_and_groups() {
    let option = RdNode::tagged(RdTag::Enc, Some(vec![]), vec![]);
    assert!(matches!(
        option.inspect_enc(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    for count in [0, 1, 3] {
        let node = enc((0..count).map(|_| group(vec![])).collect());
        assert!(
            matches!(node.inspect_enc(&base()).unwrap_err().kind(), RdShapeErrorKind::WrongArity { actual, .. } if *actual == count)
        );
    }
    for index in 0..2 {
        let mut children = vec![group(vec![]), group(vec![])];
        children[index] = RdNode::Text("bad".into());
        let error = enc(children).inspect_enc(&base()).unwrap_err();
        assert_eq!(error.path(), &base().with_child(index));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Group,
                ..
            }
        ));
    }
    let raw = RdNode::Raw(producer::raw_node(
        Some("\\enc".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    assert!(matches!(
        raw.inspect_enc(&base()).unwrap_err().kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Tagged,
            actual: RdNodeKind::Raw
        }
    ));
    let other = RdNode::tagged(RdTag::If, None, vec![]);
    assert_eq!(other.inspect_enc(&base()).unwrap(), None);
}
