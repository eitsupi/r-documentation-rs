use super::*;
use crate::{
    RdArity, RdColumnAlign, RdConstruct, RdExpectedNode, RdLinkDestination, RdLinkTopic,
    RdListItem, RdListKind, RdNode, RdNodeKind, RdPathSegment, RdSexprResults, RdShapeErrorKind,
    RdTag,
};

fn raw_group(children: Vec<RdNode>) -> RdNode {
    RdNode::group(children)
}

fn item(name: Vec<RdNode>, description: Vec<RdNode>) -> RdNode {
    RdNode::tagged(
        RdTag::Item,
        None,
        vec![raw_group(name), raw_group(description)],
    )
}

mod conditional;
mod dynamic;
mod encoding;
mod equation;
mod example;
mod figure;
mod generation;
mod inline;
mod lifecycle;
mod link;
mod list;
mod method;
mod system_macro;
mod tabular;
mod text_document;
