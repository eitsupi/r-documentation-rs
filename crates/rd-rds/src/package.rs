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

use std::{collections::BTreeMap, fmt};

use thiserror::Error;

use crate::{RObject, RStr, RValue};

/// A construction error from the typed installed-package metadata view.
#[derive(Debug, Error, PartialEq, Eq)]
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

/// A validated, owned view of a CRAN-like `PACKAGES.rds` character matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagesMatrix {
    nrow: usize,
    column_names: Vec<String>,
    cells: Vec<Option<String>>,
}

impl PackagesMatrix {
    /// Validates and copies a `PACKAGES.rds` character matrix.
    pub fn from_object(object: &RObject) -> Result<Self, ViewError> {
        let values = match &object.value() {
            RValue::Character(values) => values,
            value => {
                return Err(unexpected_type(
                    "PACKAGES",
                    None,
                    "character vector",
                    value.kind_name(),
                ));
            }
        };
        let dimensions = object
            .attributes()
            .get("dim")
            .ok_or_else(|| missing("PACKAGES.attributes.dim", None))?;
        let dimensions = match &dimensions.value() {
            RValue::Integer(values) => values,
            value => {
                return Err(unexpected_type(
                    "PACKAGES.attributes.dim",
                    None,
                    "integer vector",
                    value.kind_name(),
                ));
            }
        };
        if dimensions.len() != 2 {
            return Err(unexpected_length(
                "PACKAGES.attributes.dim",
                None,
                "2".to_owned(),
                dimensions.len(),
            ));
        }
        let mut shape = [0usize; 2];
        for (index, value) in dimensions.iter().enumerate() {
            let Some(value) = value else {
                return Err(invalid_dimensions(
                    "PACKAGES.attributes.dim",
                    "dimensions must not contain NA",
                ));
            };
            if *value < 0 {
                return Err(invalid_dimensions(
                    "PACKAGES.attributes.dim",
                    "dimensions must not be negative",
                ));
            }
            shape[index] = *value as usize;
        }
        let element_count = shape[0].checked_mul(shape[1]).ok_or_else(|| {
            invalid_dimensions("PACKAGES.attributes.dim", "dimension product overflows")
        })?;
        if values.len() != element_count {
            return Err(unexpected_length(
                "PACKAGES",
                None,
                element_count.to_string(),
                values.len(),
            ));
        }

        let dimnames = object
            .attributes()
            .get("dimnames")
            .ok_or_else(|| missing("PACKAGES.attributes.dimnames", None))?;
        let dimnames = match &dimnames.value() {
            RValue::List(values) => values,
            value => {
                return Err(unexpected_type(
                    "PACKAGES.attributes.dimnames",
                    None,
                    "list",
                    value.kind_name(),
                ));
            }
        };
        if dimnames.len() != 2 {
            return Err(unexpected_length(
                "PACKAGES.attributes.dimnames",
                None,
                "2".to_owned(),
                dimnames.len(),
            ));
        }
        validate_row_names(&dimnames[0], shape[0])?;
        let column_values = match &dimnames[1].value() {
            RValue::Character(values) => values,
            value => {
                return Err(unexpected_type(
                    "PACKAGES.attributes.dimnames[1]",
                    None,
                    "character vector",
                    value.kind_name(),
                ));
            }
        };
        if column_values.len() != shape[1] {
            return Err(unexpected_length(
                "PACKAGES.attributes.dimnames[1]",
                None,
                shape[1].to_string(),
                column_values.len(),
            ));
        }
        let mut column_names = Vec::with_capacity(shape[1]);
        let mut seen_names = std::collections::BTreeSet::new();
        for (index, value) in column_values.iter().enumerate() {
            let name = decode_required(
                value,
                &format!("PACKAGES.attributes.dimnames[1][{index}]"),
                None,
            )?;
            if !seen_names.insert(name.clone()) {
                return Err(duplicate(
                    &format!("PACKAGES.attributes.dimnames[1][{index}]"),
                    Some(name),
                ));
            }
            column_names.push(name);
        }

        let mut cells = Vec::with_capacity(element_count);
        for row in 0..shape[0] {
            for column in 0..shape[1] {
                cells.push(decode_matrix_cell(
                    &values[row + column * shape[0]],
                    row,
                    &column_names[column],
                )?);
            }
        }
        Ok(Self {
            nrow: shape[0],
            column_names,
            cells,
        })
    }

    pub fn len(&self) -> usize {
        self.nrow
    }
    pub fn is_empty(&self) -> bool {
        self.nrow == 0
    }
    pub fn column_names(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.column_names.iter().map(String::as_str)
    }
    pub fn column(&self, name: &str) -> Option<PackagesColumn<'_>> {
        self.column_names
            .iter()
            .position(|column| column == name)
            .map(|index| PackagesColumn {
                matrix: self,
                index,
            })
    }
    pub fn row(&self, index: usize) -> Option<PackagesRow<'_>> {
        (index < self.nrow).then_some(PackagesRow {
            matrix: self,
            index,
        })
    }
    pub fn rows(&self) -> impl ExactSizeIterator<Item = PackagesRow<'_>> + '_ {
        (0..self.nrow).map(|index| PackagesRow {
            matrix: self,
            index,
        })
    }
}

impl TryFrom<&RObject> for PackagesMatrix {
    type Error = ViewError;
    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

/// A row in a validated [`PackagesMatrix`].
#[derive(Debug, Clone, Copy)]
pub struct PackagesRow<'a> {
    matrix: &'a PackagesMatrix,
    index: usize,
}

impl<'a> PackagesRow<'a> {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn get(&self, column: &str) -> Option<Option<&'a str>> {
        let column = self
            .matrix
            .column_names
            .iter()
            .position(|name| name == column)?;
        Some(self.matrix.cells[self.index * self.matrix.column_names.len() + column].as_deref())
    }
}

/// A column in a validated [`PackagesMatrix`].
#[derive(Debug, Clone, Copy)]
pub struct PackagesColumn<'a> {
    matrix: &'a PackagesMatrix,
    index: usize,
}

impl<'a> PackagesColumn<'a> {
    pub fn name(&self) -> &str {
        &self.matrix.column_names[self.index]
    }
    pub fn len(&self) -> usize {
        self.matrix.nrow
    }
    pub fn is_empty(&self) -> bool {
        self.matrix.is_empty()
    }
    pub fn get(&self, row: usize) -> Option<Option<&'a str>> {
        (row < self.matrix.nrow).then(|| {
            self.matrix.cells[row * self.matrix.column_names.len() + self.index].as_deref()
        })
    }
}

/// Typed, owned metadata from an installed package's `Meta/package.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMeta {
    description: BTreeMap<String, Option<String>>,
    built: Option<Built>,
}

impl PackageMeta {
    /// Validates and copies a `packageDescription2` R object.
    pub fn from_object(object: &RObject) -> Result<Self, ViewError> {
        let items = expect_list(object, "PackageMeta", None)?;
        require_class(object, "PackageMeta", None, "packageDescription2")?;
        let names = named_values(object, "PackageMeta", None)?;
        if names.len() != items.len() {
            return Err(unexpected_length(
                "PackageMeta",
                None,
                items.len().to_string(),
                names.len(),
            ));
        }

        let mut positions = BTreeMap::new();
        for (index, name) in names.iter().enumerate() {
            let name = decode_required(name, &format!("PackageMeta[{index}]"), None)?;
            if positions.insert(name.clone(), index).is_some() {
                return Err(duplicate(&format!("PackageMeta.{name}"), Some(name)));
            }
        }

        let description_index = positions
            .get("DESCRIPTION")
            .copied()
            .ok_or_else(|| missing("PackageMeta.DESCRIPTION", Some("DESCRIPTION".to_owned())))?;
        let description = parse_description(
            &items[description_index],
            "PackageMeta.DESCRIPTION",
            Some("DESCRIPTION"),
        )?;
        let built = positions
            .get("Built")
            .copied()
            .map(|index| parse_built(&items[index], "PackageMeta.Built"))
            .transpose()?;

        Ok(Self { description, built })
    }

    /// Returns the validated `Built` metadata, if the element is present.
    pub fn built(&self) -> Option<&Built> {
        self.built.as_ref()
    }

    /// Returns all DESCRIPTION fields in sorted key order.
    pub fn description(&self) -> &BTreeMap<String, Option<String>> {
        &self.description
    }

    /// Looks up a DESCRIPTION field, preserving the distinction between absent and R `NA`.
    pub fn description_field(&self, name: &str) -> Option<Option<&str>> {
        self.description.get(name).map(|value| value.as_deref())
    }
}

impl TryFrom<&RObject> for PackageMeta {
    type Error = ViewError;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

/// The validated `Built` element of package metadata.
///
/// Unlike [`PackageMeta::description_field`], whose nested `Option`
/// distinguishes an absent field from a present R `NA`, the optional
/// accessors here deliberately collapse both cases to `None`: for build
/// metadata, an absent `Platform` and an `NA` `Platform` carry the same
/// meaning to consumers ("no usable value"), so the distinction is not
/// preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    r_version: PackageVersion,
    platform: Option<String>,
    date: Option<String>,
    os_type: Option<String>,
}

impl Built {
    /// Returns the R version used to build the package.
    pub fn r_version(&self) -> &PackageVersion {
        &self.r_version
    }

    /// Returns the build platform, if it is present and not `NA`.
    pub fn platform(&self) -> Option<&str> {
        self.platform.as_deref()
    }

    /// Returns the build date, if it is present and not `NA`.
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }

    /// Returns the operating-system type, if it is present and not `NA`.
    pub fn os_type(&self) -> Option<&str> {
        self.os_type.as_deref()
    }
}

/// A validated R `package_version`/`numeric_version` value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    components: Vec<u32>,
}

impl PackageVersion {
    /// Returns the numeric version components.
    pub fn components(&self) -> &[u32] {
        &self.components
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut components = self.components.iter();
        if let Some(first) = components.next() {
            write!(formatter, "{first}")?;
            for component in components {
                write!(formatter, ".{component}")?;
            }
        }
        Ok(())
    }
}

fn parse_description(
    object: &RObject,
    path: &str,
    field: Option<&str>,
) -> Result<BTreeMap<String, Option<String>>, ViewError> {
    let values = match &object.value() {
        RValue::Character(values) => values,
        value => {
            return Err(unexpected_type(
                path,
                field,
                "character vector",
                value.kind_name(),
            ));
        }
    };
    let names = named_values(object, path, field)?;
    if names.len() != values.len() {
        return Err(unexpected_length(
            path,
            field,
            values.len().to_string(),
            names.len(),
        ));
    }
    let mut description = BTreeMap::new();
    for (index, (name, value)) in names.iter().zip(values).enumerate() {
        let name = decode_required(name, &format!("{path}[{index}]"), field)?;
        if description.contains_key(&name) {
            return Err(duplicate(&format!("{path}[\"{name}\"]"), Some(name)));
        }
        let value = decode_optional(value, &format!("{path}[\"{name}\"]"), Some(&name))?;
        description.insert(name, value);
    }
    Ok(description)
}

fn parse_built(object: &RObject, path: &str) -> Result<Built, ViewError> {
    let items = expect_list(object, path, Some("Built"))?;
    let names = named_values(object, path, Some("Built"))?;
    if names.len() != items.len() {
        return Err(unexpected_length(
            path,
            Some("Built"),
            items.len().to_string(),
            names.len(),
        ));
    }
    let mut positions = BTreeMap::new();
    for (index, name) in names.iter().enumerate() {
        let name = decode_required(name, &format!("{path}[{index}]"), Some("Built"))?;
        if positions.insert(name.clone(), index).is_some() {
            return Err(duplicate(&format!("{path}.{name}"), Some(name)));
        }
    }
    let r_index = positions
        .get("R")
        .copied()
        .ok_or_else(|| missing(format!("{path}.R"), Some("R".to_owned())))?;
    let r_version = parse_version(&items[r_index], &format!("{path}.R"))?;
    let platform = optional_built_string(&positions, items, "Platform", path)?;
    let date = optional_built_string(&positions, items, "Date", path)?;
    let os_type = optional_built_string(&positions, items, "OStype", path)?;
    Ok(Built {
        r_version,
        platform,
        date,
        os_type,
    })
}

fn parse_version(object: &RObject, path: &str) -> Result<PackageVersion, ViewError> {
    let valid_class = match object.attributes().get("class") {
        Some(attribute) => match &attribute.value() {
            RValue::Character(values) => {
                let mut has_package = false;
                let mut has_numeric = false;
                for value in values {
                    match value.as_str() {
                        Some(Ok(value)) if value == "package_version" => has_package = true,
                        Some(Ok(value)) if value == "numeric_version" => has_numeric = true,
                        Some(Err(_)) => {
                            return Err(ViewError::InvalidStringEncoding {
                                path: format!("{path}.class"),
                                field: Some("R".to_owned()),
                                row: None,
                                column: None,
                            });
                        }
                        _ => {}
                    }
                }
                has_package && has_numeric
            }
            _ => false,
        },
        None => false,
    };
    if !valid_class {
        return Err(invalid_version(
            path,
            "missing package_version/numeric_version class",
        ));
    }
    let RValue::List(values) = &object.value() else {
        return Err(invalid_version(path, "expected a length-one list"));
    };
    if values.len() != 1 {
        return Err(invalid_version(path, "expected a length-one list"));
    }
    let RValue::Integer(components) = &values[0].value() else {
        return Err(invalid_version(path, "expected an integer vector"));
    };
    if components.is_empty() {
        return Err(invalid_version(
            path,
            "version components must not be empty",
        ));
    }
    let mut owned = Vec::with_capacity(components.len());
    for (index, component) in components.iter().enumerate() {
        let Some(component) = component else {
            return Err(invalid_version(
                &format!("{path}[0][{index}]"),
                "component is NA",
            ));
        };
        if *component < 0 {
            return Err(invalid_version(
                &format!("{path}[0][{index}]"),
                "component is negative",
            ));
        }
        owned.push(*component as u32);
    }
    Ok(PackageVersion { components: owned })
}

fn optional_built_string(
    positions: &BTreeMap<String, usize>,
    items: &[RObject],
    name: &str,
    path: &str,
) -> Result<Option<String>, ViewError> {
    match positions.get(name) {
        Some(index) => {
            decode_character_scalar(&items[*index], &format!("{path}.{name}"), Some(name))
        }
        None => Ok(None),
    }
}

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

fn require_class(
    object: &RObject,
    path: &str,
    field: Option<&str>,
    expected: &str,
) -> Result<(), ViewError> {
    let Some(attribute) = object.attributes().get("class") else {
        return Err(missing(format!("{path}.class"), field.map(str::to_owned)));
    };
    let RValue::Character(values) = &attribute.value() else {
        return Err(unexpected_type(
            &format!("{path}.class"),
            field,
            "character vector",
            attribute.value().kind_name(),
        ));
    };
    for value in values {
        match value.as_str() {
            Some(Ok(value)) if value == expected => return Ok(()),
            Some(Err(_)) => {
                return Err(ViewError::InvalidStringEncoding {
                    path: format!("{path}.class"),
                    field: field.map(str::to_owned),
                    row: None,
                    column: None,
                });
            }
            _ => {}
        }
    }
    Err(unexpected_type(
        path,
        field,
        "expected class",
        "different class",
    ))
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

fn decode_character_scalar(
    object: &RObject,
    path: &str,
    field: Option<&str>,
) -> Result<Option<String>, ViewError> {
    let RValue::Character(values) = &object.value() else {
        return Err(unexpected_type(
            path,
            field,
            "character scalar",
            object.value().kind_name(),
        ));
    };
    if values.len() != 1 {
        return Err(unexpected_length(path, field, "1".to_owned(), values.len()));
    }
    decode_optional(&values[0], path, field)
}

fn validate_row_names(object: &RObject, expected: usize) -> Result<(), ViewError> {
    let values = match &object.value() {
        RValue::Null => return Ok(()),
        RValue::Character(values) => values,
        value => {
            return Err(unexpected_type(
                "PACKAGES.attributes.dimnames[0]",
                None,
                "character vector",
                value.kind_name(),
            ));
        }
    };
    if values.len() != expected {
        return Err(unexpected_length(
            "PACKAGES.attributes.dimnames[0]",
            None,
            expected.to_string(),
            values.len(),
        ));
    }
    for (index, value) in values.iter().enumerate() {
        if let Some(Err(_)) = value.as_str() {
            return Err(ViewError::InvalidStringEncoding {
                path: format!("PACKAGES.attributes.dimnames[0][{index}]"),
                field: None,
                row: None,
                column: None,
            });
        }
    }
    Ok(())
}

fn decode_matrix_cell(value: &RStr, row: usize, column: &str) -> Result<Option<String>, ViewError> {
    match value.as_str() {
        None => Ok(None),
        Some(Ok(value)) => Ok(Some(value.into_owned())),
        Some(Err(_)) => Err(ViewError::InvalidStringEncoding {
            path: format!("PACKAGES[row={row},column=\"{column}\"]"),
            field: Some(column.to_owned()),
            row: Some(row),
            column: Some(column.to_owned()),
        }),
    }
}

fn missing(path: impl Into<String>, field: Option<String>) -> ViewError {
    ViewError::Missing {
        path: path.into(),
        field,
    }
}

fn invalid_dimensions(path: &str, reason: &str) -> ViewError {
    ViewError::InvalidDimensions {
        path: path.to_owned(),
        field: None,
        reason: reason.to_owned(),
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

fn invalid_version(path: &str, reason: &str) -> ViewError {
    ViewError::InvalidPackageVersion {
        path: path.to_owned(),
        field: Some("R".to_owned()),
        reason: reason.to_owned(),
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
mod tests {
    use super::*;
    use crate::{Attribute, Attributes, REncoding, Symbol};

    #[test]
    fn package_version_displays_components() {
        let version = PackageVersion {
            components: vec![4, 6, 1],
        };
        assert_eq!(version.to_string(), "4.6.1");
        assert_eq!(version.components(), &[4, 6, 1]);
    }

    #[test]
    fn view_error_exposes_logical_context() {
        let error = ViewError::DuplicateName {
            path: "PackageMeta.DESCRIPTION[\"Package\"]".to_owned(),
            field: Some("Package".to_owned()),
        };
        assert_eq!(error.path(), "PackageMeta.DESCRIPTION[\"Package\"]");
        assert_eq!(error.field(), Some("Package"));
        assert_eq!(error.row(), None);
        assert_eq!(error.column(), None);
    }

    fn matrix(dim: Vec<Option<i32>>, dimnames: Vec<RObject>, values: Vec<RStr>) -> RObject {
        RObject::from_parts(
            RValue::Character(values),
            Attributes::new(vec![
                Attribute::new(
                    Symbol::new("dim"),
                    RObject::from_parts(RValue::Integer(dim), Attributes::default()),
                ),
                Attribute::new(
                    Symbol::new("dimnames"),
                    RObject::from_parts(RValue::List(dimnames), Attributes::default()),
                ),
            ]),
        )
    }

    fn names(values: &[&str]) -> RObject {
        RObject::from_parts(
            RValue::Character(
                values
                    .iter()
                    .map(|value| RStr::new(value.as_bytes(), REncoding::Native, None))
                    .collect(),
            ),
            Attributes::default(),
        )
    }

    fn replace_dimnames(object: &mut RObject, replacement: Vec<RObject>) {
        let dim = object.attributes().get("dim").unwrap().clone();
        set_attributes(
            object,
            Attributes::new(vec![
                Attribute::new(Symbol::new("dim"), dim),
                Attribute::new(
                    Symbol::new("dimnames"),
                    RObject::from_parts(RValue::List(replacement), Attributes::default()),
                ),
            ]),
        );
    }

    fn set_attributes(object: &mut RObject, attributes: Attributes) {
        let (value, _) = object.clone().into_parts();
        *object = RObject::from_parts(value, attributes);
    }

    fn set_value(object: &mut RObject, value: RValue) {
        let (_, attributes) = object.clone().into_parts();
        *object = RObject::from_parts(value, attributes);
    }

    #[test]
    fn packages_matrix_rejects_malformed_shape_and_names() {
        let valid = || {
            matrix(
                vec![Some(1), Some(1)],
                vec![names(&["row"]), names(&["Package"])],
                vec![RStr::new(b"x", REncoding::Native, None)],
            )
        };
        assert!(
            matches!(PackagesMatrix::from_object(&RObject::from_parts(RValue::Character(vec![]), Attributes::default())), Err(ViewError::Missing { path, .. }) if path == "PACKAGES.attributes.dim")
        );
        let mut object = valid();
        set_attributes(
            &mut object,
            Attributes::new(vec![Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(RValue::Character(vec![]), Attributes::default()),
            )]),
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedType { path, .. }) if path == "PACKAGES.attributes.dim")
        );
        let mut object = valid();
        set_attributes(
            &mut object,
            Attributes::new(vec![Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(RValue::Integer(vec![Some(1)]), Attributes::default()),
            )]),
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dim")
        );
        let mut object = valid();
        set_attributes(
            &mut object,
            Attributes::new(vec![
                Attribute::new(
                    Symbol::new("dim"),
                    RObject::from_parts(
                        RValue::Integer(vec![None, Some(1)]),
                        Attributes::default(),
                    ),
                ),
                Attribute::new(
                    Symbol::new("dimnames"),
                    RObject::from_parts(
                        RValue::List(vec![names(&["row"]), names(&["Package"])]),
                        Attributes::default(),
                    ),
                ),
            ]),
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::InvalidDimensions { path, .. }) if path == "PACKAGES.attributes.dim")
        );
        let mut object = valid();
        set_attributes(
            &mut object,
            Attributes::new(vec![
                Attribute::new(
                    Symbol::new("dim"),
                    RObject::from_parts(
                        RValue::Integer(vec![Some(-1), Some(1)]),
                        Attributes::default(),
                    ),
                ),
                Attribute::new(
                    Symbol::new("dimnames"),
                    RObject::from_parts(
                        RValue::List(vec![names(&[]), names(&["Package"])]),
                        Attributes::default(),
                    ),
                ),
            ]),
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::InvalidDimensions { path, .. }) if path == "PACKAGES.attributes.dim")
        );
        let mut object = valid();
        replace_dimnames(
            &mut object,
            vec![names(&["row"]), names(&["Package", "Version"])],
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dimnames[1]")
        );
        let mut object = valid();
        replace_dimnames(&mut object, vec![names(&["row"]), names(&["Package"])]);
        set_value(&mut object, RValue::Character(vec![]));
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES")
        );
        let object = RObject::from_parts(
            RValue::Character(vec![RStr::new(b"x", REncoding::Native, None)]),
            Attributes::new(vec![Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(
                    RValue::Integer(vec![Some(1), Some(1)]),
                    Attributes::default(),
                ),
            )]),
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::Missing { path, .. }) if path == "PACKAGES.attributes.dimnames")
        );
        let mut object = valid();
        replace_dimnames(&mut object, vec![names(&["row"])]);
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedLength { path, .. }) if path == "PACKAGES.attributes.dimnames")
        );
        let object = matrix(
            vec![Some(i32::MAX), Some(i32::MAX)],
            vec![
                RObject::from_parts(RValue::Null, Attributes::default()),
                RObject::from_parts(RValue::Character(vec![]), Attributes::default()),
            ],
            vec![],
        );
        // i32::MAX * i32::MAX fits in a 64-bit usize (data-length mismatch)
        // but overflows a 32-bit usize (dimension-product overflow).
        let error = PackagesMatrix::from_object(&object).unwrap_err();
        if usize::BITS >= 64 {
            assert!(
                matches!(error, ViewError::UnexpectedLength { ref path, .. } if path == "PACKAGES")
            );
        } else {
            assert!(matches!(error, ViewError::InvalidDimensions { .. }));
        }
    }

    #[test]
    fn packages_matrix_rejects_na_and_duplicate_column_names() {
        let mut object = matrix(
            vec![Some(1), Some(2)],
            vec![names(&["row"]), names(&["Package", "Package"])],
            vec![
                RStr::new(b"x", REncoding::Native, None),
                RStr::new(b"y", REncoding::Native, None),
            ],
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::DuplicateName { path, .. }) if path == "PACKAGES.attributes.dimnames[1][1]")
        );
        replace_dimnames(
            &mut object,
            vec![
                names(&["row"]),
                RObject::from_parts(
                    RValue::Character(vec![
                        RStr::Na,
                        RStr::new(b"Version", REncoding::Native, None),
                    ]),
                    Attributes::default(),
                ),
            ],
        );
        assert!(
            matches!(PackagesMatrix::from_object(&object), Err(ViewError::UnexpectedType { path, .. }) if path == "PACKAGES.attributes.dimnames[1][0]")
        );
    }
}
