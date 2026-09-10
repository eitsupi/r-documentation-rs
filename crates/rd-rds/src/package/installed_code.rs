//! Owned access to an installed package's `R/<pkg>.rdx` and `R/<pkg>.rdb`.
//!
//! This is a package-level view over [`crate::lazyload`]. The caller supplies
//! the installed package directory; this module does not search library paths
//! or inspect runtime exports. The completeness domain of [`InstalledCodeDb`]
//! is therefore the `variables` map in the code database index.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use crate::{Limits, SexpKind, inspect};

use super::super::lazyload::{self, Compression, LazyLoadDb, Options as LazyLoadOptions};

/// Limits applied while opening and inspecting an installed code database.
#[derive(Debug, Clone, Copy)]
pub struct InstalledCodeOptions {
    lazyload: LazyLoadOptions,
    limits: Limits,
    max_formals: usize,
    max_bytes_visited: usize,
}

impl Default for InstalledCodeOptions {
    fn default() -> Self {
        Self {
            lazyload: LazyLoadOptions::default(),
            limits: Limits::default(),
            max_formals: 1_000_000,
            max_bytes_visited: 256 * 1024 * 1024,
        }
    }
}

impl InstalledCodeOptions {
    /// Sets the maximum number of bytes read from the `.rdx` file.
    #[must_use]
    pub fn max_index_bytes(mut self, value: usize) -> Self {
        self.lazyload = self.lazyload.max_index_bytes(value);
        self
    }

    /// Sets the maximum stored size of one `.rdb` record.
    #[must_use]
    pub fn max_stored_record_bytes(mut self, value: usize) -> Self {
        self.lazyload = self.lazyload.max_stored_record_bytes(value);
        self
    }

    /// Sets the maximum decompressed size of one `.rdb` record.
    #[must_use]
    pub fn max_decompressed_record_bytes(mut self, value: usize) -> Self {
        self.lazyload = self.lazyload.max_decompressed_record_bytes(value);
        self
    }

    /// Sets the limits shared by strict decoding and prefix inspection.
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

    /// Sets the semantic prefix-walker byte limit.
    ///
    /// The selected record is first fully read and passed through
    /// [`LazyLoadDb`]'s stored-size, decompressed-size, decompression, and
    /// container/framing checks. This limit then bounds the prefix walker;
    /// semantic payload bytes after the observed closure body tag are not
    /// read or validated by inspection.
    #[must_use]
    pub fn max_bytes_visited(mut self, value: usize) -> Self {
        self.max_bytes_visited = value;
        self
    }
}

/// Errors opening or inspecting an installed code database.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InstalledCodeError {
    /// The package has no `R/<pkg>.rdx` code database.
    #[error("installed package has no code database at {path}")]
    NoCodeDatabase { path: PathBuf },
    /// The supplied path does not identify an installed package directory.
    #[error("cannot determine package name from directory {path}")]
    InvalidPackageDirectory { path: PathBuf },
    /// Opening the index or data file failed.
    #[error("failed to open code database file at {path}: {source}")]
    Open {
        path: PathBuf,
        #[source]
        source: lazyload::Error,
    },
    /// Reading or parsing the `.rdx` code database index failed.
    #[error("failed to read code database index at {path}: {source}")]
    Index {
        path: PathBuf,
        #[source]
        source: lazyload::Error,
    },
    /// The index changed while it was opened.
    #[error("code database index changed while it was being read: {path}: {source}")]
    IndexChanged {
        path: PathBuf,
        #[source]
        source: lazyload::Error,
    },
    /// A previously opened handle observed replacement of its data file.
    #[error("code database changed while it was being read: {path}: {source}")]
    DatabaseChanged {
        path: PathBuf,
        #[source]
        source: lazyload::Error,
    },
    /// The requested binding name does not occur in the index.
    #[error("unknown stored binding {name:?}")]
    UnknownStoredBinding { name: String },
    /// The requested binding name occurs more than once in the index.
    #[error("stored binding {name:?} occurs {count} times")]
    AmbiguousStoredBinding { name: String, count: usize },
    /// Reading a selected record failed; other index entries remain usable.
    #[error("failed to read stored binding {name:?}: {source}")]
    Record {
        name: String,
        #[source]
        source: lazyload::Error,
    },
    /// Inspection failed before the serialized root could be observed.
    #[error("failed to inspect stored binding {name:?}: {failure}")]
    Inspection {
        name: String,
        #[source]
        failure: PrefixFailure,
    },
}

/// An opaque best-effort identity for one opened code database generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CodeDbGeneration {
    index: FileIdentity,
    data: FileIdentity,
}

/// Provenance for an opened installed code database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeDbProvenance {
    package_dir: PathBuf,
    package_name: String,
    index_path: PathBuf,
    data_path: PathBuf,
    compression: Compression,
    generation: CodeDbGeneration,
}

impl CodeDbProvenance {
    /// Returns the caller-supplied installed package directory.
    #[must_use]
    pub fn package_dir(&self) -> &Path {
        &self.package_dir
    }

    /// Returns the basename used to select the `R/<pkg>.rdx` and `.rdb` files.
    #[must_use]
    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    /// Returns the selected `.rdx` path.
    #[must_use]
    pub fn index_path(&self) -> &Path {
        &self.index_path
    }

    /// Returns the selected `.rdb` path.
    #[must_use]
    pub fn data_path(&self) -> &Path {
        &self.data_path
    }

    /// Returns the compression declared by the index.
    #[must_use]
    pub fn compression(&self) -> Compression {
        self.compression
    }

    /// Returns the opaque identity captured when the database was opened.
    #[must_use]
    pub fn generation(&self) -> CodeDbGeneration {
        self.generation
    }
}

/// Public name for the observed serialized S-expression kind.
pub type StoredKind = SexpKind;

/// One entry from the `.rdx` `variables` map, retained in index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBinding {
    name: String,
}

impl StoredBinding {
    /// Returns the stored binding name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

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

    /// Returns the complete record length when available.
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

/// An owned, package-level reader over the installed code database.
#[derive(Debug)]
pub struct InstalledCodeDb {
    package_dir: PathBuf,
    data_path: PathBuf,
    lazyload: LazyLoadDb,
    bindings: Vec<StoredBinding>,
    provenance: CodeDbProvenance,
    options: InstalledCodeOptions,
}

impl InstalledCodeDb {
    /// Opens `R/<basename>.rdx` and `R/<basename>.rdb` below `package_dir`.
    pub fn open(package_dir: impl AsRef<Path>) -> Result<Self, InstalledCodeError> {
        Self::open_with_options(package_dir, InstalledCodeOptions::default())
    }

    /// Opens an installed code database with explicit bounds.
    pub fn open_with_options(
        package_dir: impl AsRef<Path>,
        options: InstalledCodeOptions,
    ) -> Result<Self, InstalledCodeError> {
        let package_dir = package_dir.as_ref().to_path_buf();
        let package = package_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| InstalledCodeError::InvalidPackageDirectory {
                path: package_dir.clone(),
            })?;
        let code_dir = package_dir.join("R");
        let index_path = code_dir.join(format!("{package}.rdx"));
        let data_path = code_dir.join(format!("{package}.rdb"));
        match fs::metadata(&index_path) {
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                return Err(InstalledCodeError::NoCodeDatabase { path: index_path });
            }
            Err(source) => {
                return Err(InstalledCodeError::Open {
                    path: index_path.clone(),
                    source: lazyload::Error::Io {
                        path: index_path,
                        source,
                    },
                });
            }
        }
        let index_before = file_identity(&index_path)?;
        let data_before = file_identity(&data_path)?;
        let lazyload = LazyLoadDb::open_with_options(&index_path, &data_path, options.lazyload)
            .map_err(|error| map_open_error(error, &index_path))?;
        let generation = CodeDbGeneration {
            index: file_identity(&index_path)?,
            data: file_identity(&data_path)?,
        };
        if generation.index != index_before {
            return Err(InstalledCodeError::IndexChanged {
                path: index_path,
                source: lazyload::Error::IndexChanged,
            });
        }
        if generation.data != data_before {
            return Err(InstalledCodeError::DatabaseChanged {
                path: data_path,
                source: lazyload::Error::DataFileChanged,
            });
        }
        let compression = lazyload.compression();
        let bindings = lazyload
            .variables()
            .iter()
            .map(|binding| StoredBinding {
                name: binding.name().to_owned(),
            })
            .collect();
        let provenance = CodeDbProvenance {
            package_dir: package_dir.clone(),
            package_name: package.to_owned(),
            index_path: index_path.clone(),
            data_path: data_path.clone(),
            compression,
            generation,
        };
        Ok(Self {
            package_dir,
            data_path,
            lazyload,
            bindings,
            provenance,
            options,
        })
    }

    /// Returns the caller-supplied installed package directory.
    #[must_use]
    pub fn package_dir(&self) -> &Path {
        &self.package_dir
    }

    /// Returns the selected `.rdx` path.
    #[must_use]
    pub fn index_path(&self) -> &Path {
        &self.provenance.index_path
    }

    /// Returns the selected `.rdb` path.
    #[must_use]
    pub fn data_path(&self) -> &Path {
        &self.provenance.data_path
    }

    /// Returns provenance captured at open time.
    #[must_use]
    pub fn provenance(&self) -> &CodeDbProvenance {
        &self.provenance
    }

    /// Returns the index-declared record compression.
    #[must_use]
    pub fn compression(&self) -> Compression {
        self.lazyload.compression()
    }

    /// Returns every `.rdx` variable in index order, including duplicate names.
    #[must_use]
    pub fn stored_bindings(&self) -> &[StoredBinding] {
        &self.bindings
    }

    /// Inspects one uniquely named stored binding without materializing it.
    ///
    /// Unknown and duplicate names are returned as structured errors. A
    /// duplicate is never silently resolved with the low-level last-wins
    /// policy. Record loading and container validation happen before semantic
    /// prefix inspection, so compressed-stream, framing, and trailing-byte
    /// corruption is reported as an inspection error. For closures, the
    /// semantic body payload itself is intentionally not read or validated.
    pub fn inspect_stored_binding(
        &self,
        name: &str,
    ) -> Result<StoredObjectInspection, InstalledCodeError> {
        let mut matches = self
            .bindings
            .iter()
            .filter(|binding| binding.name() == name);
        let Some(_) = matches.next() else {
            return Err(InstalledCodeError::UnknownStoredBinding {
                name: name.to_owned(),
            });
        };
        let count = 1 + matches.count();
        if count != 1 {
            return Err(InstalledCodeError::AmbiguousStoredBinding {
                name: name.to_owned(),
                count,
            });
        }
        let record = self
            .lazyload
            .read(name)
            .map_err(|error| map_record_error(error, name, &self.data_path))?;
        let prefix = inspect::inspect_stored_object(
            record.decompressed_bytes(),
            inspect::InspectionOptions::default()
                .limits(self.options.limits)
                .max_formals(self.options.max_formals)
                .max_bytes_visited(self.options.max_bytes_visited),
        )
        .map_err(|failure| InstalledCodeError::Inspection {
            name: name.to_owned(),
            failure: map_prefix_failure(failure),
        })?;
        Ok(map_inspection(prefix))
    }
}

fn map_inspection(value: inspect::PrefixInspection) -> StoredObjectInspection {
    let formals = match value.formals {
        inspect::FormalsInspection::Available(values) => {
            FormalsInspection::Available(FunctionFormals {
                values: values
                    .into_iter()
                    .map(|formal| Formal {
                        name: formal.name,
                        default: match formal.default {
                            inspect::DefaultPresence::Absent => DefaultPresence::Absent,
                            inspect::DefaultPresence::Present => DefaultPresence::Present,
                        },
                    })
                    .collect(),
            })
        }
        inspect::FormalsInspection::NotApplicable => match value.kind {
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
        inspect::FormalsInspection::Unavailable(failure) => {
            FormalsInspection::Unavailable(FormalsUnavailable::Prefix(map_prefix_failure(failure)))
        }
    };
    StoredObjectInspection {
        kind: value.kind,
        extent: match value.extent {
            inspect::InspectionExtent::RootTagOnly => InspectionExtent::RootTagOnly,
            inspect::InspectionExtent::ThroughFormals {
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

fn map_prefix_failure(value: inspect::PrefixFailure) -> PrefixFailure {
    PrefixFailure {
        phase: match value.phase {
            inspect::FailurePhase::Root => FailurePhase::Root,
            inspect::FailurePhase::Attributes => FailurePhase::Attributes,
            inspect::FailurePhase::Environment => FailurePhase::Environment,
            inspect::FailurePhase::Formals => FailurePhase::Formals,
            inspect::FailurePhase::Default(index) => FailurePhase::Default(index),
            inspect::FailurePhase::BodyTag => FailurePhase::BodyTag,
        },
        offset: value.offset,
        cause: match value.reason {
            inspect::FailureReason::Unsupported { type_code, kind } => {
                FailureCause::Unsupported { type_code, kind }
            }
            inspect::FailureReason::Malformed => FailureCause::Malformed,
            inspect::FailureReason::ResourceLimit => FailureCause::ResourceLimit,
        },
    }
}

fn map_open_error(error: lazyload::Error, index_path: &Path) -> InstalledCodeError {
    match error {
        lazyload::Error::Io { path, source } => InstalledCodeError::Open {
            path: path.clone(),
            source: lazyload::Error::Io { path, source },
        },
        lazyload::Error::IndexChanged => InstalledCodeError::IndexChanged {
            path: index_path.to_path_buf(),
            source: lazyload::Error::IndexChanged,
        },
        source => InstalledCodeError::Index {
            path: index_path.to_path_buf(),
            source,
        },
    }
}

fn map_record_error(error: lazyload::Error, name: &str, data_path: &Path) -> InstalledCodeError {
    match error {
        lazyload::Error::DataFileChanged => InstalledCodeError::DatabaseChanged {
            path: data_path.to_path_buf(),
            source: lazyload::Error::DataFileChanged,
        },
        other => InstalledCodeError::Record {
            name: name.to_owned(),
            source: other,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FileIdentity {
    len: u64,
    modified_nanos: Option<u128>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

fn file_identity(path: &Path) -> Result<FileIdentity, InstalledCodeError> {
    let metadata = fs::metadata(path).map_err(|source| InstalledCodeError::Open {
        path: path.to_path_buf(),
        source: lazyload::Error::Io {
            path: path.to_path_buf(),
            source,
        },
    })?;
    let modified_nanos = metadata.modified().ok().and_then(|time| {
        time.duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_nanos())
    });
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(FileIdentity {
            len: metadata.len(),
            modified_nanos,
            dev: metadata.dev(),
            ino: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    Ok(FileIdentity {
        len: metadata.len(),
        modified_nanos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map_kind(kind: SexpKind) -> StoredObjectInspection {
        map_inspection(inspect::PrefixInspection {
            kind,
            extent: inspect::InspectionExtent::RootTagOnly,
            formals: inspect::FormalsInspection::NotApplicable,
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
