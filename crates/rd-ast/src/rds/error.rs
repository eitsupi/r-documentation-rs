use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LowerError {
    RootMustBeList {
        location: LowerLocation,
    },
    InvalidStringEncoding {
        location: LowerLocation,
        encoding: StringEncoding,
    },
    UnsupportedValue {
        location: LowerLocation,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerLocation {
    pub(super) path: RdPath,
    pub(super) tag: Option<String>,
    pub(super) attribute: Option<String>,
}

impl LowerLocation {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }
    pub fn attribute(&self) -> Option<&str> {
        self.attribute.as_deref()
    }
}

impl fmt::Display for LowerLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.segments().is_empty() {
            f.write_str("document root")?;
        } else {
            self.path.fmt(f)?;
        }
        if self.tag.is_some() || self.attribute.is_some() {
            f.write_str(" (")?;
            if let Some(tag) = &self.tag {
                f.write_str(tag)?;
            }
            if let Some(attribute) = &self.attribute {
                if self.tag.is_some() {
                    f.write_str(", ")?;
                }
                write!(f, "@{attribute}")?;
            }
            f.write_str(")")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StringEncoding {
    Native,
    Utf8,
    Latin1,
    Bytes,
}

impl fmt::Display for StringEncoding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Native => "Native",
            Self::Utf8 => "Utf8",
            Self::Latin1 => "Latin1",
            Self::Bytes => "Bytes",
        })
    }
}

impl LowerError {
    pub fn location(&self) -> &LowerLocation {
        match self {
            Self::RootMustBeList { location }
            | Self::InvalidStringEncoding { location, .. }
            | Self::UnsupportedValue { location } => location,
        }
    }
    pub fn encoding(&self) -> Option<StringEncoding> {
        match self {
            Self::InvalidStringEncoding { encoding, .. } => Some(*encoding),
            _ => None,
        }
    }
}

impl fmt::Display for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootMustBeList { location } => {
                write!(f, "root Rd object must be a list at {location}")
            }
            Self::InvalidStringEncoding { location, encoding } => {
                write!(f, "invalid {encoding} string encoding at {location}")
            }
            Self::UnsupportedValue { location } => {
                write!(f, "unsupported R value at {location}")
            }
        }
    }
}

impl std::error::Error for LowerError {}
