//! Lossless fallback types for Rd structures that don't fit the
//! [`RdNode::Tagged`](crate::RdNode::Tagged) or
//! [`RdNode::Group`](crate::RdNode::Group) shapes.
//!
//! Raw's normative boundary and consumer treatment are in the crate's
//! included `CONTRACT.md`.

/// A losslessly-preserved Rd node that doesn't fit
/// [`RdNode::Tagged`](crate::RdNode::Tagged) or
/// [`RdNode::Group`](crate::RdNode::Group): it carries attributes beyond
/// those the producer recognizes (for the RDS lowering, beyond
/// `Rd_tag`/`Rd_option` plus the deliberately-discarded `srcref` -- see that
/// module's validated attribute policy), or has another genuinely
/// non-canonical shape (an untagged non-list value, or an untagged node
/// carrying an `Rd_option`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawRdNode {
    /// The node's `Rd_tag` string, if any. `None` for an untagged
    /// non-canonical node; ordinary positional list groups are represented by
    /// [`crate::RdNode::Group`]. `Some(text)` preserves a
    /// recognized-shape-but-unexpected node's original tag text exactly
    /// (this is a different mechanism from
    /// [`RdTag::Unknown`](crate::RdTag::Unknown), which lives inside a
    /// well-formed [`RdNode::Tagged`](crate::RdNode::Tagged) node -- `Raw`
    /// is for nodes whose *shape*, not just tag spelling, was unexpected).
    tag: Option<String>,
    /// The bracket-option content (`Rd_option`), if present.
    option: Option<Vec<crate::RdNode>>,
    /// The node's children.
    children: Vec<crate::RdNode>,
    /// The original node value when it cannot be represented as Rd children.
    ///
    /// `None` means the value is represented by `children`; `Some` preserves
    /// an opaque value that has no child representation. A correct producer
    /// never encodes the same value in both non-empty `children` and `payload`.
    payload: Option<Box<RawRdValue>>,
    /// Any attributes beyond `Rd_tag`/`Rd_option`, preserved verbatim.
    attributes: Vec<RdAttribute>,
}

impl RawRdNode {
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }
    pub fn option(&self) -> Option<&[crate::RdNode]> {
        self.option.as_deref()
    }
    pub fn children(&self) -> &[crate::RdNode] {
        &self.children
    }
    pub fn payload(&self) -> Option<&RawRdValue> {
        self.payload.as_deref()
    }
    pub fn attributes(&self) -> &[RdAttribute] {
        &self.attributes
    }
    /// Consumes the node into `(tag, option, children, payload, attributes)`.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        Option<String>,
        Option<Vec<crate::RdNode>>,
        Vec<crate::RdNode>,
        Option<RawRdValue>,
        Vec<RdAttribute>,
    ) {
        (
            self.tag,
            self.option,
            self.children,
            self.payload.map(|payload| *payload),
            self.attributes,
        )
    }
}

/// A single R-level attribute attached to a raw Rd node, preserved for
/// losslessness.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdAttribute {
    /// The attribute's name.
    name: String,
    /// The attribute's value, including attributes attached to that value.
    value: RawRdObject,
}

impl RdAttribute {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &RawRdObject {
        &self.value
    }
    pub fn into_parts(self) -> (String, RawRdObject) {
        (self.name, self.value)
    }
}

/// A recursively preserved object used by raw attribute values and lists.
///
/// Keeping the object's attributes alongside its value is important for
/// structures such as `srcref`: the integer vector itself carries a nested
/// `srcfile` attribute. This producer-agnostic representation retains that
/// shape without exposing an RDS decoder type from `rd-ast`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawRdObject {
    /// The object's value.
    value: RawRdValue,
    /// Attributes attached to this value, recursively preserved.
    attributes: Vec<RdAttribute>,
}

impl RawRdObject {
    pub fn value(&self) -> &RawRdValue {
        &self.value
    }
    pub fn attributes(&self) -> &[RdAttribute] {
        &self.attributes
    }
    pub fn into_parts(self) -> (RawRdValue, Vec<RdAttribute>) {
        (self.value, self.attributes)
    }
}

/// Advanced, lossless construction surface for AST producers.
/// Normal consumers should inspect Raw values rather than synthesize them.
pub mod producer {
    use super::{RawRdNode, RawRdObject, RawRdValue, RdAttribute};
    use crate::RdNode;

    pub fn raw_node(
        tag: Option<String>,
        option: Option<Vec<RdNode>>,
        children: Vec<RdNode>,
        payload: Option<RawRdValue>,
        attributes: Vec<RdAttribute>,
    ) -> RawRdNode {
        RawRdNode {
            tag,
            option,
            children,
            payload: payload.map(Box::new),
            attributes,
        }
    }
    pub fn raw_attribute(name: String, value: RawRdObject) -> RdAttribute {
        RdAttribute { name, value }
    }
    pub fn raw_object(value: RawRdValue, attributes: Vec<RdAttribute>) -> RawRdObject {
        RawRdObject { value, attributes }
    }
}

/// The value of a recursively preserved raw object.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RawRdValue {
    /// R's `NULL` value.
    Null,
    /// An integer vector; `None` entries represent `NA`.
    Integer(Vec<Option<i32>>),
    /// A logical vector; `None` entries represent `NA`.
    Logical(Vec<Option<bool>>),
    /// A real vector with explicit representations for R's special values.
    Real(Vec<RawRdReal>),
    /// A character vector; `None` entries represent `NA`.
    Character(Vec<Option<String>>),
    /// A list whose elements retain their own attributes.
    List(Vec<RawRdObject>),
    /// A symbol name.
    Symbol(String),
    /// The string payload of a persisted reference.
    Persisted(Vec<Option<String>>),
    /// An opaque environment handle.
    Environment(RawRdEnvironment),
}

/// The environment categories exposed by a producer when preserving a raw
/// value. Environment frames are intentionally not part of the Rd AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RawRdEnvironment {
    Global,
    Base,
    Empty,
    Other,
}

/// A single value in an R real vector.
///
/// The special values use dedicated variants so serde formats such as JSON
/// never collapse `NA`, `NaN`, and positive or negative infinity into the
/// same `null` representation. This guarantee depends on
/// [`Finite`](RawRdReal::Finite) itself only ever holding an actual finite
/// payload -- see its documentation for the invariant.
///
/// # Why the invariant is documented rather than enforced
///
/// Nothing at the type level stops a caller from constructing
/// `Finite(f64::NAN)` or `Finite(f64::INFINITY)`. Leaving that unenforced
/// is a deliberate trade-off:
///
/// * Within this crate the only producer is the RDS lowering
///   (`lower_raw_real` in `rds.rs`), which classifies every incoming value
///   (`NA` / `NaN` / positive or negative infinity / finite) into the
///   matching variant *before* construction, so lowering can never produce
///   a non-finite `Finite`.
/// * Empirically, real (double) vectors do not occur in real-world R help
///   databases at all: Rd trees consist of character data plus
///   string/integer/logical attributes (`Rd_tag`, `Rd_option`, `srcref`,
///   `Rdfile`, `class`, `dynamicFlag`, `macro`). A full scan of an
///   installed corpus (46 packages, 3457 topics, covering both values and
///   attributes) found zero double vectors. This type exists for
///   completeness of the lossless raw mirror, not because such data is
///   expected in practice.
/// * For JSON specifically, the deserialize direction is already safe:
///   JSON numbers cannot express `NaN` or infinities, and
///   `{"kind":"finite","value":null}` fails to deserialize as `f64`. The
///   unenforced direction is serialization of a hand-constructed
///   non-finite `Finite` (silently emitted as `null`), and round-tripping
///   through binary serde formats that can represent non-finite floats.
///
/// Given the above, enforcement (a validated newtype payload plus a
/// custom `Deserialize`) was deliberately deferred; if these raw types
/// ever become a supported input surface for external producers, that is
/// the intended hardening path.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum RawRdReal {
    /// R's `NA_real_` value.
    Na,
    /// A finite IEEE-754 value.
    ///
    /// This variant must only hold a finite payload; `NA`, `NaN`, and the
    /// two infinities have their own dedicated variants ([`Na`](RawRdReal::Na),
    /// [`Nan`](RawRdReal::Nan), [`PositiveInfinity`](RawRdReal::PositiveInfinity),
    /// [`NegativeInfinity`](RawRdReal::NegativeInfinity)). Constructing
    /// `Finite` with a non-finite value violates this contract: nothing
    /// prevents it at the type level, but the derived serde `Serialize`
    /// impl would then encode it as JSON `null` (`serde_json`'s standard
    /// treatment of non-finite floats), silently defeating the point of
    /// keeping these variants separate. See the type-level documentation
    /// for why this invariant is documented rather than enforced.
    Finite(f64),
    /// A non-`NA` IEEE-754 not-a-number value.
    Nan,
    /// Positive infinity.
    PositiveInfinity,
    /// Negative infinity.
    NegativeInfinity,
}
