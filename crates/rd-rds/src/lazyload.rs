//! Bounded access to an installed package's lazy-load database.
//!
//! A lazy-load database is an `.rdx` index and a sibling `.rdb` file.  The
//! index is an RDS object. Raw records are the XDR bytes addressed by the
//! index; zlib records have a four-byte big-endian uncompressed length before
//! their compressed payload. This module deliberately exposes records through
//! names in the index; it does not expose an arbitrary offset reader.

use std::{
    collections::HashMap,
    fs::{self, File, Metadata},
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::{RObject, RStr, RValue};

const DEFAULT_LIMIT: usize = 256 * 1024 * 1024;

/// Compression used for records in a lazy-load database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Compression {
    None,
    Zlib,
    Bzip2,
    Xz,
}

/// A checked byte range in the data file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordLocation {
    offset: u64,
    length: u64,
}

impl RecordLocation {
    /// Creates a location for an index entry. The database validates the
    /// range against the data file before reading it.
    #[must_use]
    pub fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }

    /// Returns the byte offset.
    #[must_use]
    pub fn offset(self) -> u64 {
        self.offset
    }

    /// Returns the stored byte length.
    #[must_use]
    pub fn length(self) -> u64 {
        self.length
    }
}

/// A reference entry from the `.rdx` `references` map.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordReference {
    Direct(RecordLocation),
    Compound {
        eager_key: Option<RecordLocation>,
        lazy_keys: Vec<(String, RecordLocation)>,
    },
    Unsupported,
}

/// One entry from the `.rdx` `variables` map, retained in index order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    name: String,
    reference: RecordReference,
}

impl Variable {
    /// Returns the stored binding name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the parsed record descriptor.
    #[must_use]
    pub fn reference(&self) -> &RecordReference {
        &self.reference
    }

    /// Returns a direct record location, when this variable has one.
    #[must_use]
    pub fn location(&self) -> Option<RecordLocation> {
        match self.reference {
            RecordReference::Direct(location) => Some(location),
            RecordReference::Compound { .. } | RecordReference::Unsupported => None,
        }
    }
}

/// Bytes read for a variable's direct record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordBytes {
    location: RecordLocation,
    compression: Compression,
    stored: Vec<u8>,
    decompressed: Vec<u8>,
}

impl RecordBytes {
    /// Returns the location recorded in the index.
    #[must_use]
    pub fn location(&self) -> RecordLocation {
        self.location
    }

    /// Returns the compression selected by the index's `compressed` field.
    #[must_use]
    pub fn compression(&self) -> Compression {
        self.compression
    }

    /// The exact bytes addressed by the index. For raw records this is the
    /// decompressed XDR itself; for zlib records it includes the four-byte
    /// declared-length prefix and compressed payload.
    #[must_use]
    pub fn stored_bytes(&self) -> &[u8] {
        &self.stored
    }

    /// The record payload accepted by [`crate::parse`]. Raw records return the
    /// same bytes as [`Self::stored_bytes`]; zlib records remove the framing
    /// prefix and decompress the payload.
    #[must_use]
    pub fn decompressed_bytes(&self) -> &[u8] {
        &self.decompressed
    }
}

/// Limits for opening and reading a lazy-load database.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    max_index_bytes: usize,
    max_stored_record_bytes: usize,
    max_decompressed_record_bytes: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_index_bytes: DEFAULT_LIMIT,
            max_stored_record_bytes: DEFAULT_LIMIT,
            max_decompressed_record_bytes: DEFAULT_LIMIT,
        }
    }
}

impl Options {
    /// Sets the maximum number of bytes read from the index file.
    #[must_use]
    pub fn max_index_bytes(mut self, value: usize) -> Self {
        self.max_index_bytes = value;
        self
    }

    /// Sets the maximum number of bytes read for one stored record.
    #[must_use]
    pub fn max_stored_record_bytes(mut self, value: usize) -> Self {
        self.max_stored_record_bytes = value;
        self
    }

    /// Sets the maximum decompressed size of one record.
    #[must_use]
    pub fn max_decompressed_record_bytes(mut self, value: usize) -> Self {
        self.max_decompressed_record_bytes = value;
        self
    }
}

/// Errors from lazy-load index and record access.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("lazy-load index exceeds the {limit}-byte limit")]
    IndexSizeLimitExceeded { limit: usize },
    #[error("lazy-load index changed while it was being read")]
    IndexChanged,
    #[error("invalid lazy-load index: {message}")]
    InvalidIndex { message: String },
    #[error("invalid variable {name:?}: {message}")]
    InvalidVariable { name: String, message: String },
    #[error("unknown variable {name:?}")]
    UnknownVariable { name: String },
    #[error("unknown persistence reference {name:?}")]
    UnknownReference { name: String },
    #[error("record reference {name:?} does not address a direct record")]
    UnsupportedRecordReference { name: String },
    #[error("record compression {compression:?} is not supported")]
    CompressionUnsupported { compression: Compression },
    #[error("record range ({offset}, {length}) overflows")]
    RecordRangeOverflow { offset: u64, length: u64 },
    #[error("record range ({offset}, {length}) is outside data file of {file_len} bytes")]
    RecordOutOfRange {
        offset: u64,
        length: u64,
        file_len: u64,
    },
    #[error("stored record exceeds the {limit}-byte limit")]
    StoredRecordSizeLimitExceeded { limit: usize },
    #[error("decompressed record exceeds the {limit}-byte limit")]
    DecompressedRecordSizeLimitExceeded { limit: usize },
    #[error("compressed record is shorter than its four-byte length prefix")]
    RecordLengthPrefixMissing,
    #[error("record declares {declared} decompressed bytes but contains {actual}")]
    RecordSizeMismatch { declared: usize, actual: usize },
    #[error("record decompression failed: {message}")]
    Decompression { message: String },
    #[error("record has {trailing} trailing bytes after its compressed stream")]
    TrailingRecordBytes { trailing: usize },
    #[error("data file changed while it was being read")]
    DataFileChanged,
}

/// A reader over an installed package's `.rdx`/`.rdb` pair.
#[derive(Debug)]
pub struct LazyLoadDb {
    data_path: PathBuf,
    compression: Compression,
    variables: Vec<Variable>,
    variable_lookup: HashMap<String, usize>,
    references: Vec<(String, RecordReference)>,
    options: Options,
    data_metadata: FileSnapshot,
}

impl LazyLoadDb {
    /// Opens an index and its sibling data file.
    pub fn open(index_path: impl AsRef<Path>, data_path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_with_options(index_path, data_path, Options::default())
    }

    /// Opens an index with explicit resource limits.
    pub fn open_with_options(
        index_path: impl AsRef<Path>,
        data_path: impl AsRef<Path>,
        options: Options,
    ) -> Result<Self, Error> {
        let index_path = index_path.as_ref();
        let data_path = data_path.as_ref().to_path_buf();
        let index_before = snapshot(index_path)?;
        let mut index_file = File::open(index_path).map_err(|source| Error::Io {
            path: index_path.to_path_buf(),
            source,
        })?;
        let index_file_before =
            snapshot_metadata(&index_file.metadata().map_err(|source| Error::Io {
                path: index_path.to_path_buf(),
                source,
            })?);
        if index_before != index_file_before {
            return Err(Error::IndexChanged);
        }
        let mut index_bytes = Vec::new();
        index_file
            .by_ref()
            .take(options.max_index_bytes.saturating_add(1) as u64)
            .read_to_end(&mut index_bytes)
            .map_err(|source| Error::Io {
                path: index_path.to_path_buf(),
                source,
            })?;
        if index_bytes.len() > options.max_index_bytes {
            return Err(Error::IndexSizeLimitExceeded {
                limit: options.max_index_bytes,
            });
        }
        let index_after = snapshot(index_path)?;
        let index_file_after =
            snapshot_metadata(&index_file.metadata().map_err(|source| Error::Io {
                path: index_path.to_path_buf(),
                source,
            })?);
        if index_before != index_after || index_file_before != index_file_after {
            return Err(Error::IndexChanged);
        }
        let root = crate::file::from_bytes_with_options(
            &index_bytes,
            &crate::file::ReadOptions::default()
                .max_compressed_bytes(options.max_index_bytes)
                .max_decompressed_bytes(options.max_index_bytes),
        )
        .map_err(|error| Error::InvalidIndex {
            message: error.to_string(),
        })?;

        let compression = parse_compression(&root)?;
        let variables = parse_variables(&root)?;
        let references = parse_references(&root)?;
        let variable_lookup = variables
            .iter()
            .enumerate()
            .map(|(index, variable)| (variable.name.clone(), index))
            .collect();
        let data_metadata = snapshot(&data_path)?;

        Ok(Self {
            data_path,
            compression,
            variables,
            variable_lookup,
            references,
            options,
            data_metadata,
        })
    }

    #[must_use]
    pub fn compression(&self) -> Compression {
        self.compression
    }

    /// Returns every stored variable in `.rdx` order, including duplicates.
    #[must_use]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// Returns the last variable with this name, matching the database's
    /// list-to-environment lookup behavior.
    #[must_use]
    pub fn variable(&self, name: &str) -> Option<&Variable> {
        self.variable_lookup
            .get(name)
            .and_then(|index| self.variables.get(*index))
    }

    /// Returns every persistence reference in `.rdx` order.
    #[must_use]
    pub fn references(&self) -> &[(String, RecordReference)] {
        &self.references
    }

    #[must_use]
    pub fn reference(&self, name: &str) -> Option<&RecordReference> {
        self.references
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, reference)| reference)
    }

    /// Reads a direct record addressed by a variable name.
    ///
    /// Variables backed by compound or otherwise unsupported references
    /// return [`Error::UnsupportedRecordReference`].
    pub fn read(&self, name: &str) -> Result<RecordBytes, Error> {
        let variable = self
            .variable(name)
            .ok_or_else(|| Error::UnknownVariable { name: name.into() })?;
        let RecordReference::Direct(location) = variable.reference else {
            return Err(Error::UnsupportedRecordReference { name: name.into() });
        };
        self.read_location(location)
    }

    /// Reads a direct record addressed by a persistence reference name.
    ///
    /// A missing name returns [`Error::UnknownReference`]. References may
    /// describe compound or otherwise unsupported records. In that case this
    /// returns [`Error::UnsupportedRecordReference`], while retaining the
    /// reference in [`Self::references`] for callers that only need to
    /// enumerate the index.
    pub fn read_reference(&self, name: &str) -> Result<RecordBytes, Error> {
        let reference = self
            .reference(name)
            .ok_or_else(|| Error::UnknownReference { name: name.into() })?;
        let RecordReference::Direct(location) = reference else {
            return Err(Error::UnsupportedRecordReference { name: name.into() });
        };
        self.read_location(*location)
    }

    fn read_location(&self, location: RecordLocation) -> Result<RecordBytes, Error> {
        let before = snapshot(&self.data_path)?;
        if self.data_metadata != before {
            return Err(Error::DataFileChanged);
        }
        let end =
            location
                .offset
                .checked_add(location.length)
                .ok_or(Error::RecordRangeOverflow {
                    offset: location.offset,
                    length: location.length,
                })?;
        if end > before.len {
            return Err(Error::RecordOutOfRange {
                offset: location.offset,
                length: location.length,
                file_len: before.len,
            });
        }
        let stored_len =
            usize::try_from(location.length).map_err(|_| Error::StoredRecordSizeLimitExceeded {
                limit: self.options.max_stored_record_bytes,
            })?;
        if stored_len > self.options.max_stored_record_bytes {
            return Err(Error::StoredRecordSizeLimitExceeded {
                limit: self.options.max_stored_record_bytes,
            });
        }
        let mut file = File::open(&self.data_path).map_err(|source| Error::Io {
            path: self.data_path.clone(),
            source,
        })?;
        let file_before = snapshot_metadata(&file.metadata().map_err(|source| Error::Io {
            path: self.data_path.clone(),
            source,
        })?);
        if before != file_before || self.data_metadata != before {
            return Err(Error::DataFileChanged);
        }
        file.seek(SeekFrom::Start(location.offset))
            .and_then(|_| {
                let mut bytes = vec![0; stored_len];
                file.read_exact(&mut bytes).map(|_| bytes)
            })
            .map_err(|source| Error::Io {
                path: self.data_path.clone(),
                source,
            })
            .and_then(|stored| {
                let after = snapshot(&self.data_path)?;
                let file_after =
                    snapshot_metadata(&file.metadata().map_err(|source| Error::Io {
                        path: self.data_path.clone(),
                        source,
                    })?);
                if before != after
                    || self.data_metadata != before
                    || file_before != file_after
                    || after != file_after
                {
                    return Err(Error::DataFileChanged);
                }
                let decompressed = decode_stored_record(&stored, self.compression, self.options)?;
                Ok(RecordBytes {
                    location,
                    compression: self.compression,
                    stored,
                    decompressed,
                })
            })
    }
}

/// Decodes the stored bytes of one lazy-load record.
///
/// This is the low-level container primitive for callers that already know a
/// record's compression mode and have obtained its exact `(offset, length)`
/// slice. It validates the stored and decompressed size limits, the four-byte
/// length prefix, the compressed stream, and trailing bytes. The returned
/// vector is the XDR payload accepted by [`crate::parse`].
pub fn decode_stored_record(
    stored: &[u8],
    compression: Compression,
    options: Options,
) -> Result<Vec<u8>, Error> {
    if stored.len() > options.max_stored_record_bytes {
        return Err(Error::StoredRecordSizeLimitExceeded {
            limit: options.max_stored_record_bytes,
        });
    }
    if compression == Compression::None {
        if stored.len() > options.max_decompressed_record_bytes {
            return Err(Error::DecompressedRecordSizeLimitExceeded {
                limit: options.max_decompressed_record_bytes,
            });
        }
        return Ok(stored.to_vec());
    }
    let declared = u32::from_be_bytes(
        stored
            .get(..4)
            .ok_or(Error::RecordLengthPrefixMissing)?
            .try_into()
            .expect("length checked"),
    ) as usize;
    if declared > options.max_decompressed_record_bytes {
        return Err(Error::DecompressedRecordSizeLimitExceeded {
            limit: options.max_decompressed_record_bytes,
        });
    }
    let payload = &stored[4..];
    let mut decompressed = Vec::with_capacity(declared.min(8192));
    match compression {
        Compression::Zlib => {
            let mut decoder = flate2::read::ZlibDecoder::new(payload);
            decoder
                .by_ref()
                .take(options.max_decompressed_record_bytes.saturating_add(1) as u64)
                .read_to_end(&mut decompressed)
                .map_err(|error| Error::Decompression {
                    message: error.to_string(),
                })?;
            let consumed = decoder.total_in() as usize;
            if consumed != payload.len() {
                return Err(Error::TrailingRecordBytes {
                    trailing: payload.len().saturating_sub(consumed),
                });
            }
        }
        Compression::Bzip2 | Compression::Xz => {
            return Err(Error::CompressionUnsupported { compression });
        }
        Compression::None => unreachable!("raw records return above"),
    }
    if decompressed.len() > options.max_decompressed_record_bytes {
        return Err(Error::DecompressedRecordSizeLimitExceeded {
            limit: options.max_decompressed_record_bytes,
        });
    }
    if decompressed.len() != declared {
        return Err(Error::RecordSizeMismatch {
            declared,
            actual: decompressed.len(),
        });
    }
    Ok(decompressed)
}

fn parse_compression(root: &RObject) -> Result<Compression, Error> {
    let object = named_field(root, "compressed")?;
    let value = match object.value() {
        RValue::Logical(values) if values.len() == 1 => values[0]
            .map(|value| if value { 1 } else { 0 })
            .ok_or_else(|| invalid_index("'compressed' is NA"))?,
        RValue::Integer(values) if values.len() == 1 => {
            values[0].ok_or_else(|| invalid_index("'compressed' is NA"))?
        }
        RValue::Real(values) if values.len() == 1 => {
            let value = values[0].ok_or_else(|| invalid_index("'compressed' is NA"))?;
            if !value.is_finite() || value.fract() != 0.0 {
                return Err(invalid_index("'compressed' is not an integer"));
            }
            i32::try_from(value as i64).map_err(|_| invalid_index("'compressed' overflows"))?
        }
        _ => {
            return Err(invalid_index(
                "'compressed' must be a scalar logical or integer",
            ));
        }
    };
    match value {
        0 => Ok(Compression::None),
        1 => Ok(Compression::Zlib),
        2 => Ok(Compression::Bzip2),
        3 => Ok(Compression::Xz),
        _ => Err(invalid_index("'compressed' must be one of 0, 1, 2, or 3")),
    }
}

fn parse_variables(root: &RObject) -> Result<Vec<Variable>, Error> {
    let object = named_field(root, "variables")?;
    parse_map(object, "variables", true)
}

fn parse_references(root: &RObject) -> Result<Vec<(String, RecordReference)>, Error> {
    let object = named_field(root, "references")?;
    let variables = parse_map(object, "references", false)?;
    Ok(variables
        .into_iter()
        .map(|variable| (variable.name, variable.reference))
        .collect())
}

fn named_field<'a>(root: &'a RObject, field: &str) -> Result<&'a RObject, Error> {
    let RValue::List(items) = root.value() else {
        return Err(invalid_index("index root must be a list"));
    };
    let names = root
        .names()
        .ok_or_else(|| invalid_index("index root has no names attribute"))?;
    if names.len() != items.len() {
        return Err(invalid_index(
            "index root names and values differ in length",
        ));
    }
    let mut found = None;
    for (name, item) in names.iter().zip(items) {
        if string_value(name).map_err(|message| {
            invalid_index(format!(
                "index root contains an invalid field name: {message}"
            ))
        })? == field
        {
            if found.is_some() {
                return Err(invalid_index(format!("index root repeats '{field}'")));
            }
            found = Some(item);
        }
    }
    found.ok_or_else(|| invalid_index(format!("missing '{field}' field")))
}

fn parse_map(object: &RObject, field: &str, strict: bool) -> Result<Vec<Variable>, Error> {
    let RValue::List(items) = object.value() else {
        return Err(invalid_index(format!("'{field}' must be a list")));
    };
    let Some(names) = object.names() else {
        // R serializes an empty list() without a names attribute. This is
        // the normal representation of an empty references map.
        if items.is_empty() {
            return Ok(Vec::new());
        }
        return Err(invalid_index(format!("'{field}' has no names attribute")));
    };
    if names.len() != items.len() {
        return Err(invalid_index(format!(
            "'{field}' has {} names but {} values",
            names.len(),
            items.len()
        )));
    }
    items
        .iter()
        .zip(names)
        .map(|(item, name)| {
            let name = string_value(name).map_err(|message| {
                if strict {
                    Error::InvalidVariable {
                        name: "<invalid name>".into(),
                        message,
                    }
                } else {
                    invalid_index(format!("'{field}' contains an invalid name: {message}"))
                }
            })?;
            let reference = match parse_reference(item) {
                Ok(reference) => reference,
                Err(ReferenceParseError::Unknown) => {
                    if strict {
                        return Err(Error::InvalidVariable {
                            name,
                            message: "expected a numeric offset/length pair".into(),
                        });
                    }
                    RecordReference::Unsupported
                }
                Err(ReferenceParseError::Malformed(message)) => {
                    if strict {
                        return Err(Error::InvalidVariable { name, message });
                    }
                    return Err(invalid_index(format!(
                        "'{field}' entry is malformed: {message}"
                    )));
                }
            };
            Ok(Variable { name, reference })
        })
        .collect()
}

#[derive(Debug)]
enum ReferenceParseError {
    Unknown,
    Malformed(String),
}

fn parse_reference(object: &RObject) -> Result<RecordReference, ReferenceParseError> {
    if matches!(object.value(), RValue::Integer(_) | RValue::Real(_)) {
        return parse_location(object)
            .map_err(ReferenceParseError::Malformed)
            .and_then(|location| {
                location
                    .map(RecordReference::Direct)
                    .ok_or_else(|| ReferenceParseError::Malformed("missing location".into()))
            });
    }
    let RValue::List(items) = object.value() else {
        return Err(ReferenceParseError::Unknown);
    };
    let Some(names) = object.names() else {
        return Err(ReferenceParseError::Unknown);
    };
    let mut eager_key = None;
    let mut eager_seen = false;
    let mut lazy_keys = Vec::new();
    let mut lazy_seen = false;
    if names.len() != items.len() {
        return Err(ReferenceParseError::Malformed(
            "compound descriptor names and values differ in length".into(),
        ));
    }
    for (name, item) in names.iter().zip(items) {
        let field = string_value(name).map_err(ReferenceParseError::Malformed)?;
        match field.as_str() {
            "eagerKey" => {
                if eager_seen {
                    return Err(ReferenceParseError::Malformed(
                        "compound descriptor repeats eagerKey".into(),
                    ));
                }
                eager_seen = true;
                eager_key =
                    Some(parse_location_required(item).map_err(ReferenceParseError::Malformed)?);
            }
            "lazyKeys" => {
                if lazy_seen {
                    return Err(ReferenceParseError::Malformed(
                        "compound descriptor repeats lazyKeys".into(),
                    ));
                }
                lazy_seen = true;
                lazy_keys = parse_lazy_keys(item).map_err(ReferenceParseError::Malformed)?;
            }
            _ => continue,
        }
    }
    if eager_seen || lazy_seen {
        Ok(RecordReference::Compound {
            eager_key,
            lazy_keys,
        })
    } else {
        Err(ReferenceParseError::Unknown)
    }
}

fn parse_location_required(object: &RObject) -> Result<RecordLocation, String> {
    parse_location(object)?.ok_or_else(|| "expected an offset/length pair".into())
}

fn parse_lazy_keys(object: &RObject) -> Result<Vec<(String, RecordLocation)>, String> {
    let RValue::List(items) = object.value() else {
        return Err("lazyKeys must be a named list".into());
    };
    let names = object
        .names()
        .ok_or_else(|| "lazyKeys has no names attribute".to_string())?;
    if names.len() != items.len() {
        return Err("lazyKeys names and values differ in length".into());
    }
    let mut result = Vec::with_capacity(items.len());
    for (name, item) in names.iter().zip(items) {
        let name = string_value(name)?;
        if result.iter().any(|(existing, _)| existing == &name) {
            return Err("lazyKeys contains duplicate names".into());
        }
        result.push((name, parse_location_required(item)?));
    }
    Ok(result)
}

fn parse_location(object: &RObject) -> Result<Option<RecordLocation>, String> {
    let values: Vec<f64> = match object.value() {
        RValue::Integer(values) => values
            .iter()
            .map(|value| {
                value
                    .map(f64::from)
                    .ok_or_else(|| "NA location".to_string())
            })
            .collect::<Result<_, _>>()?,
        RValue::Real(values) => values
            .iter()
            .map(|value| value.ok_or_else(|| "NA location".to_string()))
            .collect::<Result<_, _>>()?,
        _ => return Ok(None),
    };
    if values.len() != 2 {
        return Ok(None);
    }
    let convert = |value: f64| -> Result<u64, String> {
        if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
            return Err("location is not a non-negative integer".into());
        }
        // IEEE-754 doubles cannot represent every integer at or above 2^53.
        // Rejecting that range avoids accepting a rounded wire value (and is
        // stricter than the platform's usize conversion requirements).
        if value >= 9_007_199_254_740_992.0 {
            return Err("location exceeds the exactly representable integer range".into());
        }
        Ok(value as u64)
    };
    Ok(Some(RecordLocation::new(
        convert(values[0])?,
        convert(values[1])?,
    )))
}

fn string_value(value: &RStr) -> Result<String, String> {
    value
        .as_str()
        .ok_or_else(|| "string is NA".to_string())?
        .map(|value| value.into_owned())
        .map_err(|error| error.to_string())
}

fn invalid_index(message: impl Into<String>) -> Error {
    Error::InvalidIndex {
        message: message.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSnapshot {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

fn snapshot(path: &Path) -> Result<FileSnapshot, Error> {
    let metadata = fs::metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(snapshot_metadata(&metadata))
}

fn snapshot_metadata(metadata: &Metadata) -> FileSnapshot {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileSnapshot {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
    #[cfg(not(unix))]
    FileSnapshot {
        len: metadata.len(),
        modified: metadata.modified().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attribute, Attributes, NativeEncodingSource, REncoding, Symbol};

    fn string(value: &str) -> RObject {
        RObject::from_parts(
            RValue::Character(vec![RStr::new(
                value.as_bytes(),
                REncoding::Native,
                NativeEncodingSource::AssumedUtf8,
            )]),
            Attributes::default(),
        )
    }

    fn strings(values: &[&str]) -> RObject {
        RObject::from_parts(
            RValue::Character(
                values
                    .iter()
                    .map(|value| {
                        RStr::new(
                            value.as_bytes(),
                            REncoding::Native,
                            NativeEncodingSource::AssumedUtf8,
                        )
                    })
                    .collect(),
            ),
            Attributes::default(),
        )
    }

    fn named_list(names: &[&str], values: Vec<RObject>) -> RObject {
        RObject::from_parts(
            RValue::List(values),
            Attributes::new(vec![Attribute::new(Symbol::from("names"), strings(names))]),
        )
    }

    #[test]
    fn parses_compression_codes() {
        for (value, expected) in [
            (RValue::Logical(vec![Some(false)]), Compression::None),
            (RValue::Logical(vec![Some(true)]), Compression::Zlib),
            (RValue::Integer(vec![Some(2)]), Compression::Bzip2),
            (RValue::Integer(vec![Some(3)]), Compression::Xz),
        ] {
            let root = named_list(
                &["compressed"],
                vec![RObject::from_parts(value, Attributes::default())],
            );
            assert_eq!(parse_compression(&root).unwrap(), expected);
        }
    }

    #[test]
    fn rejects_duplicate_known_top_level_fields_and_missing_references() {
        let compressed =
            RObject::from_parts(RValue::Logical(vec![Some(false)]), Attributes::default());
        let duplicate = named_list(
            &["compressed", "compressed"],
            vec![compressed.clone(), compressed],
        );
        assert!(matches!(
            parse_compression(&duplicate),
            Err(Error::InvalidIndex { .. })
        ));
        let no_references = named_list(
            &["variables", "compressed"],
            vec![
                named_list(&[], vec![]),
                RObject::from_parts(RValue::Logical(vec![Some(false)]), Attributes::default()),
            ],
        );
        assert!(matches!(
            parse_references(&no_references),
            Err(Error::InvalidIndex { .. })
        ));
    }

    #[test]
    fn preserves_variables_and_uses_last_duplicate() {
        let direct = |offset, length| {
            RObject::from_parts(
                RValue::Integer(vec![Some(offset), Some(length)]),
                Attributes::default(),
            )
        };
        let root = named_list(
            &["variables"],
            vec![named_list(
                &["same", "other", "same"],
                vec![direct(1, 2), direct(3, 4), direct(5, 6)],
            )],
        );
        let variables = parse_variables(&root).unwrap();
        assert_eq!(variables.len(), 3);
        assert_eq!(variables[0].location(), Some(RecordLocation::new(1, 2)));
        assert_eq!(variables[2].location(), Some(RecordLocation::new(5, 6)));
    }

    #[test]
    fn decodes_raw_record_and_checks_declared_length() {
        let payload = b"X\nexample";
        let decoded = decode_stored_record(payload, Compression::None, Options::default()).unwrap();
        assert_eq!(decoded, payload);
    }

    #[cfg(feature = "lazyload")]
    #[test]
    fn decodes_zlib_record_and_rejects_trailing_bytes() {
        use flate2::{Compression as FlateCompression, write::ZlibEncoder};
        use std::io::Write;

        let payload = b"X\nzlib";
        let mut encoder = ZlibEncoder::new(Vec::new(), FlateCompression::default());
        encoder.write_all(payload).unwrap();
        let mut stored = (payload.len() as u32).to_be_bytes().to_vec();
        stored.extend(encoder.finish().unwrap());
        let decoded = decode_stored_record(&stored, Compression::Zlib, Options::default())
            .expect("zlib record");
        assert_eq!(decoded, payload);

        stored.push(0);
        assert!(matches!(
            decode_stored_record(&stored, Compression::Zlib, Options::default()),
            Err(Error::TrailingRecordBytes { .. })
        ));
    }

    #[test]
    fn rejects_short_corrupt_and_mismatched_compressed_records() {
        assert!(matches!(
            decode_stored_record(&[0, 0, 0], Compression::Zlib, Options::default()),
            Err(Error::RecordLengthPrefixMissing)
        ));
        assert!(matches!(
            decode_stored_record(
                &[0, 0, 0, 10, 1, 2, 3],
                Compression::Zlib,
                Options::default()
            ),
            Err(Error::Decompression { .. })
        ));
        let payload = b"X\nmismatch";
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        use std::io::Write;
        encoder.write_all(payload).unwrap();
        let mut stored = (0_u32).to_be_bytes().to_vec();
        stored.extend(encoder.finish().unwrap());
        assert!(matches!(
            decode_stored_record(&stored, Compression::Zlib, Options::default()),
            Err(Error::RecordSizeMismatch { .. })
        ));
    }

    #[test]
    fn unknown_reference_shape_is_supported_as_an_entry() {
        let root = named_list(
            &["references"],
            vec![named_list(&["ref"], vec![string("unknown")])],
        );
        let references = parse_references(&root).unwrap();
        assert_eq!(references[0].1, RecordReference::Unsupported);
    }

    #[test]
    fn distinguishes_unknown_and_malformed_reference_shapes() {
        let unknown = named_list(
            &["references"],
            vec![named_list(&["ref"], vec![string("future")])],
        );
        assert!(parse_references(&unknown).is_ok());

        let malformed_direct = named_list(
            &["references"],
            vec![named_list(
                &["ref"],
                vec![RObject::from_parts(
                    RValue::Integer(vec![Some(1), None]),
                    Attributes::default(),
                )],
            )],
        );
        assert!(matches!(
            parse_references(&malformed_direct),
            Err(Error::InvalidIndex { .. })
        ));

        let malformed_variable = named_list(
            &["variables"],
            vec![named_list(
                &["ref"],
                vec![RObject::from_parts(
                    RValue::Integer(vec![Some(1), None]),
                    Attributes::default(),
                )],
            )],
        );
        assert!(matches!(
            parse_variables(&malformed_variable),
            Err(Error::InvalidVariable { .. })
        ));

        let location = || {
            RObject::from_parts(
                RValue::Integer(vec![Some(1), Some(2)]),
                Attributes::default(),
            )
        };
        let malformed_compound = named_list(
            &["references"],
            vec![named_list(
                &["ref"],
                vec![named_list(
                    &["eagerKey", "eagerKey"],
                    vec![location(), location()],
                )],
            )],
        );
        assert!(matches!(
            parse_references(&malformed_compound),
            Err(Error::InvalidIndex { .. })
        ));
    }

    #[test]
    fn parses_compound_reference_locations() {
        let location = |offset, length| {
            RObject::from_parts(
                RValue::Integer(vec![Some(offset), Some(length)]),
                Attributes::default(),
            )
        };
        let descriptor = named_list(
            &["eagerKey", "lazyKeys"],
            vec![
                location(10, 20),
                named_list(&["one", "two"], vec![location(30, 40), location(50, 60)]),
            ],
        );
        assert_eq!(
            parse_reference(&descriptor).unwrap(),
            RecordReference::Compound {
                eager_key: Some(RecordLocation::new(10, 20)),
                lazy_keys: vec![
                    ("one".into(), RecordLocation::new(30, 40)),
                    ("two".into(), RecordLocation::new(50, 60)),
                ],
            }
        );
    }
}
