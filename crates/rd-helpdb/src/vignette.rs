//! Typed access to an installed package's `Meta/vignette.rds` index.

use std::collections::BTreeMap;

use rd_rds::{RObject, RStr, RValue};

use crate::{Error, util::rstr_to_string};

const REQUIRED_COLUMNS: [&str; 6] = ["File", "Title", "PDF", "R", "Depends", "Keywords"];

/// One row of `Meta/vignette.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VignetteEntry {
    pub file: String,
    pub title: String,
    pub pdf: String,
    pub r: String,
    pub depends: Vec<String>,
    pub keywords: Vec<String>,
}

/// A validated, owned `Meta/vignette.rds` data frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VignetteIndex {
    entries: Vec<VignetteEntry>,
}

impl VignetteIndex {
    /// Validates and copies a decoded `Meta/vignette.rds` object.
    pub fn from_object(root: &RObject) -> Result<Self, Error> {
        let columns = match root.value() {
            RValue::List(columns) => columns,
            _ => return Err(malformed("root is not a list")),
        };
        require_data_frame_class(root)?;

        let names = root
            .names()
            .ok_or_else(|| malformed("missing character names attribute"))?;
        if names.len() != columns.len() {
            return Err(malformed(format!(
                "has {} column names but {} columns",
                names.len(),
                columns.len()
            )));
        }

        let mut positions = BTreeMap::new();
        for (index, name) in names.iter().enumerate() {
            let name = rstr_to_string(name).map_err(|error| {
                malformed(format!("invalid column name at position {index}: {error}"))
            })?;
            if positions.insert(name.clone(), index).is_some() {
                return Err(malformed(format!("duplicate column name {name:?}")));
            }
        }
        for name in REQUIRED_COLUMNS {
            if !positions.contains_key(name) {
                return Err(malformed(format!("missing required column {name:?}")));
            }
        }

        let files = character_column(columns, &positions, "File")?;
        let titles = character_column(columns, &positions, "Title")?;
        let pdfs = character_column(columns, &positions, "PDF")?;
        let r_sources = character_column(columns, &positions, "R")?;
        let depends = list_column(columns, &positions, "Depends")?;
        let keywords = list_column(columns, &positions, "Keywords")?;
        let nrow = files.len();

        for (name, actual) in [
            ("Title", titles.len()),
            ("PDF", pdfs.len()),
            ("R", r_sources.len()),
            ("Depends", depends.len()),
            ("Keywords", keywords.len()),
        ] {
            if actual != nrow {
                return Err(malformed(format!(
                    "column {name:?} has length {actual}, expected {nrow}"
                )));
            }
        }

        let mut entries = Vec::with_capacity(nrow);
        for row in 0..nrow {
            entries.push(VignetteEntry {
                file: required_string(&files[row], "File", row)?,
                title: required_string(&titles[row], "Title", row)?,
                pdf: required_string(&pdfs[row], "PDF", row)?,
                r: required_string(&r_sources[row], "R", row)?,
                depends: string_vector(&depends[row], "Depends", row)?,
                keywords: string_vector(&keywords[row], "Keywords", row)?,
            });
        }

        Ok(Self { entries })
    }

    /// Iterates over entries in their on-disk row order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &VignetteEntry> {
        self.entries.iter()
    }

    /// Returns the number of vignette entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the index contains no vignette entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl TryFrom<&RObject> for VignetteIndex {
    type Error = Error;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

fn require_data_frame_class(root: &RObject) -> Result<(), Error> {
    let classes = root
        .class()
        .ok_or_else(|| malformed("missing character class attribute"))?;
    for (index, class) in classes.iter().enumerate() {
        match class.as_str() {
            Some(Ok(value)) if value == "data.frame" => return Ok(()),
            Some(Ok(_)) | None => {}
            Some(Err(error)) => {
                return Err(malformed(format!(
                    "invalid class string at position {index}: {error}"
                )));
            }
        }
    }
    Err(malformed("class does not include \"data.frame\""))
}

fn character_column<'a>(
    columns: &'a [RObject],
    positions: &BTreeMap<String, usize>,
    name: &str,
) -> Result<&'a [RStr], Error> {
    match columns[positions[name]].value() {
        RValue::Character(values) => Ok(values),
        _ => Err(malformed(format!(
            "column {name:?} is not a character vector"
        ))),
    }
}

fn list_column<'a>(
    columns: &'a [RObject],
    positions: &BTreeMap<String, usize>,
    name: &str,
) -> Result<&'a [RObject], Error> {
    match columns[positions[name]].value() {
        RValue::List(values) => Ok(values),
        _ => Err(malformed(format!("column {name:?} is not a list"))),
    }
}

fn required_string(value: &RStr, column: &str, row: usize) -> Result<String, Error> {
    rstr_to_string(value).map_err(|error| {
        malformed(format!(
            "invalid value at row {row}, column {column:?}: {error}"
        ))
    })
}

fn string_vector(object: &RObject, column: &str, row: usize) -> Result<Vec<String>, Error> {
    let values = match object.value() {
        RValue::Character(values) => values,
        _ => {
            return Err(malformed(format!(
                "value at row {row}, column {column:?} is not a character vector"
            )));
        }
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            rstr_to_string(value).map_err(|error| {
                malformed(format!(
                    "invalid value at row {row}, column {column:?}, element {index}: {error}"
                ))
            })
        })
        .collect()
}

fn malformed(message: impl Into<String>) -> Error {
    Error::MalformedIndex(format!("invalid Meta/vignette.rds: {}", message.into()))
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
    fn parses_reordered_columns_and_list_columns() {
        let index = VignetteIndex::from_object(&fixture("vignette_reordered_v3.rds"))
            .expect("valid vignette fixture");
        assert_eq!(index.len(), 2);
        assert_eq!(
            index.entries().collect::<Vec<_>>(),
            vec![
                &VignetteEntry {
                    file: "first.Rnw".into(),
                    title: "First vignette".into(),
                    pdf: "first.pdf".into(),
                    r: "first.R".into(),
                    depends: vec!["tools".into(), "stats".into()],
                    keywords: vec!["models".into()],
                },
                &VignetteEntry {
                    file: "second.Rmd".into(),
                    title: "Second vignette".into(),
                    pdf: "second.html".into(),
                    r: "second.R".into(),
                    depends: Vec::new(),
                    keywords: vec!["".into()],
                },
            ]
        );
    }

    #[test]
    fn accepts_zero_row_data_frame() {
        let index = VignetteIndex::from_object(&fixture("vignette_empty_v3.rds"))
            .expect("empty vignette fixture");
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
        assert_eq!(index.entries().len(), 0);
    }

    #[test]
    fn rejects_missing_required_column() {
        let error = VignetteIndex::from_object(&fixture("vignette_missing_column_v3.rds"))
            .expect_err("missing column must fail");
        assert!(
            error
                .to_string()
                .contains("missing required column \"Keywords\"")
        );
    }
}
