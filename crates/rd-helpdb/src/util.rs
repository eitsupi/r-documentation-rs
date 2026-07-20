//! Small shared helpers used across the crate.

use rd_rds::RStr;

use crate::Error;

/// Decodes an [`RStr`] to an owned [`String`], turning an NA string or a
/// string-decode failure into a crate-level [`Error`].
pub(crate) fn rstr_to_string(value: &RStr) -> Result<String, Error> {
    match value.as_str() {
        Some(Ok(text)) => Ok(text.into_owned()),
        Some(Err(err)) => Err(Error::Rds(err)),
        None => Err(Error::MalformedIndex("unexpected NA string".into())),
    }
}
