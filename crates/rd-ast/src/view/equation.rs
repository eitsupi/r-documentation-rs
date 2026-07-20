use super::*;

/// Whether an equation is rendered inline by `\eqn` or as a block by `\deqn`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdEquationDisplay {
    Inline,
    Block,
}

/// A borrowed, structurally valid `\eqn{latex}{ascii}` or `\deqn{latex}{ascii}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdEquation<'a> {
    path: RdPath,
    display: RdEquationDisplay,
    latex: &'a [RdNode],
    ascii: Option<&'a [RdNode]>,
}

impl<'a> RdEquation<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn display(&self) -> RdEquationDisplay {
        self.display
    }
    pub fn latex(&self) -> &'a [RdNode] {
        self.latex
    }
    pub fn ascii(&self) -> Option<&'a [RdNode]> {
        self.ascii
    }
}
impl RdTagged {
    /// Strictly inspects a `\eqn` or `\deqn` equation. The first argument is
    /// LaTeX and the optional second argument is its ASCII/text fallback.
    pub fn inspect_equation<'a>(
        &'a self,
        base_path: &RdPath,
    ) -> Result<RdEquation<'a>, RdShapeError> {
        let display = match self.tag() {
            RdTag::Eqn => RdEquationDisplay::Inline,
            RdTag::Deqn => RdEquationDisplay::Block,
            tag => {
                return Err(shape(
                    base_path.clone(),
                    Some(tag.clone()),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Equation,
                        actual: RdNodeKind::Tagged,
                    },
                ));
            }
        };
        if self.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        let children = self.children();
        if !(1..=2).contains(&children.len()) {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Range { min: 1, max: 2 },
                    actual: children.len(),
                },
            ));
        }
        for (index, child) in children.iter().enumerate() {
            if child.as_group().is_none() {
                return Err(shape(
                    base_path.with_child(index),
                    Some(self.tag().clone()),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Group,
                        actual: RdNodeKind::of(child),
                    },
                ));
            }
        }
        Ok(RdEquation {
            path: base_path.clone(),
            display,
            latex: children[0].as_group().unwrap().children(),
            ascii: children
                .get(1)
                .map(|child| child.as_group().unwrap().children()),
        })
    }
}
