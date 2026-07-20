use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdConditionalKind {
    If,
    IfElse,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdConditional<'a> {
    path: RdPath,
    kind: RdConditionalKind,
    format_nodes: &'a [RdNode],
    format: String,
    then_branch: &'a [RdNode],
    else_branch: Option<&'a [RdNode]>,
}

impl<'a> RdConditional<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn kind(&self) -> RdConditionalKind {
        self.kind
    }
    pub fn format_nodes(&self) -> &'a [RdNode] {
        self.format_nodes
    }
    /// An exact Text-leaf scalar; branch selection is consumer policy.
    pub fn format(&self) -> &str {
        &self.format
    }
    pub fn then_branch(&self) -> &'a [RdNode] {
        self.then_branch
    }
    pub fn else_branch(&self) -> Option<&'a [RdNode]> {
        self.else_branch
    }
}

fn kind(tag: &RdTag) -> Option<RdConditionalKind> {
    match tag {
        RdTag::If => Some(RdConditionalKind::If),
        RdTag::IfElse => Some(RdConditionalKind::IfElse),
        _ => None,
    }
}

impl RdNode {
    pub fn conditional(&self, base_path: &RdPath) -> Option<RdConditional<'_>> {
        self.inspect_conditional(base_path).ok().flatten()
    }

    pub fn inspect_conditional(
        &self,
        base_path: &RdPath,
    ) -> Result<Option<RdConditional<'_>>, RdShapeError> {
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
        let expected = match kind {
            RdConditionalKind::If => 2,
            RdConditionalKind::IfElse => 3,
        };
        let children = tagged.children();
        if children.len() != expected {
            return Err(shape(
                base_path.clone(),
                Some(tagged.tag().clone()),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(expected),
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
        let format_nodes = children[0].as_group().unwrap().children();
        let format = concat_exact(
            format_nodes,
            RdNodeKind::Text,
            &base_path.with_child(0),
            tagged.tag(),
        )?;
        Ok(Some(RdConditional {
            path: base_path.clone(),
            kind,
            format_nodes,
            format,
            then_branch: children[1].as_group().unwrap().children(),
            else_branch: (kind == RdConditionalKind::IfElse)
                .then(|| children[2].as_group().unwrap().children()),
        }))
    }
}
