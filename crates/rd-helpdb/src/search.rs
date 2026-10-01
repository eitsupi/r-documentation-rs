//! Typed access to an installed package's `Meta/hsearch.rds` index.

use std::{collections::BTreeMap, path::Path};

use rd_rds::{RObject, RValue, file::ReadOptions, matrix::CharacterMatrix};

use crate::{Error, rds::map_file_error};

/// One row of the help-file matrix in `Meta/hsearch.rds`.
///
/// Values are kept as optional strings so R `NA` remains distinct from an
/// empty string.  This view does not interpret IDs or establish references
/// between matrices.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HelpSearchBaseEntry {
    pub package: Option<String>,
    pub lib_path: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub topic: Option<String>,
    pub encoding: Option<String>,
}

/// One row of the alias matrix in `Meta/hsearch.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HelpSearchAliasEntry {
    pub alias: Option<String>,
    pub id: Option<String>,
    pub package: Option<String>,
}

/// One row of the keyword matrix in `Meta/hsearch.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HelpSearchKeywordEntry {
    pub keyword: Option<String>,
    pub id: Option<String>,
    pub package: Option<String>,
}

/// One row of the concept matrix in `Meta/hsearch.rds`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HelpSearchConceptEntry {
    pub concept: Option<String>,
    pub id: Option<String>,
    pub package: Option<String>,
}

/// A validated, owned view of the four matrices in `Meta/hsearch.rds`.
///
/// The matrices and their rows retain the stored order, duplicate values,
/// empty strings, and R `NA` values.  This is a metadata reader only: it does
/// not implement `utils::help.search()` matching, ranking, or cross-package
/// lookup policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpSearchIndex {
    base: Vec<HelpSearchBaseEntry>,
    aliases: Vec<HelpSearchAliasEntry>,
    keywords: Vec<HelpSearchKeywordEntry>,
    concepts: Vec<HelpSearchConceptEntry>,
}

impl HelpSearchIndex {
    /// Reads `Meta/hsearch.rds` below an explicitly named installed-package
    /// directory.
    ///
    /// `Ok(None)` means that the file is absent.  A present, valid empty
    /// index is returned as `Ok(Some(index))`; all decoding and schema errors
    /// are returned as errors.
    pub fn read_installed(package_dir: impl AsRef<Path>) -> Result<Option<Self>, Error> {
        Self::read_installed_with_options(package_dir, &ReadOptions::default())
    }

    /// Reads installed help-search metadata with explicit RDS read options.
    pub fn read_installed_with_options(
        package_dir: impl AsRef<Path>,
        options: &ReadOptions,
    ) -> Result<Option<Self>, Error> {
        let path = package_dir.as_ref().join("Meta/hsearch.rds");
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

    /// Validates and copies a decoded `Meta/hsearch.rds` object.
    ///
    /// The root must be an unnamed list of four character matrices. Current
    /// R schemas use `Package, LibPath, ID, Name, Title, Topic, Encoding` for
    /// the first matrix and singular `Alias`, `Keyword`, and `Concept` names
    /// for the remaining matrices. The known R 1.8/R 2.9-compatible lower-case
    /// topic columns, plural relation names, and six- or seven-column forms
    /// without or with `Encoding` are accepted as bounded compatibility cases.
    /// Matrix column order is resolved by column name, but unknown, missing,
    /// duplicate, or NA column names are rejected. A missing legacy `Encoding`
    /// column is represented as `Some("")`, matching R's compatibility reader.
    /// Historical compatibility is limited to these documented shapes; this
    /// does not promise decoding every file produced by every old R release.
    /// RDS serialization versions 2 and 3 are accepted according to the
    /// profiles supported by `rd-rds`.
    pub fn from_object(root: &RObject) -> Result<Self, Error> {
        let RValue::List(matrices) = root.value() else {
            return Err(malformed("root is not a list"));
        };
        if root.attributes().get("names").is_some() {
            return Err(malformed("root list must be unnamed"));
        }
        if matrices.len() != 4 {
            return Err(malformed(format!(
                "root has {} matrices, expected 4",
                matrices.len()
            )));
        }

        let base = CharacterMatrix::from_object(&matrices[0])
            .map_err(|error| malformed(format!("invalid Base matrix: {error}")))?;
        let aliases = CharacterMatrix::from_object(&matrices[1])
            .map_err(|error| malformed(format!("invalid Aliases matrix: {error}")))?;
        let keywords = CharacterMatrix::from_object(&matrices[2])
            .map_err(|error| malformed(format!("invalid Keywords matrix: {error}")))?;
        let concepts = CharacterMatrix::from_object(&matrices[3])
            .map_err(|error| malformed(format!("invalid Concepts matrix: {error}")))?;

        let base_columns = columns(
            &base,
            "Base",
            &[
                "Package", "LibPath", "ID", "Name", "Title", "Topic", "Encoding",
            ],
            &["Package", "LibPath", "ID", "Name", "Title", "Topic"],
            &["Package", "LibPath", "ID", "name", "title", "topic"],
            &[
                "Package", "LibPath", "ID", "name", "title", "topic", "Encoding",
            ],
        )?;
        let alias_columns = relation_columns(&aliases, "Aliases", "Alias", "Aliases")?;
        let keyword_columns = relation_columns(&keywords, "Keywords", "Keyword", "Keywords")?;
        let concept_columns = relation_columns(&concepts, "Concepts", "Concept", "Concepts")?;

        let mut base_entries = Vec::with_capacity(base.nrow());
        for row in 0..base.nrow() {
            base_entries.push(HelpSearchBaseEntry {
                package: cell(&base, row, base_columns[0]),
                lib_path: cell(&base, row, base_columns[1]),
                id: cell(&base, row, base_columns[2]),
                name: cell(&base, row, base_columns[3]),
                title: cell(&base, row, base_columns[4]),
                topic: cell(&base, row, base_columns[5]),
                encoding: base_columns
                    .get(6)
                    .map(|&column| cell(&base, row, column))
                    .unwrap_or_else(|| Some(String::new())),
            });
        }

        let mut alias_entries = Vec::with_capacity(aliases.nrow());
        for row in 0..aliases.nrow() {
            alias_entries.push(HelpSearchAliasEntry {
                alias: cell(&aliases, row, alias_columns[0]),
                id: cell(&aliases, row, alias_columns[1]),
                package: cell(&aliases, row, alias_columns[2]),
            });
        }
        let mut keyword_entries = Vec::with_capacity(keywords.nrow());
        for row in 0..keywords.nrow() {
            keyword_entries.push(HelpSearchKeywordEntry {
                keyword: cell(&keywords, row, keyword_columns[0]),
                id: cell(&keywords, row, keyword_columns[1]),
                package: cell(&keywords, row, keyword_columns[2]),
            });
        }
        let mut concept_entries = Vec::with_capacity(concepts.nrow());
        for row in 0..concepts.nrow() {
            concept_entries.push(HelpSearchConceptEntry {
                concept: cell(&concepts, row, concept_columns[0]),
                id: cell(&concepts, row, concept_columns[1]),
                package: cell(&concepts, row, concept_columns[2]),
            });
        }

        Ok(Self {
            base: base_entries,
            aliases: alias_entries,
            keywords: keyword_entries,
            concepts: concept_entries,
        })
    }

    /// Iterates over help-file rows in stored order.
    pub fn base_entries(&self) -> impl ExactSizeIterator<Item = &HelpSearchBaseEntry> {
        self.base.iter()
    }

    /// Iterates over alias rows in stored order.
    pub fn aliases(&self) -> impl ExactSizeIterator<Item = &HelpSearchAliasEntry> {
        self.aliases.iter()
    }

    /// Iterates over keyword rows in stored order.
    pub fn keywords(&self) -> impl ExactSizeIterator<Item = &HelpSearchKeywordEntry> {
        self.keywords.iter()
    }

    /// Iterates over concept rows in stored order.
    pub fn concepts(&self) -> impl ExactSizeIterator<Item = &HelpSearchConceptEntry> {
        self.concepts.iter()
    }

    /// Returns the number of help-file rows.
    pub fn base_len(&self) -> usize {
        self.base.len()
    }

    /// Returns the number of stored alias rows.
    pub fn aliases_len(&self) -> usize {
        self.aliases.len()
    }

    /// Returns the number of stored keyword rows.
    pub fn keywords_len(&self) -> usize {
        self.keywords.len()
    }

    /// Returns the number of stored concept rows.
    pub fn concepts_len(&self) -> usize {
        self.concepts.len()
    }

    /// Returns whether all four matrices contain no rows.
    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
            && self.aliases.is_empty()
            && self.keywords.is_empty()
            && self.concepts.is_empty()
    }
}

impl TryFrom<&RObject> for HelpSearchIndex {
    type Error = Error;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

fn columns(
    matrix: &CharacterMatrix,
    label: &str,
    modern: &[&str],
    modern_without_encoding: &[&str],
    legacy_without_encoding: &[&str],
    legacy: &[&str],
) -> Result<Vec<usize>, Error> {
    let names = matrix_column_positions(matrix, label)?;
    let matched = if names.len() == modern.len() && same_names(&names, modern) {
        modern
    } else if names.len() == modern_without_encoding.len()
        && same_names(&names, modern_without_encoding)
    {
        modern_without_encoding
    } else if names.len() == legacy_without_encoding.len()
        && same_names(&names, legacy_without_encoding)
    {
        legacy_without_encoding
    } else if names.len() == legacy.len() && same_names(&names, legacy) {
        legacy
    } else {
        return Err(malformed(format!(
            "{label} matrix has unsupported columns: {:?}",
            names.keys().collect::<Vec<_>>()
        )));
    };
    Ok(matched
        .iter()
        .filter_map(|name| names.get(*name).copied())
        .collect())
}

fn relation_columns(
    matrix: &CharacterMatrix,
    label: &str,
    current_value: &str,
    legacy_value: &str,
) -> Result<[usize; 3], Error> {
    let names = matrix_column_positions(matrix, label)?;
    let current = [current_value, "ID", "Package"];
    let legacy = [legacy_value, "ID", "Package"];
    let expected = if names.len() == 3 && same_names(&names, &current) {
        current
    } else if names.len() == 3 && same_names(&names, &legacy) {
        legacy
    } else {
        return Err(malformed(format!(
            "{label} matrix has unsupported columns: {:?}",
            names.keys().collect::<Vec<_>>()
        )));
    };
    Ok([names[expected[0]], names[expected[1]], names[expected[2]]])
}

fn matrix_column_positions<'a>(
    matrix: &'a CharacterMatrix,
    label: &str,
) -> Result<BTreeMap<&'a str, usize>, Error> {
    let mut positions = BTreeMap::new();
    for column in 0..matrix.ncol() {
        let Some(name) = matrix.column_name(column) else {
            return Err(malformed(format!(
                "{label} matrix has missing or NA column name at position {column}"
            )));
        };
        if positions.insert(name, column).is_some() {
            return Err(malformed(format!(
                "{label} matrix has duplicate column name {name:?}"
            )));
        }
    }
    Ok(positions)
}

fn same_names(names: &BTreeMap<&str, usize>, expected: &[&str]) -> bool {
    expected.iter().all(|name| names.contains_key(name))
}

fn cell(matrix: &CharacterMatrix, row: usize, column: usize) -> Option<String> {
    matrix.get(row, column).flatten().map(str::to_owned)
}

fn malformed(message: impl Into<String>) -> Error {
    Error::MalformedIndex(format!("invalid Meta/hsearch.rds: {}", message.into()))
}
