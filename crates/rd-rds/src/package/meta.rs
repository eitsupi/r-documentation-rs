use super::{
    ValueKindName, ViewError, decode_optional, decode_required, duplicate, expect_list, missing,
    named_values, unexpected_length, unexpected_type,
};
use crate::{RObject, RValue};
use std::{collections::BTreeMap, fmt};

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
    pub(super) components: Vec<u32>,
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

fn invalid_version(path: &str, reason: &str) -> ViewError {
    ViewError::InvalidPackageVersion {
        path: path.to_owned(),
        field: Some("R".to_owned()),
        reason: reason.to_owned(),
    }
}
