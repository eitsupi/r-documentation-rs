use super::{
    ValueKindName, ViewError, decode_required, duplicate, invalid_dimensions, missing,
    unexpected_length, unexpected_type,
};
use crate::{RObject, RStr, RValue};

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
