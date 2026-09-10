//! Bounded, crate-private inspection of serialized object prefixes.
//!
//! This module intentionally does not construct [`RObject`](crate::RObject)
//! values. It shares the wire state used by the strict decoder and stops at a
//! closure body tag, which makes it suitable for answering metadata queries
//! about large or partially damaged objects.

#![allow(dead_code)]

use std::fmt;

use crate::wire::{self, ALTREP_SXP, BCODESXP, CLOSXP, EXTPTRSXP, ItemFlags, RefEntry, WEAKREFSXP};
use crate::{ByteCursor, Error, Header, Limits, NativeEncodingPolicy, Persisted, SexpKind};

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

    fn skip_object(&mut self, phase: FailurePhase) -> Result<(), PrefixFailure> {
        self.skip_object_at(phase, 1)
    }

    fn skip_object_at(&mut self, phase: FailurePhase, depth: u32) -> Result<(), PrefixFailure> {
        let mut tasks = vec![Task::Object { depth }];
        while let Some(task) = tasks.pop() {
            self.step_task(task, phase, &mut tasks)?;
        }
        Ok(())
    }

    fn skip_attributes(&mut self, phase: FailurePhase) -> Result<(), PrefixFailure> {
        self.skip_attributes_at(phase, 1)
    }

    fn skip_attributes_at(&mut self, phase: FailurePhase, depth: u32) -> Result<(), PrefixFailure> {
        let mut tasks = vec![Task::Attributes { depth }];
        while let Some(task) = tasks.pop() {
            self.step_task(task, phase, &mut tasks)?;
        }
        Ok(())
    }

    fn skip_flags(
        &mut self,
        flags: ItemFlags,
        depth: u32,
        phase: FailurePhase,
    ) -> Result<(), PrefixFailure> {
        let mut tasks = vec![Task::ObjectFlags { flags, depth }];
        while let Some(task) = tasks.pop() {
            self.step_task(task, phase, &mut tasks)?;
        }
        Ok(())
    }

    fn step_task(
        &mut self,
        task: Task,
        phase: FailurePhase,
        tasks: &mut Vec<Task>,
    ) -> Result<(), PrefixFailure> {
        match task {
            Task::Attributes { depth } => {
                self.state.check_depth(depth).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                let offset = self.cursor.position();
                let flags = self
                    .state
                    .read_flags(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, offset))?;
                if wire::is_nil(flags) {
                    return Ok(());
                }
                if flags.type_code() != wire::LISTSXP {
                    return Err(PrefixFailure {
                        phase,
                        offset,
                        reason: FailureReason::Malformed,
                    });
                }
                tasks.push(Task::AttributeCell {
                    flags,
                    depth,
                    offset,
                });
            }
            Task::AttributeCell {
                flags,
                depth,
                offset,
            } => {
                self.state.check_depth(depth).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                if flags.type_code() != wire::LISTSXP {
                    return Err(PrefixFailure {
                        phase,
                        offset,
                        reason: FailureReason::Malformed,
                    });
                }
                self.state
                    .account_elements(1, self.cursor.position())
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                tasks.push(Task::AttributeCellAfterAttributes {
                    flags,
                    depth,
                    offset,
                });
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            Task::AttributeCellAfterAttributes {
                flags,
                depth,
                offset,
            } => {
                if !flags.has_tag() {
                    return Err(PrefixFailure {
                        phase,
                        offset,
                        reason: FailureReason::Malformed,
                    });
                }
                let tag_offset = self.cursor.position();
                self.state
                    .check_depth(depth + 1)
                    .map_err(|error| self.failure_from_error(error, phase, tag_offset))?;
                let tag_flags = self
                    .state
                    .read_flags(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, tag_offset))?;
                match tag_flags.type_code() {
                    wire::SYMSXP => {
                        self.state
                            .decode_symbol(&mut self.cursor)
                            .map_err(|error| self.failure_from_error(error, phase, tag_offset))?;
                    }
                    wire::REFSXP => {
                        let index = self
                            .state
                            .read_ref_index(&mut self.cursor, tag_flags)
                            .map_err(|error| self.failure_from_error(error, phase, tag_offset))?;
                        let reference = self
                            .state
                            .resolve(index, tag_offset)
                            .map_err(|error| self.failure_from_error(error, phase, tag_offset))?;
                        if !matches!(reference, RefEntry::Symbol(_)) {
                            return Err(PrefixFailure {
                                phase,
                                offset: tag_offset,
                                reason: FailureReason::Malformed,
                            });
                        }
                    }
                    _ => {
                        return Err(PrefixFailure {
                            phase,
                            offset: tag_offset,
                            reason: FailureReason::Malformed,
                        });
                    }
                }
                tasks.push(Task::AttributeAfterValue { depth });
                tasks.push(Task::Object { depth: depth + 1 });
            }
            Task::AttributeAfterValue { depth } => {
                let offset = self.cursor.position();
                let flags = self
                    .state
                    .read_flags(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, offset))?;
                if wire::is_nil(flags) {
                    return Ok(());
                }
                if flags.type_code() != wire::LISTSXP {
                    return Err(PrefixFailure {
                        phase,
                        offset,
                        reason: FailureReason::Malformed,
                    });
                }
                tasks.push(Task::AttributeCell {
                    flags,
                    depth: depth + 1,
                    offset,
                });
            }
            Task::Object { depth } => {
                self.state.check_depth(depth).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                let offset = self.cursor.position();
                let flags = self
                    .state
                    .read_flags(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, offset))?;
                tasks.push(Task::ObjectFlags { flags, depth });
            }
            Task::ObjectFlags { flags, depth } => {
                self.state.check_depth(depth).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                self.schedule_flags(flags, depth, phase, tasks)?
            }
            Task::PairCell { flags, depth } => {
                self.state.check_depth(depth).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                self.state
                    .account_elements(1, self.cursor.position())
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::PairAfterAttributes { flags, depth });
                    tasks.push(Task::Attributes { depth: depth + 1 });
                } else {
                    tasks.push(Task::PairAfterAttributes { flags, depth });
                }
            }
            Task::PairAfterAttributes { flags, depth } => {
                if flags.has_tag() {
                    tasks.push(Task::PairAfterTag { depth });
                    tasks.push(Task::Object { depth: depth + 1 });
                } else {
                    tasks.push(Task::PairAfterTag { depth });
                }
            }
            Task::PairAfterTag { depth } => {
                tasks.push(Task::PairAfterCar { depth });
                tasks.push(Task::Object { depth: depth + 1 });
            }
            Task::PairAfterCar { depth } => {
                let offset = self.cursor.position();
                let flags = self
                    .state
                    .read_flags(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, offset))?;
                if wire::is_dotted_pair(flags) {
                    tasks.push(Task::PairCell { flags, depth });
                } else if !wire::is_nil(flags) {
                    tasks.push(Task::ObjectFlags {
                        flags,
                        depth: depth + 1,
                    });
                }
            }
            Task::VectorObjects {
                mut remaining,
                depth,
            } => {
                if remaining != 0 {
                    remaining -= 1;
                    tasks.push(Task::VectorObjects { remaining, depth });
                    tasks.push(Task::Object { depth: depth + 1 });
                }
            }
            Task::StringItems {
                mut remaining,
                register,
            } => {
                if remaining != 0 {
                    remaining -= 1;
                    tasks.push(Task::StringItems {
                        remaining,
                        register,
                    });
                    let flags_offset = self.cursor.position();
                    let flags = self
                        .state
                        .read_flags(&mut self.cursor)
                        .map_err(|error| self.failure_from_error(error, phase, flags_offset))?;
                    if flags.type_code() != wire::CHARSXP {
                        return Err(PrefixFailure {
                            phase,
                            offset: flags_offset,
                            reason: FailureReason::Malformed,
                        });
                    }
                    self.state
                        .decode_char_with_flags(&mut self.cursor, flags)
                        .map_err(|error| self.failure_from_error(error, phase, flags_offset))?;
                } else if let Some(register) = register {
                    self.state
                        .register(register, self.cursor.position())
                        .map_err(|error| {
                            self.failure_from_error(error, phase, self.cursor.position())
                        })?;
                }
            }
        }
        Ok(())
    }

    fn schedule_flags(
        &mut self,
        flags: ItemFlags,
        depth: u32,
        phase: FailurePhase,
        tasks: &mut Vec<Task>,
    ) -> Result<(), PrefixFailure> {
        let type_code = flags.type_code();
        match type_code {
            wire::NILSXP
            | wire::NILVALUE_SXP
            | wire::UNBOUNDVALUE_SXP
            | wire::MISSINGARG_SXP
            | wire::BASEENV_SXP
            | wire::EMPTYENV_SXP
            | wire::GLOBALENV_SXP
            | wire::BASENAMESPACE_SXP => {}
            wire::REFSXP => {
                let index =
                    self.state
                        .read_ref_index(&mut self.cursor, flags)
                        .map_err(|error| {
                            self.failure_from_error(error, phase, self.cursor.position())
                        })?;
                self.state
                    .resolve(index, self.cursor.position().saturating_sub(4))
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
            }
            wire::SYMSXP => {
                self.state
                    .decode_symbol(&mut self.cursor)
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::CHARSXP => {
                self.state
                    .decode_char_with_flags(&mut self.cursor, flags)
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::ENVSXP => {
                let _locked = self.cursor.read_be_i32().map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                self.state
                    .register(
                        RefEntry::Env(crate::EnvHandle::Other),
                        self.cursor.position(),
                    )
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                tasks.push(Task::Object { depth: depth + 1 }); // attrib
                tasks.push(Task::Object { depth: depth + 1 }); // hashtab
                tasks.push(Task::Object { depth: depth + 1 }); // frame
                tasks.push(Task::Object { depth: depth + 1 }); // enclos
            }
            wire::CLOSXP => {
                if !flags.has_tag() {
                    return Err(PrefixFailure {
                        phase,
                        offset: self.cursor.position().saturating_sub(4),
                        reason: FailureReason::Malformed,
                    });
                }
                tasks.push(Task::Object { depth: depth + 1 });
                tasks.push(Task::Object { depth: depth + 1 });
                tasks.push(Task::Object { depth: depth + 1 });
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::LISTSXP | wire::LANGSXP | wire::PROMSXP | wire::DOTSXP => {
                tasks.push(Task::PairCell { flags, depth });
            }
            wire::STRSXP | wire::VECSXP | wire::EXPRSXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_vector_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
                if type_code == wire::STRSXP {
                    tasks.push(Task::StringItems {
                        remaining: len,
                        register: None,
                    });
                } else {
                    tasks.push(Task::VectorObjects {
                        remaining: len,
                        depth,
                    });
                }
            }
            wire::LGLSXP | wire::INTSXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_vector_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                self.cursor
                    .read_exact(len.saturating_mul(4))
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::REALSXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_vector_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                self.cursor
                    .read_exact(len.saturating_mul(8))
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::CPLXSXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_vector_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                self.cursor
                    .read_exact(len.saturating_mul(16))
                    .map_err(|error| {
                        self.failure_from_error(error, phase, self.cursor.position())
                    })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::RAWSXP | wire::SPECIALSXP | wire::BUILTINSXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_vector_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                self.cursor.read_exact(len).map_err(|error| {
                    self.failure_from_error(error, phase, self.cursor.position())
                })?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            wire::PERSISTSXP | wire::PACKAGESXP | wire::NAMESPACESXP => {
                let len_offset = self.cursor.position();
                let len = self
                    .state
                    .read_string_vec_len(&mut self.cursor)
                    .map_err(|error| self.failure_from_error(error, phase, len_offset))?;
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
                tasks.push(Task::StringItems {
                    remaining: len,
                    register: Some(if type_code == wire::PERSISTSXP {
                        RefEntry::Persisted(Persisted::new(Vec::new()))
                    } else {
                        RefEntry::Env(crate::EnvHandle::Other)
                    }),
                });
            }
            wire::S4SXP => {
                if flags.has_attributes() {
                    tasks.push(Task::Attributes { depth: depth + 1 });
                }
            }
            BCODESXP | EXTPTRSXP | WEAKREFSXP | ALTREP_SXP => {
                return Err(PrefixFailure {
                    phase,
                    offset: self.cursor.position().saturating_sub(4),
                    reason: FailureReason::Unsupported {
                        type_code,
                        kind: flags.kind(),
                    },
                });
            }
            _ => {
                return Err(PrefixFailure {
                    phase,
                    offset: self.cursor.position().saturating_sub(4),
                    reason: FailureReason::Unsupported {
                        type_code,
                        kind: flags.kind(),
                    },
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum Task {
    Attributes {
        depth: u32,
    },
    AttributeCell {
        flags: ItemFlags,
        depth: u32,
        offset: usize,
    },
    AttributeCellAfterAttributes {
        flags: ItemFlags,
        depth: u32,
        offset: usize,
    },
    AttributeAfterValue {
        depth: u32,
    },
    Object {
        depth: u32,
    },
    ObjectFlags {
        flags: ItemFlags,
        depth: u32,
    },
    PairCell {
        flags: ItemFlags,
        depth: u32,
    },
    PairAfterAttributes {
        flags: ItemFlags,
        depth: u32,
    },
    PairAfterTag {
        depth: u32,
    },
    PairAfterCar {
        depth: u32,
    },
    VectorObjects {
        remaining: usize,
        depth: u32,
    },
    StringItems {
        remaining: usize,
        register: Option<RefEntry>,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

    fn closure_bytes() -> Vec<u8> {
        let mut bytes = vec![b'X', b'\n'];
        bytes.extend_from_slice(&[0, 0, 0, 3, 0, 4, 6, 1, 0, 3, 5, 0, 0, 0, 0, 5]);
        bytes.extend_from_slice(b"UTF-8");
        bytes.extend_from_slice(&[0, 0, 4, CLOSXP, 0, 0, 0, wire::EMPTYENV_SXP]);
        bytes.extend_from_slice(&[0, 0, 0, wire::NILSXP]);
        bytes.extend_from_slice(&[0, 0, 0, wire::BCODESXP]);
        bytes
    }

    fn closure_with_attribute_chain(length: usize) -> Vec<u8> {
        let mut bytes = closure_bytes();
        bytes[25] |= 0x02;

        let mut attributes = vec![0, 0, 0, wire::NILSXP];
        for index in (0..length).rev() {
            let name = format!("attribute_{index}");
            let mut cell = vec![0, 0, 4, wire::LISTSXP];
            cell.extend_from_slice(&[0, 0, 0, wire::SYMSXP]);
            cell.extend_from_slice(&[0, 0, 0, wire::CHARSXP]);
            cell.extend_from_slice(&(name.len() as u32).to_be_bytes());
            cell.extend_from_slice(name.as_bytes());
            cell.extend_from_slice(&[0, 0, 0, wire::NILSXP]);
            cell.extend_from_slice(&attributes);
            attributes = cell;
        }

        bytes.splice(27..27, attributes);
        bytes
    }

    fn replace_dots_tag_with_reference(bytes: &mut Vec<u8>, reference: u32) {
        let marker = [
            0,
            0,
            0,
            wire::SYMSXP,
            0,
            4,
            0,
            wire::CHARSXP,
            0,
            0,
            0,
            3,
            b'.',
            b'.',
            b'.',
        ];
        let start = bytes
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("dots formal tag marker");
        bytes.splice(
            start..start + marker.len(),
            [
                0,
                0,
                0,
                wire::REFSXP,
                (reference >> 24) as u8,
                (reference >> 16) as u8,
                (reference >> 8) as u8,
                reference as u8,
            ],
        );
    }

    #[test]
    fn closure_prefix_stops_after_body_tag() {
        let result = inspect_stored_object(&closure_bytes(), InspectionOptions::default()).unwrap();
        assert_eq!(result.kind, SexpKind::Closure);
        assert!(
            matches!(result.formals, FormalsInspection::Available(ref values) if values.is_empty())
        );
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals {
                body_kind: SexpKind::ByteCode,
                ..
            }
        ));
    }

    #[test]
    fn header_limit_is_top_level_failure() {
        let error = inspect_stored_object(
            &closure_bytes(),
            InspectionOptions::default().max_bytes_visited(2),
        )
        .unwrap_err();
        assert_eq!(error.phase, FailurePhase::Root);
        assert_eq!(error.reason, FailureReason::ResourceLimit);
    }

    #[test]
    fn truncated_body_tag_is_retained_as_unavailable() {
        let mut bytes = closure_bytes();
        bytes.truncate(bytes.len() - 1);
        let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::BodyTag,
                reason: FailureReason::Malformed,
                ..
            })
        ));
    }

    #[test]
    fn body_payload_is_not_read_after_a_valid_body_tag() {
        let mut bytes = closure_bytes();
        bytes.extend_from_slice(&[0xff, 0xff, 0xff]);
        let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals { .. }
        ));
        assert!(crate::parse(&bytes).is_err());
    }

    #[test]
    fn prefix_failures_keep_their_phase_and_reason() {
        let mut bytes = closure_bytes();
        bytes[30] = BCODESXP;
        let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Environment,
                reason: FailureReason::Unsupported { .. },
                ..
            })
        ));

        let mut missing_tag = closure_bytes();
        missing_tag[25] = 0;
        let result = inspect_stored_object(&missing_tag, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Environment,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        let result = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds"),
            InspectionOptions::default().limits(Limits::default().max_references(0)),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::ResourceLimit,
                ..
            })
        ));
    }

    #[test]
    fn formal_limit_does_not_return_partial_formals() {
        let result = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds"),
            InspectionOptions::default().max_formals(2),
        )
        .unwrap();
        assert!(matches!(result.formals, FormalsInspection::Unavailable(_)));
    }

    #[test]
    fn depth_limit_tracks_nested_fields_not_formal_ordinals() {
        let result = inspect_stored_object(
            &closure_bytes(),
            InspectionOptions::default().limits(Limits::default().max_depth(0)),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Environment,
                reason: FailureReason::ResourceLimit,
                ..
            })
        ));

        let result = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds"),
            InspectionOptions::default().limits(Limits::default().max_depth(1)),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::ResourceLimit,
                ..
            })
        ));
    }

    #[test]
    fn formal_tags_accept_shared_symbols_and_reject_bad_references() {
        let mut shared = include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        replace_dots_tag_with_reference(&mut shared, 1);
        let result = inspect_stored_object(&shared, InspectionOptions::default()).unwrap();
        let FormalsInspection::Available(formals) = result.formals else {
            panic!("shared symbol formal should be available");
        };
        assert_eq!(formals[0].name, "alpha");
        assert_eq!(formals[1].name, "alpha");

        let mut invalid = include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        replace_dots_tag_with_reference(&mut invalid, 99);
        let result = inspect_stored_object(&invalid, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        for environment_code in [wire::ENVSXP, wire::PERSISTSXP] {
            let mut non_symbol =
                include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
            non_symbol[30] = environment_code;
            let payload = if environment_code == wire::ENVSXP {
                [0; 20].to_vec()
            } else {
                [0; 8].to_vec()
            };
            non_symbol.splice(31..31, payload);
            replace_dots_tag_with_reference(&mut non_symbol, 1);
            let result = inspect_stored_object(&non_symbol, InspectionOptions::default()).unwrap();
            assert!(matches!(
                result.formals,
                FormalsInspection::Unavailable(PrefixFailure {
                    phase: FailurePhase::Formals,
                    reason: FailureReason::Malformed,
                    ..
                })
            ));
        }
    }

    #[test]
    fn malformed_prefix_shapes_keep_specific_failure_phases() {
        let mut attributes = closure_bytes();
        attributes[25] = 6;
        attributes.splice(27..27, [0, 0, 0, EXTPTRSXP]);
        let result = inspect_stored_object(&attributes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Attributes,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        let mut missing_attribute_tag = closure_bytes();
        missing_attribute_tag[25] |= 0x02;
        missing_attribute_tag[27..31].copy_from_slice(&[0, 0, 0, wire::LISTSXP]);
        let result =
            inspect_stored_object(&missing_attribute_tag, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Attributes,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        let mut formal_attributes =
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        let cell = formal_attributes
            .windows(4)
            .position(|window| window == [0, 0, 4, wire::LISTSXP])
            .unwrap();
        formal_attributes[cell + 2] |= 0x02;
        formal_attributes[cell + 7] = EXTPTRSXP;
        let result =
            inspect_stored_object(&formal_attributes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        let mut non_symbol =
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        let marker = [
            0,
            0,
            0,
            wire::SYMSXP,
            0,
            4,
            0,
            wire::CHARSXP,
            0,
            0,
            0,
            3,
            b'.',
            b'.',
            b'.',
        ];
        let tag = non_symbol
            .windows(marker.len())
            .position(|window| window == marker)
            .unwrap();
        non_symbol[tag + 3] = wire::CHARSXP;
        let result = inspect_stored_object(&non_symbol, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        let mut invalid_cdr =
            include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        let pair = [0, 0, 4, wire::LISTSXP];
        let positions = invalid_cdr
            .windows(pair.len())
            .enumerate()
            .filter_map(|(index, window)| (window == pair).then_some(index))
            .collect::<Vec<_>>();
        invalid_cdr[positions[1] + 3] = 26;
        let result = inspect_stored_object(&invalid_cdr, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::Malformed,
                ..
            })
        ));

        for type_code in [BCODESXP, ALTREP_SXP] {
            let mut default =
                include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").to_vec();
            let missing = [0, 0, 0, wire::MISSINGARG_SXP];
            let offset = default
                .windows(missing.len())
                .position(|window| window == missing)
                .unwrap();
            default[offset + 3] = type_code;
            let result = inspect_stored_object(&default, InspectionOptions::default()).unwrap();
            assert!(matches!(
                result.formals,
                FormalsInspection::Unavailable(PrefixFailure {
                    phase: FailurePhase::Default(0),
                    reason: FailureReason::Unsupported { .. },
                    ..
                })
            ));
        }
    }

    #[test]
    fn attribute_tags_honor_the_nested_depth_limit() {
        let bytes = include_bytes!("../tests/fixtures/data/closure_attributes_s4_v3.rds");
        let name_offset = bytes
            .windows(b"inspection_s4".len())
            .position(|window| window == b"inspection_s4")
            .expect("S4 attribute name");
        let tag_offset = name_offset - 12;
        let result = inspect_stored_object(
            bytes,
            InspectionOptions::default().limits(Limits::default().max_depth(1)),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Attributes,
                offset,
                reason: FailureReason::ResourceLimit,
            }) if offset == tag_offset
        ));
    }

    #[test]
    fn sibling_attribute_chain_honors_the_nested_depth_limit() {
        let bytes = closure_with_attribute_chain(3);
        let result = inspect_stored_object(
            &bytes,
            InspectionOptions::default().limits(Limits::default().max_depth(2)),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Attributes,
                reason: FailureReason::ResourceLimit,
                ..
            })
        ));
    }

    #[test]
    fn generated_diagnostic_fixtures_retain_their_failure_phase() {
        let cases = [
            (
                include_bytes!("../tests/fixtures/data/closure_environment_compiled_v3.rds")
                    .as_slice(),
                FailurePhase::Environment,
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_default_compiled_v3.rds").as_slice(),
                FailurePhase::Default(0),
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_default_altrep_v3.rds").as_slice(),
                FailurePhase::Default(0),
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_attributes_compiled_v3.rds")
                    .as_slice(),
                FailurePhase::Attributes,
            ),
        ];
        for (bytes, phase) in cases {
            let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
            assert!(matches!(
                result.formals,
                FormalsInspection::Unavailable(PrefixFailure {
                    phase: actual,
                    reason: FailureReason::Unsupported { .. },
                    ..
                }) if actual == phase
            ));
        }

        let s4 = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_attributes_s4_v3.rds"),
            InspectionOptions::default(),
        )
        .unwrap();
        assert!(matches!(s4.formals, FormalsInspection::Available(_)));
    }

    #[test]
    fn generated_namespace_environment_preserves_the_prefix() {
        let result = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_namespace_v3.rds"),
            InspectionOptions::default(),
        )
        .unwrap();
        assert!(matches!(result.formals, FormalsInspection::Available(_)));
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals { .. }
        ));
    }

    #[test]
    fn generated_persisted_environment_preserves_the_prefix() {
        let result = inspect_stored_object(
            include_bytes!("../tests/fixtures/data/closure_environment_persisted_v3.rds"),
            InspectionOptions::default(),
        )
        .unwrap();
        assert!(matches!(result.formals, FormalsInspection::Available(_)));
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals { .. }
        ));
    }

    #[test]
    fn byte_cap_applies_only_when_a_new_read_crosses_it() {
        let mut bytes = closure_bytes();
        let baseline = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
        let InspectionExtent::ThroughFormals { body_offset, .. } = baseline.extent else {
            panic!("closure prefix should reach the body tag");
        };
        bytes.extend(std::iter::repeat_n(0xff, 4096));

        let result = inspect_stored_object(
            &bytes,
            InspectionOptions::default().max_bytes_visited(body_offset + 4),
        )
        .unwrap();
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals { .. }
        ));

        let result = inspect_stored_object(
            &bytes,
            InspectionOptions::default().max_bytes_visited(body_offset + 3),
        )
        .unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::BodyTag,
                reason: FailureReason::ResourceLimit,
                ..
            })
        ));
    }

    #[test]
    fn unknown_body_tag_is_observed_without_body_validation() {
        let mut bytes = closure_bytes();
        *bytes.last_mut().unwrap() = 200;
        let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals {
                body_kind: SexpKind::Other(200),
                ..
            }
        ));
        assert!(crate::parse(&bytes).is_err());
    }

    #[test]
    fn generated_closure_fixtures_preserve_formal_order_and_defaults() {
        for (bytes, names) in [
            (
                include_bytes!("../tests/fixtures/data/closure_formals_v2.rds").as_slice(),
                ["alpha", "...", "not syntactic", "非構文", "omega"],
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_formals_v3.rds").as_slice(),
                ["alpha", "...", "not syntactic", "非構文", "omega"],
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_formals_compiled_v2.rds").as_slice(),
                ["alpha", "...", "not syntactic", "非構文", "omega"],
            ),
            (
                include_bytes!("../tests/fixtures/data/closure_formals_compiled_v3.rds").as_slice(),
                ["alpha", "...", "not syntactic", "非構文", "omega"],
            ),
        ] {
            let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
            let FormalsInspection::Available(formals) = result.formals else {
                panic!("closure formals should be available");
            };
            assert_eq!(
                formals
                    .iter()
                    .map(|formal| formal.name.as_str())
                    .collect::<Vec<_>>(),
                names
            );
            assert!(matches!(formals[0].default, DefaultPresence::Absent));
            assert!(matches!(formals[1].default, DefaultPresence::Absent));
            assert!(matches!(formals[2].default, DefaultPresence::Present));
            assert!(matches!(formals[3].default, DefaultPresence::Present));
            assert!(matches!(formals[4].default, DefaultPresence::Present));
            assert!(matches!(
                result.extent,
                InspectionExtent::ThroughFormals { .. }
            ));
        }

        for bytes in [
            include_bytes!("../tests/fixtures/data/closure_formals_compiled_v2.rds").as_slice(),
            include_bytes!("../tests/fixtures/data/closure_formals_compiled_v3.rds").as_slice(),
        ] {
            let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
            assert!(matches!(
                result.extent,
                InspectionExtent::ThroughFormals {
                    body_kind: SexpKind::ByteCode,
                    ..
                }
            ));
        }
    }
}
