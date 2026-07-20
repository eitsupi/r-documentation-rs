use std::fmt;

/// A human-oriented structural path through a producer's Rd object tree.
///
/// The [`fmt::Display`] representation is intended for diagnostics, not as a
/// machine-readable protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdPath {
    segments: Vec<RdPathSegment>,
}

impl RdPath {
    pub fn new(segments: Vec<RdPathSegment>) -> Self {
        Self { segments }
    }

    pub fn segments(&self) -> &[RdPathSegment] {
        &self.segments
    }

    pub fn with_child(&self, index: usize) -> RdPath {
        let mut path = self.clone();
        path.segments.push(RdPathSegment::Child(index));
        path
    }

    pub fn with_option(&self) -> RdPath {
        let mut path = self.clone();
        path.segments.push(RdPathSegment::Option);
        path
    }

    pub(crate) fn with_character(&self, index: usize) -> RdPath {
        let mut path = self.clone();
        path.segments.push(RdPathSegment::CharacterElement(index));
        path
    }
}

impl From<Vec<RdPathSegment>> for RdPath {
    fn from(segments: Vec<RdPathSegment>) -> Self {
        Self::new(segments)
    }
}

impl fmt::Display for RdPath {
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdPathSegment {
    TopLevel(usize),
    Child(usize),
    Option,
    Attribute(String),
    AttributeValue,
    ListElement(usize),
    CharacterElement(usize),
}

impl fmt::Display for RdPathSegment {
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
        let path = RdPath::new(vec![
            RdPathSegment::TopLevel(8),
            RdPathSegment::Child(3),
            RdPathSegment::Option,
            RdPathSegment::Attribute("srcref".to_string()),
            RdPathSegment::AttributeValue,
            RdPathSegment::ListElement(2),
            RdPathSegment::CharacterElement(0),
        ]);
        assert_eq!(
            path.to_string(),
            "top-level[8] / child[3] / @option / @attr(srcref) / value / list[2] / character[0]"
        );
    }
}
