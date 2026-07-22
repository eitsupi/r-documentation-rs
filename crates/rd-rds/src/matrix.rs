//! Validated, owned views of R character matrices.
//!
//! [`CharacterMatrix`] accepts the ordinary R matrix shape: a character
//! vector with a required two-element integer `dim` attribute and an optional
//! `dimnames` attribute. R stores matrix cells in column-major order; the view
//! copies them into row-major order for Rust callers.

use thiserror::Error;

use crate::{RObject, RStr, RValue};

/// A construction error from [`CharacterMatrix`].
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ViewError {
    #[error("missing value at {path}")]
    Missing { path: String },
    #[error("unexpected type at {path}: expected {expected}, got {actual}")]
    UnexpectedType {
        path: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("unexpected length at {path}: expected {expected}, got {actual}")]
    UnexpectedLength {
        path: String,
        expected: String,
        actual: usize,
    },
    #[error("invalid dimensions at {path}: {reason}")]
    InvalidDimensions { path: String, reason: String },
    #[error("invalid string encoding at {path}")]
    InvalidStringEncoding { path: String },
}

impl ViewError {
    /// Returns the logical location of the invalid value.
    pub fn path(&self) -> &str {
        match self {
            Self::Missing { path }
            | Self::UnexpectedType { path, .. }
            | Self::UnexpectedLength { path, .. }
            | Self::InvalidDimensions { path, .. }
            | Self::InvalidStringEncoding { path } => path,
        }
    }
}

/// A validated, owned view of an R character matrix.
///
/// Matrix cells are stored in row-major order. An R `NA` cell is represented
/// by the inner `None` returned from [`CharacterMatrix::get`], while an
/// out-of-bounds lookup is represented by the outer `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterMatrix {
    nrow: usize,
    ncol: usize,
    cells: Vec<Option<String>>,
    row_names: Option<Vec<Option<String>>>,
    column_names: Option<Vec<Option<String>>>,
}

impl CharacterMatrix {
    /// Validates and copies an R character matrix.
    pub fn from_object(object: &RObject) -> Result<Self, ViewError> {
        let values = match object.value() {
            RValue::Character(values) => values,
            value => {
                return Err(unexpected_type(
                    "CharacterMatrix",
                    "character vector",
                    kind_name(value),
                ));
            }
        };

        let dimensions = object
            .attributes()
            .get("dim")
            .ok_or_else(|| missing("CharacterMatrix.attributes.dim"))?;
        let dimensions = match dimensions.value() {
            RValue::Integer(values) => values,
            value => {
                return Err(unexpected_type(
                    "CharacterMatrix.attributes.dim",
                    "integer vector",
                    kind_name(value),
                ));
            }
        };
        if dimensions.len() != 2 {
            return Err(unexpected_length(
                "CharacterMatrix.attributes.dim",
                "2",
                dimensions.len(),
            ));
        }

        let mut shape = [0usize; 2];
        for (index, value) in dimensions.iter().enumerate() {
            let Some(value) = value else {
                return Err(invalid_dimensions(
                    "CharacterMatrix.attributes.dim",
                    "dimensions must not contain NA",
                ));
            };
            if *value < 0 {
                return Err(invalid_dimensions(
                    "CharacterMatrix.attributes.dim",
                    "dimensions must not be negative",
                ));
            }
            shape[index] = *value as usize;
        }

        let element_count = shape[0].checked_mul(shape[1]).ok_or_else(|| {
            invalid_dimensions(
                "CharacterMatrix.attributes.dim",
                "dimension product overflows",
            )
        })?;
        if values.len() != element_count {
            return Err(unexpected_length(
                "CharacterMatrix",
                &element_count.to_string(),
                values.len(),
            ));
        }

        let (row_names, column_names) = match object.attributes().get("dimnames") {
            None => (None, None),
            Some(dimnames) => {
                let dimnames = match dimnames.value() {
                    RValue::List(values) => values,
                    value => {
                        return Err(unexpected_type(
                            "CharacterMatrix.attributes.dimnames",
                            "list",
                            kind_name(value),
                        ));
                    }
                };
                if dimnames.len() != 2 {
                    return Err(unexpected_length(
                        "CharacterMatrix.attributes.dimnames",
                        "2",
                        dimnames.len(),
                    ));
                }
                (
                    decode_names(&dimnames[0], shape[0], 0)?,
                    decode_names(&dimnames[1], shape[1], 1)?,
                )
            }
        };

        let mut cells = Vec::with_capacity(element_count);
        for row in 0..shape[0] {
            for column in 0..shape[1] {
                cells.push(decode_string(
                    &values[row + column * shape[0]],
                    &format!("CharacterMatrix[row={row},column={column}]"),
                )?);
            }
        }

        Ok(Self {
            nrow: shape[0],
            ncol: shape[1],
            cells,
            row_names,
            column_names,
        })
    }

    /// Returns the number of rows.
    pub fn nrow(&self) -> usize {
        self.nrow
    }

    /// Returns the number of columns.
    pub fn ncol(&self) -> usize {
        self.ncol
    }

    /// Returns a cell, distinguishing an out-of-bounds lookup from an R `NA`.
    pub fn get(&self, row: usize, column: usize) -> Option<Option<&str>> {
        if row >= self.nrow || column >= self.ncol {
            return None;
        }
        Some(self.cells[row * self.ncol + column].as_deref())
    }

    /// Returns a row name, or `None` if it is absent, `NA`, or out of bounds.
    pub fn row_name(&self, row: usize) -> Option<&str> {
        self.row_names
            .as_ref()?
            .get(row)?
            .as_ref()
            .map(String::as_str)
    }

    /// Returns a column name, or `None` if it is absent, `NA`, or out of bounds.
    pub fn column_name(&self, column: usize) -> Option<&str> {
        self.column_names
            .as_ref()?
            .get(column)?
            .as_ref()
            .map(String::as_str)
    }
}

impl TryFrom<&RObject> for CharacterMatrix {
    type Error = ViewError;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

fn decode_names(
    object: &RObject,
    expected: usize,
    axis: usize,
) -> Result<Option<Vec<Option<String>>>, ViewError> {
    let values = match object.value() {
        RValue::Null => return Ok(None),
        RValue::Character(values) => values,
        value => {
            return Err(unexpected_type(
                &format!("CharacterMatrix.attributes.dimnames[{axis}]"),
                "NULL or character vector",
                kind_name(value),
            ));
        }
    };
    if values.len() != expected {
        return Err(unexpected_length(
            &format!("CharacterMatrix.attributes.dimnames[{axis}]"),
            &expected.to_string(),
            values.len(),
        ));
    }

    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            decode_string(
                value,
                &format!("CharacterMatrix.attributes.dimnames[{axis}][{index}]"),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn decode_string(value: &RStr, path: &str) -> Result<Option<String>, ViewError> {
    match value.as_str() {
        None => Ok(None),
        Some(Ok(value)) => Ok(Some(value.into_owned())),
        Some(Err(_)) => Err(ViewError::InvalidStringEncoding {
            path: path.to_owned(),
        }),
    }
}

fn missing(path: &str) -> ViewError {
    ViewError::Missing {
        path: path.to_owned(),
    }
}

fn unexpected_type(path: &str, expected: &'static str, actual: &'static str) -> ViewError {
    ViewError::UnexpectedType {
        path: path.to_owned(),
        expected,
        actual,
    }
}

fn unexpected_length(path: &str, expected: &str, actual: usize) -> ViewError {
    ViewError::UnexpectedLength {
        path: path.to_owned(),
        expected: expected.to_owned(),
        actual,
    }
}

fn invalid_dimensions(path: &str, reason: &str) -> ViewError {
    ViewError::InvalidDimensions {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}

fn kind_name(value: &RValue) -> &'static str {
    match value {
        RValue::Null => "NULL",
        RValue::Logical(_) => "logical vector",
        RValue::Integer(_) => "integer vector",
        RValue::Real(_) => "real vector",
        RValue::Character(_) => "character vector",
        RValue::List(_) => "list",
        RValue::Symbol(_) => "symbol",
        RValue::Persisted(_) => "persisted reference",
        RValue::Environment(_) => "environment",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Attributes, REncoding, Symbol};

    fn strings(values: &[Option<&str>]) -> Vec<RStr> {
        values
            .iter()
            .map(|value| match value {
                Some(value) => RStr::new(value.as_bytes(), REncoding::Utf8, None),
                None => RStr::Na,
            })
            .collect()
    }

    fn character(values: &[Option<&str>]) -> RObject {
        RObject::from_parts(RValue::Character(strings(values)), Attributes::default())
    }

    fn null() -> RObject {
        RObject::from_parts(RValue::Null, Attributes::default())
    }

    fn matrix(
        dim: Option<Vec<Option<i32>>>,
        dimnames: Option<Vec<RObject>>,
        values: &[Option<&str>],
    ) -> RObject {
        let mut attributes = Vec::new();
        if let Some(dim) = dim {
            attributes.push(Attribute::new(
                Symbol::new("dim"),
                RObject::from_parts(RValue::Integer(dim), Attributes::default()),
            ));
        }
        if let Some(dimnames) = dimnames {
            attributes.push(Attribute::new(
                Symbol::new("dimnames"),
                RObject::from_parts(RValue::List(dimnames), Attributes::default()),
            ));
        }
        RObject::from_parts(
            RValue::Character(strings(values)),
            Attributes::new(attributes),
        )
    }

    #[test]
    fn requires_valid_dimensions_and_matching_cell_count() {
        let missing_dim = matrix(None, None, &[]);
        assert!(matches!(
            CharacterMatrix::from_object(&missing_dim),
            Err(ViewError::Missing { ref path }) if path == "CharacterMatrix.attributes.dim"
        ));

        let short_dim = matrix(Some(vec![Some(1)]), None, &[Some("x")]);
        assert!(matches!(
            CharacterMatrix::from_object(&short_dim),
            Err(ViewError::UnexpectedLength { ref path, .. }) if path == "CharacterMatrix.attributes.dim"
        ));

        let negative_dim = matrix(Some(vec![Some(-1), Some(0)]), None, &[]);
        assert!(matches!(
            CharacterMatrix::from_object(&negative_dim),
            Err(ViewError::InvalidDimensions { ref path, .. }) if path == "CharacterMatrix.attributes.dim"
        ));

        let wrong_cell_count = matrix(Some(vec![Some(2), Some(2)]), None, &[Some("x")]);
        assert!(matches!(
            CharacterMatrix::from_object(&wrong_cell_count),
            Err(ViewError::UnexpectedLength { ref path, .. }) if path == "CharacterMatrix"
        ));
    }

    #[test]
    fn converts_to_row_major_and_preserves_na_and_empty_strings() {
        let object = matrix(
            Some(vec![Some(2), Some(2)]),
            None,
            &[Some("a"), None, Some(""), Some("d")],
        );
        let matrix = CharacterMatrix::try_from(&object).expect("valid matrix");

        assert_eq!(matrix.nrow(), 2);
        assert_eq!(matrix.ncol(), 2);
        assert_eq!(matrix.get(0, 0), Some(Some("a")));
        assert_eq!(matrix.get(0, 1), Some(Some("")));
        assert_eq!(matrix.get(1, 0), Some(None));
        assert_eq!(matrix.get(1, 1), Some(Some("d")));
        assert_eq!(matrix.get(2, 0), None);
        assert_eq!(matrix.get(0, 2), None);
        assert_eq!(matrix.row_name(0), None);
        assert_eq!(matrix.column_name(0), None);
    }

    #[test]
    fn accepts_optional_well_shaped_dimnames() {
        let object = matrix(
            Some(vec![Some(2), Some(1)]),
            Some(vec![character(&[Some("one"), None]), null()]),
            &[Some("a"), Some("b")],
        );
        let matrix = CharacterMatrix::from_object(&object).expect("valid dimnames");

        assert_eq!(matrix.row_name(0), Some("one"));
        assert_eq!(matrix.row_name(1), None);
        assert_eq!(matrix.row_name(2), None);
        assert_eq!(matrix.column_name(0), None);
    }

    #[test]
    fn rejects_malformed_dimnames_shapes() {
        let short_list = matrix(
            Some(vec![Some(1), Some(1)]),
            Some(vec![null()]),
            &[Some("x")],
        );
        assert!(matches!(
            CharacterMatrix::from_object(&short_list),
            Err(ViewError::UnexpectedLength { ref path, .. }) if path == "CharacterMatrix.attributes.dimnames"
        ));

        let wrong_axis_length = matrix(
            Some(vec![Some(1), Some(1)]),
            Some(vec![character(&[Some("one"), Some("two")]), null()]),
            &[Some("x")],
        );
        assert!(matches!(
            CharacterMatrix::from_object(&wrong_axis_length),
            Err(ViewError::UnexpectedLength { ref path, .. }) if path == "CharacterMatrix.attributes.dimnames[0]"
        ));
    }
}
