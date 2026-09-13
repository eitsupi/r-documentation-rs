use std::fmt;

/// A canonical, producer-independent structural path through an
/// [`crate::RdDocument`].
///
/// The empty path denotes the document root. The display representation is
/// intended for diagnostics, not as a machine-readable protocol.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RdAstPath {
    segments: Vec<RdAstPathSegment>,
}

impl RdAstPath {
    pub fn new(segments: Vec<RdAstPathSegment>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[RdAstPathSegment] {
        &self.segments
    }

    pub fn with_child(&self, index: usize) -> RdAstPath {
        let mut path = self.clone();
        path.segments.push(RdAstPathSegment::Child(index));
        path
    }

    pub fn with_option(&self) -> RdAstPath {
        let mut path = self.clone();
        path.segments.push(RdAstPathSegment::Option);
        path
    }
}

impl From<Vec<RdAstPathSegment>> for RdAstPath {
    fn from(segments: Vec<RdAstPathSegment>) -> Self {
        Self::new(segments)
    }
}

impl fmt::Display for RdAstPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index != 0 {
                f.write_str(" / ")?;
            }
            segment.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RdAstPathSegment {
    TopLevel(usize),
    Child(usize),
    Option,
}

impl fmt::Display for RdAstPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopLevel(index) => write!(f, "top-level[{index}]"),
            Self::Child(index) => write!(f, "child[{index}]"),
            Self::Option => f.write_str("@option"),
        }
    }
}

/// The detailed structural path used while lowering an RDS value.
///
/// This producer-specific path is available only with the `rds` feature. It
/// deliberately has no conversion to [`RdAstPath`], because its segments
/// describe producer storage rather than canonical AST locations.
#[cfg(feature = "rds")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum LowerPathSegment {
    TopLevel(usize),
    Child(usize),
    Option,
    Attribute(String),
    AttributeValue,
    ListElement(usize),
    CharacterElement(usize),
}

#[cfg(feature = "rds")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LowerPath {
    segments: Vec<LowerPathSegment>,
}

#[cfg(feature = "rds")]
impl LowerPath {
    pub fn new(segments: Vec<LowerPathSegment>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[LowerPathSegment] {
        &self.segments
    }
}

#[cfg(feature = "rds")]
impl fmt::Display for LowerPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index != 0 {
                f.write_str(" / ")?;
            }
            segment.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(feature = "rds")]
impl fmt::Display for LowerPathSegment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TopLevel(index) => write!(f, "top-level[{index}]"),
            Self::Child(index) => write!(f, "child[{index}]"),
            Self::Option => f.write_str("@option"),
            Self::Attribute(name) => write!(f, "@attr({name})"),
            Self::AttributeValue => f.write_str("value"),
            Self::ListElement(index) => write!(f, "list[{index}]"),
            Self::CharacterElement(index) => write!(f, "character[{index}]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_structural_paths() {
        let path = RdAstPath::new(vec![
            RdAstPathSegment::TopLevel(8),
            RdAstPathSegment::Child(3),
            RdAstPathSegment::Option,
        ]);
        assert_eq!(path.to_string(), "top-level[8] / child[3] / @option");
    }

    #[test]
    fn ast_paths_are_orderable_and_hashable() {
        use std::collections::{BTreeSet, HashSet};

        let path = RdAstPath::new(vec![RdAstPathSegment::TopLevel(0)]);
        let mut ordered = BTreeSet::new();
        ordered.insert(path.clone());
        let mut hashed = HashSet::new();
        hashed.insert(path.clone());
        assert!(ordered.contains(&path));
        assert!(hashed.contains(&path));
    }
}
