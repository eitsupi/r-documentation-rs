//! Typed access to declarations stored in `Meta/nsInfo.rds`.
//!
//! These declarations describe namespace metadata written from a package's
//! `NAMESPACE`; they are not a runtime export set, a set of stored lazy-load
//! bindings, or the result of evaluating export patterns. In particular, this
//! view does not resolve regular expressions, synthesize S4 exports into
//! regular exports, or inspect `.onLoad` behavior.

use std::fmt;

use super::{
    ValueKindName, ViewError, decode_optional, decode_required, expect_list, invalid_dimensions,
    named_values, unexpected_length, unexpected_type,
};
use crate::{RObject, RStr, RValue};

/// The state of one known namespace metadata field.
///
/// `Missing` means that the field was absent from the metadata object.
/// `Present` includes an empty collection. `Invalid` means the field belongs
/// to the supported schema but contains invalid data. `UnsupportedSchema`
/// means that the shape was understood but is outside the supported profile.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MetadataField<T> {
    Missing,
    Present(T),
    Invalid(ViewError),
    UnsupportedSchema { description: String },
}

impl<T> MetadataField<T> {
    /// Borrows a present value while preserving the field state.
    pub fn as_ref(&self) -> MetadataField<&T> {
        match self {
            Self::Missing => MetadataField::Missing,
            Self::Present(value) => MetadataField::Present(value),
            Self::Invalid(error) => MetadataField::Invalid(error.clone()),
            Self::UnsupportedSchema { description } => MetadataField::UnsupportedSchema {
                description: description.clone(),
            },
        }
    }

    /// Returns the owned value when the field is present and valid.
    pub fn present(&self) -> Option<&T> {
        match self {
            Self::Present(value) => Some(value),
            Self::Missing | Self::Invalid(_) | Self::UnsupportedSchema { .. } => None,
        }
    }
}

/// One imported binding, retaining the source name and local alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedName {
    source_name: String,
    local_name: String,
}

impl ImportedName {
    /// Creates an imported source/local name pair.
    pub fn new(source_name: impl Into<String>, local_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            local_name: local_name.into(),
        }
    }

    /// Returns the name exported by the source package.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Returns the name bound in the importing package.
    pub fn local_name(&self) -> &str {
        &self.local_name
    }
}

/// One declaration from a namespace `import` or `importFrom` directive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceImport {
    /// Import all exports from `package`, except the listed names.
    All {
        package: String,
        except: Vec<String>,
    },
    /// Import selected names from `package`, retaining local aliases.
    From {
        package: String,
        names: Vec<ImportedName>,
    },
}

impl NamespaceImport {
    /// Returns the source package name.
    pub fn package(&self) -> &str {
        match self {
            Self::All { package, .. } | Self::From { package, .. } => package,
        }
    }
}

/// Whether an S3 registration has an explicit function name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum S3MethodName {
    /// The registration has an `NA` function column.
    Implicit,
    /// The registration names a function explicitly.
    Explicit(String),
}

/// One row from the supported four-column S3 registration matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Registration {
    generic: String,
    class: String,
    method: S3MethodName,
    generic_package: Option<String>,
}

impl S3Registration {
    pub fn generic(&self) -> &str {
        &self.generic
    }

    pub fn class(&self) -> &str {
        &self.class
    }

    pub fn method(&self) -> &S3MethodName {
        &self.method
    }

    pub fn generic_package(&self) -> Option<&str> {
        self.generic_package.as_deref()
    }
}

/// Owned typed declarations from an installed package's `Meta/nsInfo.rds`.
///
/// The fields are independent: an invalid S3 matrix does not prevent valid
/// declared exports from being read. The declarations are retained in input
/// order and duplicates are not removed. They describe static namespace
/// declarations only; they do not promise runtime exports, stored bindings,
/// regex evaluation, re-exports, or `.onLoad` additions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceMetadata {
    declared_exports: MetadataField<Vec<String>>,
    export_patterns: MetadataField<Vec<String>>,
    imports: MetadataField<Vec<NamespaceImport>>,
    s3_registrations: MetadataField<Vec<S3Registration>>,
    s3_generic_evidence: MetadataField<Vec<String>>,
    export_classes: MetadataField<Vec<String>>,
    export_methods: MetadataField<Vec<String>>,
    export_class_patterns: MetadataField<Vec<String>>,
}

impl NamespaceMetadata {
    /// Parses an owned view of a named `Meta/nsInfo.rds` list.
    ///
    /// The root must be a named list. Unknown fields are ignored. Duplicate
    /// known fields make only that field `Invalid`, allowing other fields to
    /// remain useful to callers.
    pub fn from_object(object: &RObject) -> Result<Self, ViewError> {
        let items = expect_list(object, "NamespaceMetadata", None)?;
        let names = named_values(object, "NamespaceMetadata", None)?;
        if names.len() != items.len() {
            return Err(unexpected_length(
                "NamespaceMetadata",
                None,
                items.len().to_string(),
                names.len(),
            ));
        }

        let mut fields = Vec::with_capacity(items.len());
        for (index, name) in names.iter().enumerate() {
            let name = decode_required(name, &format!("NamespaceMetadata[{index}]"), None)?;
            fields.push((name, &items[index]));
        }

        Ok(Self {
            declared_exports: parse_known_field(&fields, "exports", parse_strings),
            export_patterns: parse_known_field(&fields, "exportPatterns", parse_strings),
            imports: parse_known_field(&fields, "imports", parse_imports),
            s3_registrations: parse_known_field(&fields, "S3methods", parse_s3_registrations),
            s3_generic_evidence: parse_known_field(&fields, "S3methods", parse_s3_generic_evidence),
            export_classes: parse_known_field(&fields, "exportClasses", parse_strings),
            export_methods: parse_known_field(&fields, "exportMethods", parse_strings),
            export_class_patterns: parse_known_field(&fields, "exportClassPatterns", parse_strings),
        })
    }

    pub fn declared_exports(&self) -> &MetadataField<Vec<String>> {
        &self.declared_exports
    }

    pub fn export_patterns(&self) -> &MetadataField<Vec<String>> {
        &self.export_patterns
    }

    pub fn imports(&self) -> &MetadataField<Vec<NamespaceImport>> {
        &self.imports
    }

    pub fn s3_registrations(&self) -> &MetadataField<Vec<S3Registration>> {
        &self.s3_registrations
    }

    pub fn s3_generic_evidence(&self) -> &MetadataField<Vec<String>> {
        &self.s3_generic_evidence
    }

    pub fn export_classes(&self) -> &MetadataField<Vec<String>> {
        &self.export_classes
    }

    pub fn export_methods(&self) -> &MetadataField<Vec<String>> {
        &self.export_methods
    }

    pub fn export_class_patterns(&self) -> &MetadataField<Vec<String>> {
        &self.export_class_patterns
    }
}

impl TryFrom<&RObject> for NamespaceMetadata {
    type Error = ViewError;

    fn try_from(value: &RObject) -> Result<Self, Self::Error> {
        Self::from_object(value)
    }
}

enum ParseFailure {
    Invalid(ViewError),
    Unsupported(String),
}

fn parse_known_field<T, F>(fields: &[(String, &RObject)], name: &str, parser: F) -> MetadataField<T>
where
    F: FnOnce(&RObject, &str) -> Result<T, ParseFailure>,
{
    let mut matches = fields.iter().filter(|(field, _)| field == name);
    let Some((_, object)) = matches.next() else {
        return MetadataField::Missing;
    };
    if matches.next().is_some() {
        return MetadataField::Invalid(super::duplicate(
            &format!("NamespaceMetadata.{name}"),
            Some(name.to_owned()),
        ));
    }
    match parser(object, &format!("NamespaceMetadata.{name}")) {
        Ok(value) => MetadataField::Present(value),
        Err(ParseFailure::Invalid(error)) => MetadataField::Invalid(error),
        Err(ParseFailure::Unsupported(description)) => {
            MetadataField::UnsupportedSchema { description }
        }
    }
}

fn parse_strings(object: &RObject, path: &str) -> Result<Vec<String>, ParseFailure> {
    let values = match object.value() {
        RValue::Character(values) => values,
        value => {
            return Err(ParseFailure::Invalid(unexpected_type(
                path,
                None,
                "character vector",
                value.kind_name(),
            )));
        }
    };
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            decode_required(value, &format!("{path}[{index}]"), None).map_err(ParseFailure::Invalid)
        })
        .collect()
}

fn parse_imports(object: &RObject, path: &str) -> Result<Vec<NamespaceImport>, ParseFailure> {
    match object.value() {
        RValue::Character(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let package = decode_required(value, &format!("{path}[{index}]"), None)
                    .map_err(ParseFailure::Invalid)?;
                Ok(NamespaceImport::All {
                    package,
                    except: Vec::new(),
                })
            })
            .collect(),
        RValue::List(values) => values
            .iter()
            .enumerate()
            .map(|(index, value)| parse_import_entry(value, &format!("{path}[{index}]")))
            .collect(),
        value => Err(ParseFailure::Invalid(unexpected_type(
            path,
            None,
            "character vector or list",
            value.kind_name(),
        ))),
    }
}

fn parse_import_entry(object: &RObject, path: &str) -> Result<NamespaceImport, ParseFailure> {
    if let RValue::Character(values) = object.value() {
        if values.len() != 1 {
            return Err(ParseFailure::Invalid(unexpected_length(
                path,
                None,
                "1".to_owned(),
                values.len(),
            )));
        }
        return Ok(NamespaceImport::All {
            package: decode_required(&values[0], path, None).map_err(ParseFailure::Invalid)?,
            except: Vec::new(),
        });
    }

    let RValue::List(values) = object.value() else {
        return Err(ParseFailure::Unsupported(format!(
            "{path} is not a supported import declaration"
        )));
    };
    let names = match object.attributes().get("names") {
        Some(attribute) => match &attribute.value() {
            RValue::Character(names) => Some(names),
            value => {
                return Err(ParseFailure::Invalid(unexpected_type(
                    &format!("{path}.names"),
                    None,
                    "character vector",
                    value.kind_name(),
                )));
            }
        },
        None => None,
    };
    let Some(names) = names else {
        // The only observed unnamed shape is list(package, selections).
        if values.len() != 2 {
            return Err(ParseFailure::Unsupported(format!(
                "{path} does not contain a supported positional import declaration"
            )));
        }
        let package = parse_package_scalar(&values[0], &format!("{path}[0]"))?;
        let selections = parse_imported_names(&values[1], &format!("{path}[1]"))?;
        return Ok(NamespaceImport::From {
            package,
            names: selections,
        });
    };

    if names.len() != values.len() {
        return Err(ParseFailure::Invalid(unexpected_length(
            &format!("{path}.names"),
            None,
            values.len().to_string(),
            names.len(),
        )));
    }

    // A named entry must be interpreted entirely by its names. In particular,
    // an unknown second name must not silently turn a two-element list into
    // the positional package/selections shape.
    let mut decoded_names = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        let decoded = decode_required(name, &format!("{path}.names[{index}]"), None)
            .map_err(ParseFailure::Invalid)?;
        if !matches!(
            decoded.as_str(),
            "package" | "except" | "selections" | "names"
        ) {
            return Err(ParseFailure::Unsupported(format!(
                "{path} contains unsupported import field {decoded:?}"
            )));
        }
        if decoded_names.iter().any(|existing| existing == &decoded) {
            return Err(ParseFailure::Invalid(super::duplicate(
                &format!("{path}.names"),
                Some(decoded),
            )));
        }
        decoded_names.push(decoded);
    }

    let package_index = decoded_names.iter().position(|name| name == "package");
    let Some(package_index) = package_index else {
        return Err(ParseFailure::Unsupported(format!(
            "{path} does not contain a package entry"
        )));
    };
    let package =
        parse_package_scalar(&values[package_index], &format!("{path}[{package_index}]"))?;

    let except_index = decoded_names.iter().position(|name| name == "except");
    let selection_indices: Vec<_> = decoded_names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            matches!(name.as_str(), "selections" | "names").then_some(index)
        })
        .collect();
    if selection_indices.len() > 1 {
        return Err(ParseFailure::Invalid(unexpected_type(
            &format!("{path}.names"),
            None,
            "one import selection field",
            "multiple import selection fields",
        )));
    }
    if except_index.is_some() && !selection_indices.is_empty() {
        return Err(ParseFailure::Invalid(unexpected_type(
            &format!("{path}.names"),
            None,
            "either except or selections/names",
            "both except and selections/names",
        )));
    }
    if let Some(except_index) = except_index {
        let except =
            parse_string_vector(&values[except_index], &format!("{path}[{except_index}]"))?;
        return Ok(NamespaceImport::All { package, except });
    }

    let Some(selection_index) = selection_indices.first().copied() else {
        return Err(ParseFailure::Unsupported(format!(
            "{path} does not contain import selections or except"
        )));
    };
    let selections = parse_imported_names(
        &values[selection_index],
        &format!("{path}[{selection_index}]"),
    )?;
    Ok(NamespaceImport::From {
        package,
        names: selections,
    })
}

fn parse_package_scalar(object: &RObject, path: &str) -> Result<String, ParseFailure> {
    let RValue::Character(values) = object.value() else {
        return Err(ParseFailure::Invalid(unexpected_type(
            path,
            None,
            "character scalar",
            object.value().kind_name(),
        )));
    };
    if values.len() != 1 {
        return Err(ParseFailure::Invalid(unexpected_length(
            path,
            None,
            "1".to_owned(),
            values.len(),
        )));
    }
    decode_required(&values[0], path, None).map_err(ParseFailure::Invalid)
}

fn parse_string_vector(object: &RObject, path: &str) -> Result<Vec<String>, ParseFailure> {
    parse_strings(object, path)
}

fn parse_imported_names(object: &RObject, path: &str) -> Result<Vec<ImportedName>, ParseFailure> {
    let RValue::Character(values) = object.value() else {
        return Err(ParseFailure::Invalid(unexpected_type(
            path,
            None,
            "character vector",
            object.value().kind_name(),
        )));
    };
    let aliases = match object.attributes().get("names") {
        None => None,
        Some(attribute) => match &attribute.value() {
            RValue::Character(values) => Some(values),
            value => {
                return Err(ParseFailure::Invalid(unexpected_type(
                    &format!("{path}.names"),
                    None,
                    "character vector",
                    value.kind_name(),
                )));
            }
        },
    };
    if let Some(aliases) = aliases
        && aliases.len() != values.len()
    {
        return Err(ParseFailure::Invalid(unexpected_length(
            &format!("{path}.names"),
            None,
            values.len().to_string(),
            aliases.len(),
        )));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let source_name = decode_required(value, &format!("{path}[{index}]"), None)
                .map_err(ParseFailure::Invalid)?;
            let local_name = match aliases {
                Some(aliases) => {
                    decode_required(&aliases[index], &format!("{path}.names[{index}]"), None)
                        .map_err(ParseFailure::Invalid)?
                }
                None => source_name.clone(),
            };
            Ok(ImportedName {
                source_name,
                local_name,
            })
        })
        .collect()
}

fn parse_s3_registrations(
    object: &RObject,
    path: &str,
) -> Result<Vec<S3Registration>, ParseFailure> {
    let matrix = parse_character_matrix(object, path)?;
    if matrix.ncol == 0 {
        return Err(ParseFailure::Invalid(invalid_dimensions(
            path,
            "S3 registration matrix must have at least one column",
        )));
    }
    if matrix.ncol != 4 {
        return Err(ParseFailure::Unsupported(format!(
            "S3 registration matrix has {} columns; only the four-column schema is supported",
            matrix.ncol
        )));
    }
    (0..matrix.nrow)
        .map(|row| {
            let generic = matrix.required(row, 0, path)?;
            let class = matrix.required(row, 1, path)?;
            let method = match matrix.optional(row, 2, path)? {
                Some(value) => S3MethodName::Explicit(value),
                None => S3MethodName::Implicit,
            };
            let generic_package = matrix.optional(row, 3, path)?;
            Ok(S3Registration {
                generic,
                class,
                method,
                generic_package,
            })
        })
        .collect()
}

fn parse_s3_generic_evidence(object: &RObject, path: &str) -> Result<Vec<String>, ParseFailure> {
    let matrix = parse_character_matrix(object, path)?;
    if matrix.ncol == 0 {
        return Err(ParseFailure::Invalid(invalid_dimensions(
            path,
            "S3 metadata matrix must have at least one column",
        )));
    }
    (0..matrix.nrow)
        .map(|row| matrix.required(row, 0, path))
        .collect()
}

struct CharacterMatrix<'a> {
    values: &'a [RStr],
    nrow: usize,
    ncol: usize,
}

impl CharacterMatrix<'_> {
    fn at(&self, row: usize, column: usize) -> &RStr {
        &self.values[row + column * self.nrow]
    }

    fn required(&self, row: usize, column: usize, path: &str) -> Result<String, ParseFailure> {
        decode_required(
            self.at(row, column),
            &format!("{path}[row={row},column={column}]"),
            None,
        )
        .map_err(ParseFailure::Invalid)
    }

    fn optional(
        &self,
        row: usize,
        column: usize,
        path: &str,
    ) -> Result<Option<String>, ParseFailure> {
        decode_optional(
            self.at(row, column),
            &format!("{path}[row={row},column={column}]"),
            None,
        )
        .map_err(ParseFailure::Invalid)
    }
}

fn parse_character_matrix<'a>(
    object: &'a RObject,
    path: &str,
) -> Result<CharacterMatrix<'a>, ParseFailure> {
    let values = match object.value() {
        RValue::Character(values) => values,
        value => {
            return Err(ParseFailure::Invalid(unexpected_type(
                path,
                None,
                "character matrix",
                value.kind_name(),
            )));
        }
    };
    let dimensions = object.attributes().get("dim").ok_or_else(|| {
        ParseFailure::Invalid(super::missing(format!("{path}.attributes.dim"), None))
    })?;
    let RValue::Integer(dimensions) = dimensions.value() else {
        return Err(ParseFailure::Invalid(unexpected_type(
            &format!("{path}.attributes.dim"),
            None,
            "integer vector",
            dimensions.value().kind_name(),
        )));
    };
    if dimensions.len() != 2 {
        return Err(ParseFailure::Invalid(unexpected_length(
            &format!("{path}.attributes.dim"),
            None,
            "2".to_owned(),
            dimensions.len(),
        )));
    }
    let mut shape = [0usize; 2];
    for (index, value) in dimensions.iter().enumerate() {
        let Some(value) = value else {
            return Err(ParseFailure::Invalid(invalid_dimensions(
                &format!("{path}.attributes.dim"),
                "dimensions must not contain NA",
            )));
        };
        if *value < 0 {
            return Err(ParseFailure::Invalid(invalid_dimensions(
                &format!("{path}.attributes.dim"),
                "dimensions must not be negative",
            )));
        }
        shape[index] = *value as usize;
    }
    let expected = shape[0].checked_mul(shape[1]).ok_or_else(|| {
        ParseFailure::Invalid(invalid_dimensions(
            &format!("{path}.attributes.dim"),
            "dimension product overflows",
        ))
    })?;
    if expected != values.len() {
        return Err(ParseFailure::Invalid(unexpected_length(
            path,
            None,
            expected.to_string(),
            values.len(),
        )));
    }
    Ok(CharacterMatrix {
        values,
        nrow: shape[0],
        ncol: shape[1],
    })
}

impl fmt::Display for S3MethodName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implicit => formatter.write_str("implicit"),
            Self::Explicit(value) => formatter.write_str(value),
        }
    }
}
