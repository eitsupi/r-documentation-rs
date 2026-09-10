use super::{FailurePhase, FailureReason, Inspector, PrefixFailure};
use crate::Persisted;
use crate::wire::{self, ALTREP_SXP, BCODESXP, EXTPTRSXP, ItemFlags, RefEntry, WEAKREFSXP};

impl<'a> Inspector<'a> {
    pub(super) fn skip_object(&mut self, phase: FailurePhase) -> Result<(), PrefixFailure> {
        self.skip_object_at(phase, 1)
    }

    fn skip_object_at(&mut self, phase: FailurePhase, depth: u32) -> Result<(), PrefixFailure> {
        let mut tasks = vec![Task::Object { depth }];
        while let Some(task) = tasks.pop() {
            self.step_task(task, phase, &mut tasks)?;
        }
        Ok(())
    }

    pub(super) fn skip_attributes(&mut self, phase: FailurePhase) -> Result<(), PrefixFailure> {
        self.skip_attributes_at(phase, 1)
    }

    pub(super) fn skip_attributes_at(
        &mut self,
        phase: FailurePhase,
        depth: u32,
    ) -> Result<(), PrefixFailure> {
        let mut tasks = vec![Task::Attributes { depth }];
        while let Some(task) = tasks.pop() {
            self.step_task(task, phase, &mut tasks)?;
        }
        Ok(())
    }

    pub(super) fn skip_flags(
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
