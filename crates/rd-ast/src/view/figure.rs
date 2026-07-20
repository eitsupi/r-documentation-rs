//! A borrowed view of `\figure` arguments.
//!
//! In an R-like context such as `\usage{\figure{f}{o}}`, the parser gives
//! `\figure` only its first brace argument and the second brace remains RCode
//! content of the parent. This view therefore sees a one-argument figure and
//! MUST NOT absorb siblings as a second argument. Attribute parsing and image
//! alt-text policy belong to consumers.

use super::*;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdFigureSecondArgument<'a> {
    AltText {
        nodes: &'a [RdNode],
        text: String,
    },
    Options {
        nodes: &'a [RdNode],
        attributes: String,
    },
}

impl<'a> RdFigureSecondArgument<'a> {
    pub fn nodes(&self) -> &'a [RdNode] {
        match self {
            Self::AltText { nodes, .. } | Self::Options { nodes, .. } => nodes,
        }
    }
    pub fn alt_text(&self) -> Option<&str> {
        match self {
            Self::AltText { text, .. } => Some(text),
            Self::Options { .. } => None,
        }
    }
    pub fn option_attributes(&self) -> Option<&str> {
        match self {
            Self::AltText { .. } => None,
            Self::Options { attributes, .. } => Some(attributes),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdFigure<'a> {
    path: RdPath,
    file_nodes: &'a [RdNode],
    file: String,
    second: Option<RdFigureSecondArgument<'a>>,
}

impl<'a> RdFigure<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn file_nodes(&self) -> &'a [RdNode] {
        self.file_nodes
    }
    pub fn file(&self) -> &str {
        &self.file
    }
    pub fn second(&self) -> Option<&RdFigureSecondArgument<'a>> {
        self.second.as_ref()
    }
}

impl RdNode {
    pub fn figure(&self, base_path: &RdPath) -> Option<RdFigure<'_>> {
        self.inspect_figure(base_path).ok().flatten()
    }
    pub fn inspect_figure(&self, base_path: &RdPath) -> Result<Option<RdFigure<'_>>, RdShapeError> {
        let tagged = match self {
            RdNode::Tagged(tagged) => tagged,
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if tag == RdTag::Figure {
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
        if tagged.tag() != &RdTag::Figure {
            return Ok(None);
        }
        if tagged.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Figure),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        let children = tagged.children();
        if !(1..=2).contains(&children.len()) {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Figure),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Range { min: 1, max: 2 },
                    actual: children.len(),
                },
            ));
        }
        let mut groups = Vec::with_capacity(children.len());
        for (index, child) in children.iter().enumerate() {
            let Some(group) = child.as_group() else {
                return Err(shape(
                    base_path.with_child(index),
                    Some(RdTag::Figure),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Group,
                        actual: RdNodeKind::of(child),
                    },
                ));
            };
            groups.push(group.children());
        }
        let file_nodes = groups[0];
        let file = concat_exact(
            file_nodes,
            RdNodeKind::Verb,
            &base_path.with_child(0),
            &RdTag::Figure,
        )?;
        let second = groups
            .get(1)
            .map(|nodes| {
                let text = concat_exact(
                    nodes,
                    RdNodeKind::Verb,
                    &base_path.with_child(1),
                    &RdTag::Figure,
                )?;
                Ok(
                    if let Some(attributes) = text
                        .strip_prefix("options:")
                        .filter(|rest| rest.chars().next().is_some_and(char::is_whitespace))
                    {
                        RdFigureSecondArgument::Options {
                            nodes,
                            attributes: attributes.trim_start().to_owned(),
                        }
                    } else {
                        RdFigureSecondArgument::AltText { nodes, text }
                    },
                )
            })
            .transpose()?;
        Ok(Some(RdFigure {
            path: base_path.clone(),
            file_nodes,
            file,
            second,
        }))
    }
}
