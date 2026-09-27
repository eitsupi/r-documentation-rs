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

use crate::Limits;
use crate::inspection::{self, InspectionOptions, PrefixFailure, StoredObjectInspection};

use super::super::lazyload::{self, Compression, LazyLoadDb, Options as LazyLoadOptions};

/// Limits applied while opening and inspecting an installed code database.
#[derive(Debug, Clone, Copy, Default)]
pub struct InstalledCodeOptions {
    lazyload: LazyLoadOptions,
    inspection: InspectionOptions,
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
        self.inspection = self.inspection.limits(value);
        self
    }

    /// Sets the maximum number of formals collected from one closure.
    #[must_use]
    pub fn max_formals(mut self, value: usize) -> Self {
        self.inspection = self.inspection.max_formals(value);
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
        self.inspection = self.inspection.max_bytes_visited(value);
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
        inspection::inspect_with_options(record.decompressed_bytes(), self.options.inspection)
            .map_err(|failure| InstalledCodeError::Inspection {
                name: name.to_owned(),
                failure,
            })
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
