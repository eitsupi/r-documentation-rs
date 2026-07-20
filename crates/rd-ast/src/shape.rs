//! Shared diagnostics for malformed or unexpected Rd tree shapes.

use std::fmt;

use crate::{RdNode, RdTag};

/// A human-oriented explanation of a shape mismatch, including its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdShapeError {
    path: crate::RdPath,
    tag: Option<RdTag>,
    kind: RdShapeErrorKind,
}

impl RdShapeError {
    pub(crate) fn new(path: crate::RdPath, tag: Option<RdTag>, kind: RdShapeErrorKind) -> Self {
        Self { path, tag, kind }
    }
    pub fn path(&self) -> &crate::RdPath {
        &self.path
    }
    pub fn tag(&self) -> Option<&RdTag> {
        self.tag.as_ref()
    }
    pub fn kind(&self) -> &RdShapeErrorKind {
        &self.kind
    }
}

impl fmt::Display for RdShapeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        if let Some(tag) = &self.tag {
            write!(f, " for {}", tag.as_rd_tag())?;
        }
        write!(f, " at {}", self.path)
    }
}

impl std::error::Error for RdShapeError {}

/// The shared vocabulary of shape errors returned by strict views.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdShapeErrorKind {
    UnexpectedNode {
        expected: RdExpectedNode,
        actual: RdNodeKind,
    },
    UnexpectedOption,
    MissingOption,
    WrongArity {
        expected: RdArity,
        actual: usize,
    },
    Duplicate {
        construct: RdConstruct,
        first_path: crate::RdPath,
    },
    UnexpectedContent {
        actual: RdNodeKind,
    },
    InvalidValue {
        construct: RdConstruct,
        value: String,
    },
    CountMismatch {
        construct: RdConstruct,
        expected: usize,
        actual: usize,
    },
    MissingFollowing {
        construct: RdConstruct,
    },
    DefinitionMismatch {
        construct: RdConstruct,
    },
    UnsupportedRepresentation {
        construct: RdConstruct,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdNodeKind {
    Text,
    RCode,
    Verb,
    Comment,
    Tagged,
    Group,
    Raw,
}

impl RdNodeKind {
    pub fn of(node: &RdNode) -> Self {
        match node {
            RdNode::Text(_) => Self::Text,
            RdNode::RCode(_) => Self::RCode,
            RdNode::Verb(_) => Self::Verb,
            RdNode::Comment(_) => Self::Comment,
            RdNode::Tagged(_) => Self::Tagged,
            RdNode::Group(_) => Self::Group,
            RdNode::Raw(_) => Self::Raw,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdArity {
    Exactly(usize),
    Range { min: usize, max: usize },
    AtLeast(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdExpectedNode {
    Tagged,
    Link,
    Href,
    List,
    Tabular,
    Equation,
    Sexpr,
    RdOpts,
    Group,
    Item,
    TextLike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdConstruct {
    Tag(RdTag),
    OptionKey(String),
    ColumnSpec,
    TableRow,
    SystemMacro(String),
    SystemMacroExpansion(String),
}

/// Returns whether a node is ignorable trivia between structural items.
pub fn is_inter_item_trivia(node: &RdNode) -> bool {
    matches!(node, RdNode::Text(text) if text.trim().is_empty())
        || matches!(node, RdNode::Comment(_))
}

impl fmt::Display for RdShapeErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedNode { expected, actual } => {
                write!(f, "expected {expected}, found {actual}")
            }
            Self::UnexpectedOption => f.write_str("unexpected option"),
            Self::MissingOption => f.write_str("missing option"),
            Self::WrongArity { expected, actual } => {
                write!(f, "expected {expected} children, found {actual}")
            }
            Self::Duplicate { construct, .. } => write!(f, "duplicate {construct}"),
            Self::UnexpectedContent { actual } => write!(f, "unexpected {actual} content"),
            Self::InvalidValue { construct, value } => {
                write!(f, "invalid {construct} value '{value}'")
            }
            Self::CountMismatch {
                construct,
                expected,
                actual,
            } => write!(f, "expected {expected} {construct} entries, found {actual}"),
            Self::MissingFollowing { construct } => write!(f, "missing following {construct}"),
            Self::DefinitionMismatch { construct } => {
                write!(f, "definition mismatch for {construct}")
            }
            Self::UnsupportedRepresentation { construct } => {
                write!(f, "unsupported representation for {construct}")
            }
        }
    }
}

impl fmt::Display for RdNodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} node",
            match self {
                Self::Text => "text",
                Self::RCode => "R code",
                Self::Verb => "verbatim",
                Self::Comment => "comment",
                Self::Tagged => "tagged",
                Self::Group => "group",
                Self::Raw => "raw",
            }
        )
    }
}
impl fmt::Display for RdExpectedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Tagged => "tagged node",
            Self::Link => r"\link node",
            Self::Href => r"\href node",
            Self::List => "list node",
            Self::Tabular => r"\tabular node",
            Self::Equation => "equation node",
            Self::Sexpr => r"\Sexpr node",
            Self::RdOpts => r"\RdOpts node",
            Self::Group => "group",
            Self::Item => "item",
            Self::TextLike => "text-like node",
        })
    }
}
impl fmt::Display for RdArity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exactly(n) => write!(f, "exactly {n}"),
            Self::Range { min, max } => write!(f, "{min}..={max}"),
            Self::AtLeast(n) => write!(f, "at least {n}"),
        }
    }
}
impl fmt::Display for RdConstruct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tag(tag) => write!(f, "{}", tag.as_rd_tag()),
            Self::OptionKey(key) => write!(f, "option {key}"),
            Self::ColumnSpec => f.write_str("column specification"),
            Self::TableRow => f.write_str("table row"),
            Self::SystemMacro(name) => write!(f, "system macro {name}"),
            Self::SystemMacroExpansion(name) => write!(f, "system macro {name} expansion"),
        }
    }
}

impl UnexpectedRawNode {
    /// Converts corpus classification details into the shared shape vocabulary.
    pub fn shape_error(&self, path: crate::RdPath) -> RdShapeError {
        RdShapeError::new(
            path,
            self.tag().map(RdTag::from_rd_tag),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw,
            },
        )
    }
}

use crate::UnexpectedRawNode;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displays_shape_error() {
        let error = RdShapeError::new(
            crate::RdPath::new(vec![crate::RdPathSegment::TopLevel(2)]),
            Some(RdTag::Title),
            RdShapeErrorKind::UnexpectedOption,
        );
        assert_eq!(
            error.to_string(),
            r"unexpected option for \title at top-level[2]"
        );
    }

    #[test]
    fn displays_value_and_count_shape_errors() {
        let path = crate::RdPath::new(vec![crate::RdPathSegment::TopLevel(0)]);
        let invalid = RdShapeError::new(
            path.clone(),
            None,
            RdShapeErrorKind::InvalidValue {
                construct: RdConstruct::ColumnSpec,
                value: "bad".into(),
            },
        );
        assert_eq!(
            invalid.to_string(),
            "invalid column specification value 'bad' at top-level[0]"
        );
        let count = RdShapeError::new(
            path,
            None,
            RdShapeErrorKind::CountMismatch {
                construct: RdConstruct::TableRow,
                expected: 2,
                actual: 3,
            },
        );
        assert_eq!(
            count.to_string(),
            "expected 2 table row entries, found 3 at top-level[0]"
        );
    }

    #[cfg(feature = "rds")]
    #[test]
    fn unexpected_raw_classifier_bridges_to_shared_shape_error() {
        let raw = crate::producer::raw_node(Some("OTHER".into()), None, vec![], None, vec![]);
        let crate::RawNodeClassification::Unexpected(unexpected) = crate::classify_raw_node(&raw)
        else {
            panic!("expected an unexpected raw classification");
        };
        let path = crate::RdPath::new(vec![crate::RdPathSegment::TopLevel(4)]);
        let error = unexpected.shape_error(path.clone());
        assert_eq!(error.path(), &path);
        assert_eq!(error.tag(), Some(&crate::RdTag::Unknown("OTHER".into())));
        assert!(matches!(
            error.kind(),
            RdShapeErrorKind::UnexpectedNode {
                expected: RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw
            }
        ));
    }
}
