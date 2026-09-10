//! Bounded, crate-private inspection of serialized object prefixes.
//!
//! This module intentionally does not construct [`RObject`](crate::RObject)
//! values. It shares the wire state used by the strict decoder and stops at a
//! closure body tag, which makes it suitable for answering metadata queries
//! about large or partially damaged objects.

#![cfg_attr(not(feature = "lazyload"), allow(dead_code))]

use std::fmt;

use crate::wire::{self, CLOSXP, RefEntry};
#[cfg(test)]
use crate::wire::{ALTREP_SXP, BCODESXP, EXTPTRSXP};
use crate::{ByteCursor, Error, Header, Limits, NativeEncodingPolicy, SexpKind};

/// The type observed in the serialized root (including unknown type codes).
pub(crate) type StoredKind = SexpKind;

#[derive(Debug, Clone, Copy)]
pub(crate) struct InspectionOptions {
    limits: Limits,
    max_formals: usize,
    max_bytes_visited: usize,
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
    pub(crate) fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub(crate) fn max_formals(mut self, value: usize) -> Self {
        self.max_formals = value;
        self
    }

    pub(crate) fn max_bytes_visited(mut self, value: usize) -> Self {
        self.max_bytes_visited = value;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InspectionExtent {
    RootTagOnly,
    ThroughFormals {
        body_offset: usize,
        record_len: usize,
        body_kind: StoredKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DefaultPresence {
    Absent,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Formal {
    pub(crate) name: String,
    pub(crate) default: DefaultPresence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FormalsInspection {
    Available(Vec<Formal>),
    NotApplicable,
    Unavailable(PrefixFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrefixInspection {
    pub(crate) kind: StoredKind,
    pub(crate) extent: InspectionExtent,
    pub(crate) formals: FormalsInspection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailurePhase {
    Root,
    Attributes,
    Environment,
    Formals,
    Default(usize),
    BodyTag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureReason {
    Unsupported { type_code: u8, kind: StoredKind },
    Malformed,
    ResourceLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrefixFailure {
    pub(crate) phase: FailurePhase,
    pub(crate) offset: usize,
    pub(crate) reason: FailureReason,
}

impl fmt::Display for PrefixFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} failure at byte {}: {:?}",
            self.phase, self.offset, self.reason
        )
    }
}

pub(crate) fn inspect_stored_object(
    bytes: &[u8],
    options: InspectionOptions,
) -> Result<PrefixInspection, PrefixFailure> {
    let visible_len = bytes.len().min(options.max_bytes_visited);
    let mut cursor = ByteCursor::new(&bytes[..visible_len]);
    let header = Header::parse(&mut cursor).map_err(|error| {
        failure_from_error_bounded(error, FailurePhase::Root, 0, bytes.len() > visible_len)
    })?;
    let mut inspector = Inspector {
        cursor,
        state: wire::WireState::new(
            options.limits,
            header.native_encoding,
            NativeEncodingPolicy::RejectUnknown,
        ),
        options,
        record_len: bytes.len(),
        input_len: bytes.len(),
        byte_limit: visible_len,
    };
    inspector.inspect_root()
}

struct Inspector<'a> {
    cursor: ByteCursor<'a>,
    state: wire::WireState,
    options: InspectionOptions,
    record_len: usize,
    input_len: usize,
    byte_limit: usize,
}

impl<'a> Inspector<'a> {
    fn failure_from_error(
        &self,
        error: Error,
        phase: FailurePhase,
        fallback: usize,
    ) -> PrefixFailure {
        failure_from_error_bounded(error, phase, fallback, self.input_len > self.byte_limit)
    }

    fn inspect_root(&mut self) -> Result<PrefixInspection, PrefixFailure> {
        let root_offset = self.cursor.position();
        let root_flags = self
            .state
            .read_flags(&mut self.cursor)
            .map_err(|error| self.failure_from_error(error, FailurePhase::Root, root_offset))?;
        let kind = root_flags.kind();

        if root_flags.type_code() != CLOSXP {
            return Ok(PrefixInspection {
                kind,
                extent: InspectionExtent::RootTagOnly,
                formals: FormalsInspection::NotApplicable,
            });
        }

        if !root_flags.has_tag() {
            return Ok(self.unavailable(
                kind,
                FormalsInspection::Unavailable(PrefixFailure {
                    phase: FailurePhase::Environment,
                    offset: root_offset,
                    reason: FailureReason::Malformed,
                }),
            ));
        }

        if root_flags.has_attributes()
            && let Err(failure) = self.skip_attributes(FailurePhase::Attributes)
        {
            return Ok(self.unavailable(kind, FormalsInspection::Unavailable(failure)));
        }

        if let Err(failure) = self.skip_object(FailurePhase::Environment) {
            return Ok(self.unavailable(kind, FormalsInspection::Unavailable(failure)));
        }

        let formals = match self.read_formals() {
            Ok(formals) => formals,
            Err(failure) => {
                return Ok(self.unavailable(kind, FormalsInspection::Unavailable(failure)));
            }
        };

        let body_offset = self.cursor.position();
        let body_flags = match self.state.read_flags(&mut self.cursor) {
            Ok(flags) => flags,
            Err(error) => {
                let failure = self.failure_from_error(error, FailurePhase::BodyTag, body_offset);
                return Ok(self.unavailable(kind, FormalsInspection::Unavailable(failure)));
            }
        };

        Ok(PrefixInspection {
            kind,
            extent: InspectionExtent::ThroughFormals {
                body_offset,
                record_len: self.record_len,
                body_kind: body_flags.kind(),
            },
            formals: FormalsInspection::Available(formals),
        })
    }

    fn unavailable(&self, kind: StoredKind, formals: FormalsInspection) -> PrefixInspection {
        PrefixInspection {
            kind,
            extent: InspectionExtent::RootTagOnly,
            formals,
        }
    }

    fn read_formals(&mut self) -> Result<Vec<Formal>, PrefixFailure> {
        let flags_offset = self.cursor.position();
        let mut flags = self
            .state
            .read_flags(&mut self.cursor)
            .map_err(|error| self.failure_from_error(error, FailurePhase::Formals, flags_offset))?;
        if wire::is_nil(flags) {
            return Ok(Vec::new());
        }

        let mut formals = Vec::new();
        let mut index = 0usize;
        loop {
            self.state.check_depth(1).map_err(|error| {
                self.failure_from_error(error, FailurePhase::Formals, self.cursor.position())
            })?;
            if formals.len() >= self.options.max_formals {
                return Err(PrefixFailure {
                    phase: FailurePhase::Formals,
                    offset: self.cursor.position().saturating_sub(4),
                    reason: FailureReason::ResourceLimit,
                });
            }
            if flags.type_code() != wire::LISTSXP {
                return Err(PrefixFailure {
                    phase: FailurePhase::Formals,
                    offset: self.cursor.position().saturating_sub(4),
                    reason: FailureReason::Malformed,
                });
            }
            self.state
                .account_elements(1, self.cursor.position())
                .map_err(|error| {
                    self.failure_from_error(error, FailurePhase::Formals, self.cursor.position())
                })?;

            if flags.has_attributes() {
                self.skip_attributes_at(FailurePhase::Formals, 2)?;
            }
            if !flags.has_tag() {
                return Err(PrefixFailure {
                    phase: FailurePhase::Formals,
                    offset: self.cursor.position().saturating_sub(4),
                    reason: FailureReason::Malformed,
                });
            }
            let tag_offset = self.cursor.position();
            self.state.check_depth(2).map_err(|error| {
                self.failure_from_error(error, FailurePhase::Formals, tag_offset)
            })?;
            let tag_flags = self.state.read_flags(&mut self.cursor).map_err(|error| {
                self.failure_from_error(error, FailurePhase::Formals, tag_offset)
            })?;
            let name_offset = self.cursor.position();
            let name_symbol = match tag_flags.type_code() {
                wire::SYMSXP => self
                    .state
                    .decode_symbol(&mut self.cursor)
                    .map_err(|error| {
                        self.failure_from_error(error, FailurePhase::Formals, name_offset)
                    })?,
                wire::REFSXP => {
                    let reference = self
                        .state
                        .read_ref_index(&mut self.cursor, tag_flags)
                        .map_err(|error| {
                            self.failure_from_error(error, FailurePhase::Formals, name_offset)
                        })?;
                    match self
                        .state
                        .resolve(reference, name_offset)
                        .map_err(|error| {
                            self.failure_from_error(error, FailurePhase::Formals, name_offset)
                        })? {
                        RefEntry::Symbol(symbol) => symbol.clone(),
                        RefEntry::Persisted(_) | RefEntry::Env(_) => {
                            return Err(PrefixFailure {
                                phase: FailurePhase::Formals,
                                offset: name_offset,
                                reason: FailureReason::Malformed,
                            });
                        }
                    }
                }
                _ => {
                    return Err(PrefixFailure {
                        phase: FailurePhase::Formals,
                        offset: tag_offset,
                        reason: FailureReason::Malformed,
                    });
                }
            };
            let name = name_symbol.as_str().to_owned();

            let default_offset = self.cursor.position();
            let default_flags = self.state.read_flags(&mut self.cursor).map_err(|error| {
                self.failure_from_error(error, FailurePhase::Default(index), default_offset)
            })?;
            let default = if default_flags.type_code() == wire::MISSINGARG_SXP {
                DefaultPresence::Absent
            } else {
                self.skip_flags(default_flags, 2, FailurePhase::Default(index))?;
                DefaultPresence::Present
            };
            formals.push(Formal { name, default });
            index += 1;

            let cdr_offset = self.cursor.position();
            flags = self.state.read_flags(&mut self.cursor).map_err(|error| {
                self.failure_from_error(error, FailurePhase::Formals, cdr_offset)
            })?;
            if wire::is_nil(flags) {
                return Ok(formals);
            }
            if !wire::is_dotted_pair(flags) {
                return Err(PrefixFailure {
                    phase: FailurePhase::Formals,
                    offset: cdr_offset,
                    reason: FailureReason::Malformed,
                });
            }
        }
    }
}

fn failure_from_error_bounded(
    error: Error,
    phase: FailurePhase,
    fallback: usize,
    bounded: bool,
) -> PrefixFailure {
    let offset = error_offset(&error).unwrap_or(fallback);
    let reason = match error {
        Error::UnexpectedEof { .. } if bounded => FailureReason::ResourceLimit,
        Error::UnsupportedSexp {
            kind, type_code, ..
        } => FailureReason::Unsupported { type_code, kind },
        Error::ReferenceLimitExceeded { .. }
        | Error::DepthLimitExceeded { .. }
        | Error::VectorLengthLimitExceeded { .. }
        | Error::TotalElementsLimitExceeded { .. } => FailureReason::ResourceLimit,
        Error::LongVectorUnsupported { .. } | Error::PersistedLongVectorUnsupported { .. } => {
            FailureReason::ResourceLimit
        }
        _ => FailureReason::Malformed,
    };
    PrefixFailure {
        phase,
        offset,
        reason,
    }
}

fn error_offset(error: &Error) -> Option<usize> {
    match error {
        Error::UnexpectedEof { offset, .. }
        | Error::UnsupportedMarker { offset, .. }
        | Error::UnsupportedVersion { offset, .. }
        | Error::InvalidUtf8 { offset }
        | Error::InvalidSexpType { offset, .. }
        | Error::RefIndexOutOfRange { offset, .. }
        | Error::ReferenceLimitExceeded { offset, .. }
        | Error::UnsupportedSexp { offset, .. }
        | Error::VectorLengthLimitExceeded { offset, .. }
        | Error::TotalElementsLimitExceeded { offset, .. }
        | Error::PersistedLongVectorUnsupported { offset, .. }
        | Error::LongVectorUnsupported { offset, .. }
        | Error::NegativeLength { offset, .. }
        | Error::InvalidAttributeTag { offset } => Some(*offset),
        Error::DepthLimitExceeded { .. }
        | Error::InvalidStringEncoding
        | Error::InvalidSymbolName => None,
    }
}

mod walk;

#[cfg(test)]
mod tests;
