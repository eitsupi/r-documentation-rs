//! Typed access to an installed package's `Meta/demo.rds` index.

use rd_rds::{RObject, matrix::CharacterMatrix};

use crate::Error;

/// One row of `Meta/demo.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoEntry {
    pub name: String,
    pub title: String,
}

/// A validated, owned `Meta/demo.rds` matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoIndex {
    entries: Vec<DemoEntry>,
}

impl DemoIndex {
    /// Validates and copies a decoded `Meta/demo.rds` object.
    pub fn from_object(root: &RObject) -> Result<Self, Error> {
        let matrix = CharacterMatrix::try_from(root)
            .map_err(|error| malformed(format!("invalid character matrix: {error}")))?;
        if matrix.ncol() != 2 {
            return Err(malformed(format!(
                "expected exactly 2 columns, got {}",
                matrix.ncol()
            )));
        }

        let mut entries = Vec::with_capacity(matrix.nrow());
        for row in 0..matrix.nrow() {
            let name = required_cell(&matrix, row, 0, "Name")?;
            let title = required_cell(&matrix, row, 1, "Title")?;
            entries.push(DemoEntry { name, title });
        }
        Ok(Self { entries })
    }

    /// Iterates over entries in their on-disk row order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &DemoEntry> {
        self.entries.iter()
    }

    /// Returns the number of demo entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the index contains no demo entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl TryFrom<&RObject> for DemoIndex {
    type Error = Error;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

fn required_cell(
    matrix: &CharacterMatrix,
    row: usize,
    column: usize,
    label: &str,
) -> Result<String, Error> {
    match matrix.get(row, column) {
        Some(Some(value)) => Ok(value.to_owned()),
        Some(None) => Err(malformed(format!(
            "unexpected NA at row {row}, column {column} ({label})"
        ))),
        None => Err(malformed(format!(
            "missing cell at row {row}, column {column} ({label})"
        ))),
    }
}

fn malformed(message: impl Into<String>) -> Error {
    Error::MalformedIndex(format!("invalid Meta/demo.rds: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::read_rds_file;

    use super::*;

    fn fixture(name: &str) -> RObject {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/data")
            .join(name);
        read_rds_file(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
    }

    #[test]
    fn parses_two_columns_by_position_without_dimnames() {
        let index =
            DemoIndex::from_object(&fixture("demo_valid_v3.rds")).expect("valid demo fixture");
        assert_eq!(
            index.entries().collect::<Vec<_>>(),
            vec![
                &DemoEntry {
                    name: "first".into(),
                    title: "First demo".into(),
                },
                &DemoEntry {
                    name: "second".into(),
                    title: "".into(),
                },
            ]
        );
    }

    #[test]
    fn accepts_zero_row_matrix() {
        let index =
            DemoIndex::from_object(&fixture("demo_empty_v3.rds")).expect("empty demo fixture");
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
        assert_eq!(index.entries().len(), 0);
    }

    #[test]
    fn rejects_wrong_column_count() {
        let error = DemoIndex::from_object(&fixture("demo_three_columns_v3.rds"))
            .expect_err("three columns must fail");
        assert!(error.to_string().contains("expected exactly 2 columns"));
    }
}
