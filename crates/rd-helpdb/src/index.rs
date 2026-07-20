//! Parsing of the `.rdx` index: the `variables` and `references` named
//! lists that map topic/persistence keys to `(offset, length)` records in
//! the sibling `.rdb` file.

use std::collections::HashMap;

use rd_rds::{RObject, RValue};

use crate::{Error, util::rstr_to_string};

/// A `(offset, length)` pointer into a `.rdb` file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Record {
    pub(crate) offset: usize,
    pub(crate) length: usize,
}

/// The decoded `.rdx` index: `variables` (topic name -> record) and
/// `references` (persistence key, e.g. `"env::0"` -> record), both kept in
/// their original on-disk order.
#[derive(Debug)]
pub(crate) struct Index {
    variables: Vec<(String, Record)>,
    variable_lookup: HashMap<String, usize>,
    references: Vec<(String, Record)>,
    reference_lookup: HashMap<String, usize>,
}

impl Index {
    /// Builds an [`Index`] from the parsed `.rdx` root object, a named list
    /// with (at least) `variables`, `references`, and `compressed` entries.
    pub(crate) fn from_rdx(root: &RObject) -> Result<Self, Error> {
        let variables_obj = root
            .get_named("variables")
            .ok_or_else(|| Error::MalformedIndex("'.rdx' is missing a 'variables' field".into()))?;
        let references_obj = root.get_named("references").ok_or_else(|| {
            Error::MalformedIndex("'.rdx' is missing a 'references' field".into())
        })?;

        let variables = parse_record_map(variables_obj, "variables")?;
        let references = parse_record_map(references_obj, "references")?;
        let variable_lookup = build_lookup(&variables);
        let reference_lookup = build_lookup(&references);

        Ok(Self {
            variables,
            variable_lookup,
            references,
            reference_lookup,
        })
    }

    pub(crate) fn topics(&self) -> impl Iterator<Item = &str> {
        self.variables.iter().map(|(name, _)| name.as_str())
    }

    pub(crate) fn reference_keys(&self) -> impl Iterator<Item = &str> {
        self.references.iter().map(|(name, _)| name.as_str())
    }

    pub(crate) fn variable(&self, topic: &str) -> Option<Record> {
        self.variable_lookup
            .get(topic)
            .map(|&index| self.variables[index].1)
    }

    pub(crate) fn reference(&self, key: &str) -> Option<Record> {
        self.reference_lookup
            .get(key)
            .map(|&index| self.references[index].1)
    }
}

fn build_lookup(entries: &[(String, Record)]) -> HashMap<String, usize> {
    entries
        .iter()
        .enumerate()
        .map(|(index, (name, _))| (name.clone(), index))
        .collect()
}

/// Parses a named-list `.rdx` field (`variables` or `references`) into an
/// ordered `(key, Record)` list. `field_name` is only used for error
/// messages.
fn parse_record_map(list_obj: &RObject, field_name: &str) -> Result<Vec<(String, Record)>, Error> {
    let RValue::List(items) = &list_obj.value() else {
        return Err(Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' is not a list"
        )));
    };
    let names = list_obj.names().ok_or_else(|| {
        Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' has no names attribute"
        ))
    })?;
    if names.len() != items.len() {
        return Err(Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' has {} names but {} items",
            names.len(),
            items.len()
        )));
    }

    names
        .iter()
        .zip(items.iter())
        .map(|(name, item)| {
            let name = rstr_to_string(name)?;
            let record = parse_record(item, field_name, &name)?;
            Ok((name, record))
        })
        .collect()
}

/// Parses a single `.rdx` entry value: a length-2 numeric vector
/// `c(offset, length)`. R may serialize this as either an integer or a
/// double vector depending on whether the offsets overflow 32-bit ints.
fn parse_record(item: &RObject, field_name: &str, key: &str) -> Result<Record, Error> {
    let values: Vec<f64> = match &item.value() {
        RValue::Integer(values) => values
            .iter()
            .map(|value| value.map(f64::from))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| na_entry_error(field_name, key))?,
        RValue::Real(values) => values
            .iter()
            .copied()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| na_entry_error(field_name, key))?,
        other => {
            return Err(Error::MalformedIndex(format!(
                "'.rdx' field '{field_name}' entry '{key}' is not a numeric vector, got {other:?}"
            )));
        }
    };
    if values.len() != 2 {
        return Err(Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' entry '{key}' has {} elements, expected 2",
            values.len()
        )));
    }

    let offset = to_usize(values[0], field_name, key, "offset")?;
    let length = to_usize(values[1], field_name, key, "length")?;
    Ok(Record { offset, length })
}

fn na_entry_error(field_name: &str, key: &str) -> Error {
    Error::MalformedIndex(format!(
        "'.rdx' field '{field_name}' entry '{key}' contains an NA offset/length"
    ))
}

/// Validates that `value` is a non-negative integer that fits in a `usize`,
/// as required for an `(offset, length)` component.
fn to_usize(value: f64, field_name: &str, key: &str, component: &str) -> Result<usize, Error> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' entry '{key}' has a non-integer or negative {component} ({value})"
        )));
    }
    if value > usize::MAX as f64 {
        return Err(Error::MalformedIndex(format!(
            "'.rdx' field '{field_name}' entry '{key}' has a {component} that overflows usize ({value})"
        )));
    }
    Ok(value as usize)
}
