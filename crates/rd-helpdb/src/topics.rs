//! Typed access to `Meta/Rd.rds`, independent of the compiled help database.

use std::{collections::BTreeMap, path::Path};

use rd_rds::{RObject, RValue, file::ReadOptions};

use crate::{Error, rds::map_file_error, util::rstr_to_string};

/// A title or source file name in help-topic metadata.
///
/// Missing columns, R `NA`, and malformed values remain distinct. An invalid
/// optional field does not prevent reading the other fields or rows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HelpTopicText {
    /// The column is absent.
    Missing,
    /// The stored character value is R `NA`.
    Na,
    /// The column has the wrong type, this row is absent, the column exceeds
    /// the row count, or the string cannot be decoded. The explanation is
    /// diagnostic text, not a stable machine-readable format.
    Invalid(String),
    /// A decoded value, including an empty string.
    Text(String),
}

impl HelpTopicText {
    /// Returns usable text, collapsing missing, NA, and invalid fields to `None`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }
}

/// One row of `Meta/Rd.rds`, in stored order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HelpTopicEntry {
    /// Aliases in stored order, including duplicates and R `NA` as `None`.
    pub aliases: Vec<Option<String>>,
    /// The optional `Title` value. No whitespace normalization is applied.
    pub title: HelpTopicText,
    /// The optional `File` value, preserved exactly as decoded.
    pub file: HelpTopicText,
}

impl HelpTopicEntry {
    /// Derives the help-database key from the basename of [`Self::file`],
    /// removing one trailing `.Rd` or `.rd` suffix as R does. Trailing path
    /// separators are ignored. The stored file value is unchanged, and an
    /// empty basename yields an empty key. This does not establish that a
    /// corresponding topic exists in the help database.
    pub fn topic_key(&self) -> Option<&str> {
        self.file.as_str().map(|file| {
            let basename = file
                .trim_end_matches(std::path::is_separator)
                .rsplit(std::path::is_separator)
                .next()
                .unwrap_or(file);
            basename
                .strip_suffix(".Rd")
                .or_else(|| basename.strip_suffix(".rd"))
                .unwrap_or(basename)
        })
    }
}

/// An owned view of an installed package's `Meta/Rd.rds` topic metadata.
///
/// This source is separate from `help/aliases.rds`. Rows and alias groups are
/// preserved, and [`Self::find_alias`] selects the **first** matching row.
/// [`crate::PackageHelpDb::resolve_alias`] instead uses the **last** occurrence
/// in `aliases.rds`. Neither lookup consults or overrides the other source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpTopicIndex {
    entries: Vec<HelpTopicEntry>,
}

impl HelpTopicIndex {
    /// Reads `Meta/Rd.rds` below an explicitly named installed-package directory.
    ///
    /// Returns `Ok(None)` only for a missing path, `Ok(Some(index))` for
    /// present metadata (including a valid empty index), and an error for
    /// other I/O failures, decoding failures, or an invalid required schema.
    /// No help `.rdx`, `.rdb`, or `aliases.rds` file is opened.
    pub fn read_installed(package_dir: impl AsRef<Path>) -> Result<Option<Self>, Error> {
        Self::read_installed_with_options(package_dir, &ReadOptions::default())
    }

    /// Reads installed metadata with explicit file, decompression, and decoder
    /// bounds and encoding policy supplied to [`rd_rds::file`].
    pub fn read_installed_with_options(
        package_dir: impl AsRef<Path>,
        options: &ReadOptions,
    ) -> Result<Option<Self>, Error> {
        let path = package_dir.as_ref().join("Meta/Rd.rds");
        match rd_rds::file::read_with_options(&path, options) {
            Ok(root) => Self::from_object(&root).map(Some),
            Err(rd_rds::file::ReadError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(None)
            }
            Err(error) => Err(map_file_error(&path, error)),
        }
    }

    /// Validates and copies a decoded topic metadata data frame.
    ///
    /// The root must be a named list with class `data.frame`, unique non-NA
    /// column names, and `row.names` agreeing with the required `Aliases`
    /// list column. Each alias cell must be a character vector; NA aliases
    /// are retained but never match a lookup. Invalid alias strings are
    /// errors. Additional columns are ignored.
    ///
    /// `Title` and `File` are optional character columns. Missing columns
    /// and NA values are retained explicitly. Wrong column types, excess
    /// values, missing row values in short columns, and undecodable strings
    /// become [`HelpTopicText::Invalid`], preserving usable neighboring
    /// fields and rows. RDS decoding errors remain fatal before this view
    /// can recover individual fields.
    pub fn from_object(root: &RObject) -> Result<Self, Error> {
        let RValue::List(columns) = root.value() else {
            return Err(malformed("root is not a list"));
        };
        if !root.class().is_some_and(|classes| {
            classes
                .iter()
                .any(|class| matches!(class.as_str(), Some(Ok(value)) if value == "data.frame"))
        }) {
            return Err(malformed("class does not include \"data.frame\""));
        }
        let names = root
            .names()
            .ok_or_else(|| malformed("missing character names attribute"))?;
        if names.len() != columns.len() {
            return Err(malformed("column names and columns have different lengths"));
        }
        let mut positions = BTreeMap::new();
        for (position, name) in names.iter().enumerate() {
            let name = rstr_to_string(name).map_err(|error| {
                malformed(format!("invalid column name at {position}: {error}"))
            })?;
            if positions.insert(name.clone(), position).is_some() {
                return Err(malformed(format!("duplicate column name {name:?}")));
            }
        }
        let column = |name: &str| positions.get(name).map(|&index| &columns[index]);
        let aliases =
            column("Aliases").ok_or_else(|| malformed("missing required column \"Aliases\""))?;
        let RValue::List(aliases) = aliases.value() else {
            return Err(malformed("column \"Aliases\" is not a list"));
        };
        let nrow = aliases.len();
        validate_row_count(root, nrow)?;
        let mut entries = Vec::with_capacity(nrow);
        for (row, cell) in aliases.iter().enumerate() {
            let RValue::Character(values) = cell.value() else {
                return Err(malformed(format!(
                    "Aliases at row {row} is not a character vector"
                )));
            };
            let aliases = values
                .iter()
                .enumerate()
                .map(|(element, value)| {
                    value
                        .as_str()
                        .map(|result| result.map(|text| text.into_owned()))
                        .transpose()
                        .map_err(|error| {
                            malformed(format!(
                                "invalid Aliases at row {row}, element {element}: {error}"
                            ))
                        })
                })
                .collect::<Result<_, _>>()?;
            entries.push(HelpTopicEntry {
                aliases,
                title: optional_text(column("Title"), "Title", row, nrow),
                file: optional_text(column("File"), "File", row, nrow),
            });
        }
        Ok(Self { entries })
    }

    /// Iterates over entries in stored row order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &HelpTopicEntry> {
        self.entries.iter()
    }

    /// Finds the first row containing this alias, ignoring NA alias values.
    /// Lookup is case-sensitive and does not normalize text. A first match
    /// with an unavailable title or file still wins over later matches.
    pub fn find_alias(&self, alias: &str) -> Option<&HelpTopicEntry> {
        self.entries.iter().find(|entry| {
            entry
                .aliases
                .iter()
                .any(|value| value.as_deref() == Some(alias))
        })
    }

    /// Returns the number of stored rows, including rows with no aliases.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the metadata contains no rows.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl TryFrom<&RObject> for HelpTopicIndex {
    type Error = Error;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

fn optional_text(column: Option<&RObject>, name: &str, row: usize, nrow: usize) -> HelpTopicText {
    let Some(column) = column else {
        return HelpTopicText::Missing;
    };
    let RValue::Character(values) = column.value() else {
        return HelpTopicText::Invalid(format!("{name} is not a character vector"));
    };
    if values.len() > nrow {
        return HelpTopicText::Invalid(format!(
            "{name} has {} values for {nrow} rows",
            values.len()
        ));
    }
    let Some(value) = values.get(row) else {
        return HelpTopicText::Invalid(format!("{name} has no value at row {row}"));
    };
    match value.as_str() {
        None => HelpTopicText::Na,
        Some(Ok(value)) => HelpTopicText::Text(value.into_owned()),
        Some(Err(error)) => HelpTopicText::Invalid(format!("{name} at row {row}: {error}")),
    }
}

fn validate_row_count(root: &RObject, nrow: usize) -> Result<(), Error> {
    let row_names = root
        .attributes()
        .get("row.names")
        .ok_or_else(|| malformed("missing row.names attribute"))?;
    let count = match row_names.value() {
        // R's compact row names encode the count in the second element.
        RValue::Integer(values) if values.len() == 2 && values[0].is_none() => values[1]
            .ok_or_else(|| malformed("NA row count in row.names"))?
            .unsigned_abs()
            as usize,
        RValue::Integer(values) => values.len(),
        RValue::Character(values) => values.len(),
        _ => return Err(malformed("row.names is not an integer or character vector")),
    };
    if count != nrow {
        return Err(malformed(format!(
            "row.names implies {count} rows but Aliases has {nrow}"
        )));
    }
    Ok(())
}

fn malformed(message: impl Into<String>) -> Error {
    Error::MalformedIndex(format!("invalid Meta/Rd.rds: {}", message.into()))
}
