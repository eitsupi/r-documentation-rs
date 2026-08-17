use std::{borrow::Cow, sync::Arc};

use crate::Error;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RObject {
    value: RValue,
    attributes: Attributes,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RValue {
    Null,
    Logical(Vec<Option<bool>>),
    Integer(Vec<Option<i32>>),
    Real(Vec<Option<f64>>),
    Character(Vec<RStr>),
    List(Vec<RObject>),
    Symbol(Symbol),
    Persisted(Persisted),
    Environment(EnvHandle),
}

/// An opaque handle to an environment.
///
/// `rd-rds` never models environment frames or enclosures: it only consumes
/// the wire bytes correctly and keeps the reference table aligned. All
/// non-singleton environments collapse to [`EnvHandle::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnvHandle {
    /// `R_GlobalEnv`.
    Global,
    /// `R_BaseEnv` / `R_BaseNamespace`.
    Base,
    /// `R_EmptyEnv`.
    Empty,
    /// Any other environment/package/namespace. Its wire fields were
    /// decoded (to keep the stream and reference table in sync) and then
    /// discarded.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    name: Symbol,
    value: RObject,
}

impl Attribute {
    pub fn new(name: Symbol, value: RObject) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &Symbol {
        &self.name
    }

    pub fn value(&self) -> &RObject {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attributes(Vec<Attribute>);

impl Attributes {
    /// Constructs an attribute collection from decoded attributes.
    pub fn new(values: Vec<Attribute>) -> Self {
        Self(values)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Attribute> {
        self.0.iter()
    }

    pub fn get(&self, name: &str) -> Option<&RObject> {
        self.0
            .iter()
            .find(|attribute| attribute.name().as_str() == name)
            .map(|attribute| attribute.value())
    }
}

impl RObject {
    pub fn from_parts(value: RValue, attributes: Attributes) -> Self {
        Self { value, attributes }
    }

    pub fn value(&self) -> &RValue {
        &self.value
    }

    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    pub fn into_parts(self) -> (RValue, Attributes) {
        (self.value, self.attributes)
    }

    pub fn class(&self) -> Option<&[RStr]> {
        match self.attributes.get("class")?.value() {
            RValue::Character(values) => Some(values),
            _ => None,
        }
    }

    pub fn names(&self) -> Option<&[RStr]> {
        match self.attributes.get("names")?.value() {
            RValue::Character(values) => Some(values),
            _ => None,
        }
    }

    pub fn get_named(&self, name: &str) -> Option<&RObject> {
        let RValue::List(items) = self.value() else {
            return None;
        };
        let names = self.names()?;
        names
            .iter()
            .position(|value| value.as_str().and_then(Result::ok).as_deref() == Some(name))
            .and_then(|index| items.get(index))
    }
}

/// A symbol whose print name is decoded eagerly during parsing, so it always
/// holds valid text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol(Arc<str>);

impl Symbol {
    pub fn new(value: impl Into<Arc<str>>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Persisted(Arc<[RStr]>);

impl Persisted {
    pub(crate) fn new(values: Vec<RStr>) -> Self {
        Self(values.into())
    }

    #[cfg(test)]
    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub fn as_slice(&self) -> &[RStr] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RStr {
    Na,
    Value(RStrValue),
}

/// The source of native-encoding context used to interpret an R string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NativeEncodingSource {
    /// The stream header declared this native encoding name (format 3 only).
    Header(Arc<str>),
    /// The header carried no native encoding and the caller opted into
    /// [`crate::NativeEncodingPolicy::AssumeUtf8`].
    AssumedUtf8,
    /// The header carried no native encoding and the caller made no
    /// assumption.
    Unknown,
}

impl NativeEncodingSource {
    pub(crate) fn decodes_as_utf8(&self) -> bool {
        match self {
            Self::Header(name) => is_utf8_native_encoding(Some(name)),
            Self::AssumedUtf8 => true,
            Self::Unknown => false,
        }
    }
}

/// The opaque payload of an [`RStr::Value`] string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RStrValue {
    bytes: Arc<[u8]>,
    encoding: REncoding,
    native_encoding_source: NativeEncodingSource,
}

impl RStr {
    /// Constructs an R string value from its serialized bytes and encoding
    /// context.
    pub fn new(
        bytes: &[u8],
        encoding: REncoding,
        native_encoding_source: NativeEncodingSource,
    ) -> Self {
        Self::Value(RStrValue {
            bytes: Arc::from(bytes),
            encoding,
            native_encoding_source,
        })
    }

    pub fn as_str(&self) -> Option<Result<Cow<'_, str>, Error>> {
        match self {
            Self::Na => None,
            Self::Value(value) => Some(value.as_str()),
        }
    }

    /// Reports the CHARSXP encoding flag recorded in the serialized data.
    /// R's `Encoding()` normalizes ASCII strings to "unknown" after reading,
    /// so the two can differ while the decoded string contents agree.
    pub fn encoding(&self) -> Option<REncoding> {
        match self {
            Self::Na => None,
            Self::Value(value) => Some(value.encoding()),
        }
    }
}

impl RStrValue {
    fn as_str(&self) -> Result<Cow<'_, str>, Error> {
        match self.encoding {
            REncoding::Native => {
                if self.bytes.is_ascii() || self.native_encoding_source.decodes_as_utf8() {
                    std::str::from_utf8(&self.bytes)
                        .map(Cow::Borrowed)
                        .map_err(|_| Error::InvalidStringEncoding)
                } else {
                    Err(Error::InvalidStringEncoding)
                }
            }
            REncoding::Utf8 => std::str::from_utf8(&self.bytes)
                .map(Cow::Borrowed)
                .map_err(|_| Error::InvalidStringEncoding),
            REncoding::Latin1 => Ok(Cow::Owned(
                self.bytes.iter().map(|&byte| byte as char).collect(),
            )),
            REncoding::Bytes => Err(Error::InvalidStringEncoding),
        }
    }

    /// Returns the serialized string bytes without converting them.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns the CHARSXP encoding flag recorded in the serialized data.
    pub fn encoding(&self) -> REncoding {
        self.encoding
    }

    /// Returns the native-encoding provenance used for this value.
    pub fn native_encoding_source(&self) -> &NativeEncodingSource {
        &self.native_encoding_source
    }

    /// Returns the native encoding name declared by the stream header, if any.
    ///
    /// This reports header evidence only; it is not the effective decoding
    /// context and therefore returns `None` for [`NativeEncodingSource::AssumedUtf8`]
    /// and [`NativeEncodingSource::Unknown`].
    pub fn header_native_encoding(&self) -> Option<&str> {
        match &self.native_encoding_source {
            NativeEncodingSource::Header(name) => Some(name),
            NativeEncodingSource::AssumedUtf8 | NativeEncodingSource::Unknown => None,
        }
    }
}

fn is_utf8_native_encoding(encoding: Option<&str>) -> bool {
    encoding
        .map(|encoding| {
            encoding.eq_ignore_ascii_case("UTF-8") || encoding.eq_ignore_ascii_case("UTF8")
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum REncoding {
    Native,
    Utf8,
    Latin1,
    Bytes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_ascii_strings_decode_without_header_encoding() {
        let value = RStr::new(b"name", REncoding::Native, NativeEncodingSource::Unknown);
        assert_eq!(value.as_str().unwrap().unwrap().as_ref(), "name");
    }

    #[test]
    fn native_utf8_strings_decode_when_header_encoding_is_utf8() {
        let value = RStr::new(
            "cafe\u{301}".as_bytes(),
            REncoding::Native,
            NativeEncodingSource::Header(Arc::from("UTF-8")),
        );
        assert_eq!(value.as_str().unwrap().unwrap().as_ref(), "cafe\u{301}");
    }

    #[test]
    fn native_non_ascii_strings_reject_unknown_native_encoding() {
        let value = RStr::new(
            "é".as_bytes(),
            REncoding::Native,
            NativeEncodingSource::Unknown,
        );
        assert_eq!(
            value.as_str().unwrap().unwrap_err(),
            Error::InvalidStringEncoding
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    max_depth: u32,
    max_vector_len: usize,
    max_total_elements: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 5_000,
            max_vector_len: 8_000_000,
            max_total_elements: 16_000_000,
        }
    }
}

impl Limits {
    pub fn max_depth(mut self, value: u32) -> Self {
        self.max_depth = value;
        self
    }

    pub fn max_vector_len(mut self, value: usize) -> Self {
        self.max_vector_len = value;
        self
    }

    pub fn max_total_elements(mut self, value: usize) -> Self {
        self.max_total_elements = value;
        self
    }

    pub(crate) fn max_depth_value(self) -> u32 {
        self.max_depth
    }
    pub(crate) fn max_vector_len_value(self) -> usize {
        self.max_vector_len
    }
    pub(crate) fn max_total_elements_value(self) -> usize {
        self.max_total_elements
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SexpKind {
    Nil,
    Sym,
    PairList,
    Env,
    Char,
    Logical,
    Integer,
    Real,
    String,
    List,
    ExtPtr,
    WeakRef,
    Persist,
    Package,
    Namespace,
    Ref,
    Closure,
    BuiltIn,
    Special,
    Promise,
    Lang,
    DotDotDot,
    ByteCode,
    S4,
    Complex,
    Raw,
    Other(u8),
}

impl SexpKind {
    pub fn from_type_code(type_code: u8) -> Self {
        match type_code {
            0 | 254 => Self::Nil,
            1 => Self::Sym,
            2 => Self::PairList,
            3 => Self::Closure,
            4 => Self::Env,
            5 => Self::Promise,
            6 => Self::Lang,
            7 => Self::Special,
            8 => Self::BuiltIn,
            9 => Self::Char,
            10 => Self::Logical,
            13 => Self::Integer,
            14 => Self::Real,
            15 => Self::Complex,
            16 => Self::String,
            17 => Self::DotDotDot,
            19 => Self::List,
            21 => Self::ByteCode,
            22 => Self::ExtPtr,
            23 => Self::WeakRef,
            24 => Self::Raw,
            25 => Self::S4,
            247 => Self::Persist,
            248 => Self::Package,
            249 | 250 => Self::Namespace,
            255 => Self::Ref,
            other => Self::Other(other),
        }
    }
}
