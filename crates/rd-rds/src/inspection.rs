//! Bounded inspection of a decompressed XDR serialization stream.
//!
//! [`inspect`] observes the serialized root kind and, for closures, formal
//! names and default presence. It does not construct an [`RObject`](crate::RObject)
//! or validate the complete object. Non-closures stop after the root tag;
//! closures stop after the body tag, leaving the body payload unchecked.
//!
//! Callers own file access and decompression. A lazy-load consumer can inspect
//! `RecordBytes::decompressed_bytes()` and then pass the same bytes to another
//! decoder, without reading or decompressing the record again. This module
//! requires no optional features. The package-level inspection types remain
//! available at their existing `package` paths with the `lazyload` feature.

use crate::{Limits, SexpKind, inspect as prefix};

/// Limits for inspecting one decompressed stream.
///
/// Defaults are [`Limits::default()`], one million formals, and 256 MiB of
/// visited bytes. These limits apply to the prefix walker, not to file reads,
/// decompression, or unvisited payload bytes.
#[derive(Debug, Clone, Copy)]
pub struct InspectionOptions {
    pub(crate) limits: Limits,
    pub(crate) max_formals: usize,
    pub(crate) max_bytes_visited: usize,
}

impl Default for InspectionOptions {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            max_formals: 1_000_000,
            max_bytes_visited: 256 * 1024 * 1024,
        }
    }
}

impl InspectionOptions {
    /// Sets depth, element, vector, and reference limits shared with decoding.
    #[must_use]
    pub fn limits(mut self, value: Limits) -> Self {
        self.limits = value;
        self
    }

    /// Sets the maximum number of formals collected from one closure.
    #[must_use]
    pub fn max_formals(mut self, value: usize) -> Self {
        self.max_formals = value;
        self
    }

    /// Bounds the prefix of the stream visible to inspection.
    ///
    /// A longer input is allowed when inspection stops within this bound.
    /// Reaching the bound while reading required prefix bytes reports a
    /// resource-limit failure. Closure body payloads are never inspected.
    #[must_use]
    pub fn max_bytes_visited(mut self, value: usize) -> Self {
        self.max_bytes_visited = value;
        self
    }
}

/// Inspects a decompressed XDR stream using default limits.
///
/// See [`inspect_with_options`] for the inspection and failure boundaries.
///
/// ```no_run
/// use rd_rds::inspection::{self, FormalsInspection};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let bytes = std::fs::read("closure.xdr")?;
/// let observed = inspection::inspect(&bytes)?;
/// if let FormalsInspection::Available(formals) = observed.formals() {
///     for formal in formals.iter() {
///         println!("{}: {:?}", formal.name(), formal.default());
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub fn inspect(bytes: &[u8]) -> Result<StoredObjectInspection, PrefixFailure> {
    inspect_with_options(bytes, InspectionOptions::default())
}

/// Inspects a decompressed XDR stream using explicit limits.
///
/// The input must start with the serialization header, not a record's
/// compression framing or a slice of a default expression. References and
/// encoding state are interpreted from the start of this stream.
///
/// An error before observing the root tag returns [`Err`]. For a closure,
/// subsequent failures retain the root kind and appear as
/// [`FormalsInspection::Unavailable`], with phase, byte offset, and cause.
/// Available formals preserve serialized order and distinguish absent defaults
/// from present defaults, including `NULL`; default expressions are not exposed.
/// No R code, promises, or persistent references are evaluated.
///
/// Success does not establish validity or runtime callability of the object.
/// Non-closures report [`InspectionExtent::RootTagOnly`], even for unknown root
/// kinds or missing payloads. Closures stop after the body tag and report
/// [`BodyValidation::NotValidated`]. Body payloads and trailing bytes are not
/// checked. File, decompression, and container validation belong to the caller.
pub fn inspect_with_options(
    bytes: &[u8],
    options: InspectionOptions,
) -> Result<StoredObjectInspection, PrefixFailure> {
    prefix::inspect_stored_object(bytes, options)
        .map(map_inspection)
        .map_err(map_prefix_failure)
}

/// Public name for the observed serialized S-expression kind.
pub type StoredKind = SexpKind;

/// The extent reached by a bounded record inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InspectionExtent {
    RootTagOnly,
    ThroughFormals {
        body_offset: usize,
        record_len: usize,
        body_kind: StoredKind,
        body_validation: BodyValidation,
    },
}

impl InspectionExtent {
    /// Returns the body tag offset when the closure prefix reached it.
    #[must_use]
    pub fn body_offset(&self) -> Option<usize> {
        match self {
            Self::ThroughFormals { body_offset, .. } => Some(*body_offset),
            Self::RootTagOnly => None,
        }
    }

    /// Returns the observed body kind when the closure prefix reached it.
    #[must_use]
    pub fn body_kind(&self) -> Option<StoredKind> {
        match self {
            Self::ThroughFormals { body_kind, .. } => Some(*body_kind),
            Self::RootTagOnly => None,
        }
    }

    /// Returns the body validation status when the body tag was observed.
    #[must_use]
    pub fn body_validation(&self) -> Option<BodyValidation> {
        match self {
            Self::ThroughFormals {
                body_validation, ..
            } => Some(*body_validation),
            Self::RootTagOnly => None,
        }
    }

    /// Returns the supplied stream length when the body tag was reached.
    ///
    /// This length includes unvisited bytes and does not establish that the
    /// stream contains a complete or valid object.
    #[must_use]
    pub fn record_len(&self) -> Option<usize> {
        match self {
            Self::ThroughFormals { record_len, .. } => Some(*record_len),
            Self::RootTagOnly => None,
        }
    }
}

/// Presence of a closure formal's default expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DefaultPresence {
    Absent,
    Present,
}

/// One owned formal name and default-presence observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Formal {
    name: String,
    default: DefaultPresence,
}

impl Formal {
    /// Returns the formal name in serialized order.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns whether a default expression was serialized.
    #[must_use]
    pub fn default(&self) -> DefaultPresence {
        self.default
    }
}

/// Owned closure formals in serialized order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionFormals {
    values: Vec<Formal>,
}

impl FunctionFormals {
    /// Returns the owned formal slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Formal] {
        &self.values
    }

    /// Returns the number of serialized formals.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether no formals were serialized.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns one formal by serialized position.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Formal> {
        self.values.get(index)
    }

    /// Iterates over formals in serialized order.
    pub fn iter(&self) -> impl Iterator<Item = &Formal> {
        self.values.iter()
    }
}

impl AsRef<[Formal]> for FunctionFormals {
    fn as_ref(&self) -> &[Formal] {
        self.as_slice()
    }
}

impl std::ops::Index<usize> for FunctionFormals {
    type Output = Formal;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

/// Closure-formal availability after bounded prefix inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormalsInspection {
    Available(FunctionFormals),
    NotApplicable(FormalsNotApplicable),
    Unavailable(FormalsUnavailable),
}

/// Why a stored object has no closure formals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormalsNotApplicable {
    BuiltIn,
    Special,
    NonClosure,
}

/// Why closure formals are unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormalsUnavailable {
    PromiseNotEvaluated,
    PersistentReferenceUnresolved,
    Prefix(PrefixFailure),
}

/// A failure observed after (or while) reading a serialized prefix.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{phase:?} failure at byte {offset}: {cause:?}")]
pub struct PrefixFailure {
    phase: FailurePhase,
    offset: usize,
    cause: FailureCause,
}

impl PrefixFailure {
    /// Returns the logical phase in which inspection failed.
    #[must_use]
    pub fn phase(&self) -> FailurePhase {
        self.phase
    }

    /// Returns the serialized byte offset associated with the failure.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the failure cause.
    #[must_use]
    pub fn cause(&self) -> &FailureCause {
        &self.cause
    }
}

/// Logical inspection phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FailurePhase {
    Root,
    Attributes,
    Environment,
    Formals,
    Default(usize),
    BodyTag,
}

/// Cause of an inspection prefix failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FailureCause {
    Unsupported { type_code: u8, kind: StoredKind },
    Malformed,
    ResourceLimit,
}

/// Body bytes are intentionally not validated by prefix inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BodyValidation {
    NotValidated,
}

/// Owned metadata observed from one selected stored record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredObjectInspection {
    kind: StoredKind,
    extent: InspectionExtent,
    formals: FormalsInspection,
}

impl StoredObjectInspection {
    /// Returns the serialized root kind.
    #[must_use]
    pub fn kind(&self) -> StoredKind {
        self.kind
    }

    /// Returns how far inspection read the selected record.
    #[must_use]
    pub fn extent(&self) -> &InspectionExtent {
        &self.extent
    }

    /// Returns the closure formal observation.
    #[must_use]
    pub fn formals(&self) -> &FormalsInspection {
        &self.formals
    }
}

fn map_inspection(value: prefix::PrefixInspection) -> StoredObjectInspection {
    let formals = match value.formals {
        prefix::FormalsInspection::Available(values) => {
            FormalsInspection::Available(FunctionFormals {
                values: values
                    .into_iter()
                    .map(|formal| Formal {
                        name: formal.name,
                        default: match formal.default {
                            prefix::DefaultPresence::Absent => DefaultPresence::Absent,
                            prefix::DefaultPresence::Present => DefaultPresence::Present,
                        },
                    })
                    .collect(),
            })
        }
        prefix::FormalsInspection::NotApplicable => match value.kind {
            SexpKind::Promise => {
                FormalsInspection::Unavailable(FormalsUnavailable::PromiseNotEvaluated)
            }
            SexpKind::Persist => {
                FormalsInspection::Unavailable(FormalsUnavailable::PersistentReferenceUnresolved)
            }
            SexpKind::BuiltIn => FormalsInspection::NotApplicable(FormalsNotApplicable::BuiltIn),
            SexpKind::Special => FormalsInspection::NotApplicable(FormalsNotApplicable::Special),
            _ => FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure),
        },
        prefix::FormalsInspection::Unavailable(failure) => {
            FormalsInspection::Unavailable(FormalsUnavailable::Prefix(map_prefix_failure(failure)))
        }
    };
    StoredObjectInspection {
        kind: value.kind,
        extent: match value.extent {
            prefix::InspectionExtent::RootTagOnly => InspectionExtent::RootTagOnly,
            prefix::InspectionExtent::ThroughFormals {
                body_offset,
                record_len,
                body_kind,
            } => InspectionExtent::ThroughFormals {
                body_offset,
                record_len,
                body_kind,
                body_validation: BodyValidation::NotValidated,
            },
        },
        formals,
    }
}

fn map_prefix_failure(value: prefix::PrefixFailure) -> PrefixFailure {
    PrefixFailure {
        phase: match value.phase {
            prefix::FailurePhase::Root => FailurePhase::Root,
            prefix::FailurePhase::Attributes => FailurePhase::Attributes,
            prefix::FailurePhase::Environment => FailurePhase::Environment,
            prefix::FailurePhase::Formals => FailurePhase::Formals,
            prefix::FailurePhase::Default(index) => FailurePhase::Default(index),
            prefix::FailurePhase::BodyTag => FailurePhase::BodyTag,
        },
        offset: value.offset,
        cause: match value.reason {
            prefix::FailureReason::Unsupported { type_code, kind } => {
                FailureCause::Unsupported { type_code, kind }
            }
            prefix::FailureReason::Malformed => FailureCause::Malformed,
            prefix::FailureReason::ResourceLimit => FailureCause::ResourceLimit,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_kind(kind: SexpKind) -> StoredObjectInspection {
        map_inspection(prefix::PrefixInspection {
            kind,
            extent: prefix::InspectionExtent::RootTagOnly,
            formals: prefix::FormalsInspection::NotApplicable,
        })
    }

    #[test]
    fn maps_non_closure_formal_reasons_without_inventing_failures() {
        assert!(matches!(
            map_kind(SexpKind::BuiltIn).formals(),
            FormalsInspection::NotApplicable(FormalsNotApplicable::BuiltIn)
        ));
        assert!(matches!(
            map_kind(SexpKind::Special).formals(),
            FormalsInspection::NotApplicable(FormalsNotApplicable::Special)
        ));
        assert!(matches!(
            map_kind(SexpKind::Integer).formals(),
            FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure)
        ));
        assert!(matches!(
            map_kind(SexpKind::Promise).formals(),
            FormalsInspection::Unavailable(FormalsUnavailable::PromiseNotEvaluated)
        ));
        assert!(matches!(
            map_kind(SexpKind::Persist).formals(),
            FormalsInspection::Unavailable(FormalsUnavailable::PersistentReferenceUnresolved)
        ));
    }
}
