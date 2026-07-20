use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdMethodKind {
    Method,
    S3Method,
    S4Method,
}

#[derive(Debug, Clone, PartialEq)]
/// A method usage node with exact Text-scalar generic and qualifier arguments.
/// The following call syntax `(x, ...)` in `\usage` is not part of this
/// method node; it remains a sibling `RCode` leaf of the parent section.
pub struct RdMethod<'a> {
    path: RdPath,
    kind: RdMethodKind,
    generic_nodes: &'a [RdNode],
    generic: String,
    qualifier_nodes: &'a [RdNode],
    qualifier: String,
}

impl<'a> RdMethod<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn kind(&self) -> RdMethodKind {
        self.kind
    }
    pub fn generic_nodes(&self) -> &'a [RdNode] {
        self.generic_nodes
    }
    pub fn generic(&self) -> &str {
        &self.generic
    }
    pub fn qualifier_nodes(&self) -> &'a [RdNode] {
        self.qualifier_nodes
    }
    /// Class for `\method`/`\S3method`, signature for `\S4method`; consumers branch on `kind()`.
    pub fn qualifier(&self) -> &str {
        &self.qualifier
    }
}

fn kind(tag: &RdTag) -> Option<RdMethodKind> {
    match tag {
        RdTag::Method => Some(RdMethodKind::Method),
        RdTag::S3Method => Some(RdMethodKind::S3Method),
        RdTag::S4Method => Some(RdMethodKind::S4Method),
        _ => None,
    }
}

impl RdNode {
    pub fn method(&self, base_path: &RdPath) -> Option<RdMethod<'_>> {
        self.inspect_method(base_path).ok().flatten()
    }

    pub fn inspect_method(&self, base_path: &RdPath) -> Result<Option<RdMethod<'_>>, RdShapeError> {
        let tagged = match self {
            RdNode::Tagged(tagged) => tagged,
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if kind(&tag).is_some() {
                    return Err(shape(
                        base_path.clone(),
                        Some(tag),
                        RdShapeErrorKind::UnexpectedNode {
                            expected: RdExpectedNode::Tagged,
                            actual: RdNodeKind::Raw,
                        },
                    ));
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };
        let Some(kind) = kind(tagged.tag()) else {
            return Ok(None);
        };
        if tagged.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(tagged.tag().clone()),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        let children = tagged.children();
        if children.len() != 2 {
            return Err(shape(
                base_path.clone(),
                Some(tagged.tag().clone()),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(2),
                    actual: children.len(),
                },
            ));
        }
        for (index, child) in children.iter().enumerate() {
            if child.as_group().is_none() {
                return Err(shape(
                    base_path.with_child(index),
                    Some(tagged.tag().clone()),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Group,
                        actual: RdNodeKind::of(child),
                    },
                ));
            }
        }
        let generic_nodes = children[0].as_group().unwrap().children();
        let qualifier_nodes = children[1].as_group().unwrap().children();
        let generic = concat_exact(
            generic_nodes,
            RdNodeKind::Text,
            &base_path.with_child(0),
            tagged.tag(),
        )?;
        let qualifier = concat_exact(
            qualifier_nodes,
            RdNodeKind::Text,
            &base_path.with_child(1),
            tagged.tag(),
        )?;
        Ok(Some(RdMethod {
            path: base_path.clone(),
            kind,
            generic_nodes,
            generic,
            qualifier_nodes,
            qualifier,
        }))
    }
}
