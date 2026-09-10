//! Typed access to an installed package's `Meta/package.rds` metadata.
//!
//! This module covers the `packageDescription2` shape written by R for an
//! installed package. It is intentionally a typed reader rather than a
//! general R object model: values are validated and copied into owned Rust
//! data during construction. The raw [`crate::RObject`] remains available for
//! metadata shapes not covered here.
//!
//! A missing package field is represented by an outer [`Option`], while an R
//! `NA` character value is represented by an inner [`Option`]. Thus
//! [`PackageMeta::description_field`] can distinguish an absent field from a
//! present field whose value is `NA`. [`Built`] is the deliberate exception:
//! its optional character accessors collapse both cases to `None`, because an
//! absent and an `NA` build field carry the same meaning to consumers.
//!
//! [`PackagesMatrix`] covers CRAN-like `PACKAGES.rds` character matrices. It
//! validates and owns the matrix data, absorbing R's column-major layout.
//! Row/column lookup uses an outer `Option` for a missing column or row and an
//! inner `Option` for an R `NA` cell.
//!
//! [`NamespaceMetadata`] provides a separate owned view of static declarations
//! from `Meta/nsInfo.rds`. It does not represent runtime namespace state or
//! stored lazy-load bindings.

use thiserror::Error;

use crate::{RObject, RStr, RValue};

mod namespace;

#[cfg(feature = "lazyload")]
mod installed_code;

#[cfg(feature = "lazyload")]
pub use installed_code::{
    BodyValidation, CodeDbGeneration, CodeDbProvenance, DefaultPresence, FailureCause,
    FailurePhase, Formal, FormalsInspection, FormalsNotApplicable, FormalsUnavailable,
    FunctionFormals, InspectionExtent, InstalledCodeDb, InstalledCodeError, InstalledCodeOptions,
    PrefixFailure, StoredBinding, StoredKind, StoredObjectInspection,
};

pub use namespace::{
    ImportedName, MetadataField, NamespaceExport, NamespaceImport, NamespaceMetadata, S3MethodName,
    S3Registration,
};

/// A construction error from the typed installed-package metadata view.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ViewError {
    #[error("missing value at {path}")]
    Missing { path: String, field: Option<String> },
    #[error("unexpected type at {path}: expected {expected}, got {actual}")]
    UnexpectedType {
        path: String,
        field: Option<String>,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("unexpected length at {path}: expected {expected}, got {actual}")]
    UnexpectedLength {
        path: String,
        field: Option<String>,
        expected: String,
        actual: usize,
    },
    #[error("duplicate name at {path}")]
    DuplicateName { path: String, field: Option<String> },
    #[error("invalid string encoding at {path}")]
    InvalidStringEncoding {
        path: String,
        field: Option<String>,
        row: Option<usize>,
        column: Option<String>,
    },
    #[error("invalid dimensions at {path}: {reason}")]
    InvalidDimensions {
        path: String,
        field: Option<String>,
        reason: String,
    },
    #[error("invalid package version at {path}: {reason}")]
    InvalidPackageVersion {
        path: String,
        field: Option<String>,
        reason: String,
    },
}

impl ViewError {
    /// Returns the logical location of the invalid value.
    pub fn path(&self) -> String {
        match self {
            Self::Missing { path, .. }
            | Self::UnexpectedType { path, .. }
            | Self::UnexpectedLength { path, .. }
            | Self::DuplicateName { path, .. }
            | Self::InvalidStringEncoding { path, .. }
            | Self::InvalidDimensions { path, .. }
            | Self::InvalidPackageVersion { path, .. } => path.clone(),
        }
    }

    /// Returns the metadata field associated with the error, when there is one.
    pub fn field(&self) -> Option<&str> {
        match self {
            Self::Missing { field, .. }
            | Self::UnexpectedType { field, .. }
            | Self::UnexpectedLength { field, .. }
            | Self::DuplicateName { field, .. }
            | Self::InvalidStringEncoding { field, .. }
            | Self::InvalidDimensions { field, .. }
            | Self::InvalidPackageVersion { field, .. } => field.as_deref(),
        }
    }

    /// Returns row context when the error was caused by a matrix cell.
    pub fn row(&self) -> Option<usize> {
        match self {
            Self::InvalidStringEncoding { row, .. } => *row,
            _ => None,
        }
    }

    /// Returns column-name context when the error was caused by a matrix cell.
    pub fn column(&self) -> Option<&str> {
        match self {
            Self::InvalidStringEncoding { column, .. } => column.as_deref(),
            _ => None,
        }
    }
}

mod meta;
mod packages;

pub use meta::{Built, PackageMeta, PackageVersion};
pub use packages::{PackagesColumn, PackagesMatrix, PackagesRow};

fn expect_list<'a>(
    object: &'a RObject,
    path: &str,
    field: Option<&str>,
) -> Result<&'a [RObject], ViewError> {
    match &object.value() {
        RValue::List(values) => Ok(values),
        value => Err(unexpected_type(path, field, "list", value.kind_name())),
    }
}

fn named_values<'a>(
    object: &'a RObject,
    path: &str,
    field: Option<&str>,
) -> Result<&'a [RStr], ViewError> {
    let Some(attribute) = object.attributes().get("names") else {
        return Err(missing(format!("{path}.names"), field.map(str::to_owned)));
    };
    match &attribute.value() {
        RValue::Character(values) => Ok(values),
        value => Err(unexpected_type(
            &format!("{path}.names"),
            field,
            "character vector",
            value.kind_name(),
        )),
    }
}

fn decode_optional(
    value: &RStr,
    path: &str,
    field: Option<&str>,
) -> Result<Option<String>, ViewError> {
    match value.as_str() {
        None => Ok(None),
        Some(Ok(value)) => Ok(Some(value.into_owned())),
        Some(Err(_)) => Err(ViewError::InvalidStringEncoding {
            path: path.to_owned(),
            field: field.map(str::to_owned),
            row: None,
            column: None,
        }),
    }
}

fn invalid_dimensions(path: &str, reason: &str) -> ViewError {
    ViewError::InvalidDimensions {
        path: path.to_owned(),
        field: None,
        reason: reason.to_owned(),
    }
}

fn decode_required(value: &RStr, path: &str, field: Option<&str>) -> Result<String, ViewError> {
    match value.as_str() {
        None => Err(unexpected_type(path, field, "non-NA string", "NA")),
        Some(Ok(value)) => Ok(value.into_owned()),
        Some(Err(_)) => Err(ViewError::InvalidStringEncoding {
            path: path.to_owned(),
            field: field.map(str::to_owned),
            row: None,
            column: None,
        }),
    }
}

fn missing(path: impl Into<String>, field: Option<String>) -> ViewError {
    ViewError::Missing {
        path: path.into(),
        field,
    }
}

fn unexpected_type(
    path: &str,
    field: Option<&str>,
    expected: &'static str,
    actual: &'static str,
) -> ViewError {
    ViewError::UnexpectedType {
        path: path.to_owned(),
        field: field.map(str::to_owned),
        expected,
        actual,
    }
}

fn unexpected_length(
    path: &str,
    field: Option<&str>,
    expected: String,
    actual: usize,
) -> ViewError {
    ViewError::UnexpectedLength {
        path: path.to_owned(),
        field: field.map(str::to_owned),
        expected,
        actual,
    }
}

fn duplicate(path: &str, field: Option<String>) -> ViewError {
    ViewError::DuplicateName {
        path: path.to_owned(),
        field,
    }
}

trait ValueKindName {
    fn kind_name(&self) -> &'static str;
}

impl ValueKindName for RValue {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Logical(_) => "logical vector",
            Self::Integer(_) => "integer vector",
            Self::Real(_) => "real vector",
            Self::Character(_) => "character vector",
            Self::List(_) => "list",
            Self::Symbol(_) => "symbol",
            Self::Persisted(_) => "persisted value",
            Self::Environment(_) => "environment",
        }
    }
}

#[cfg(test)]
mod tests;
