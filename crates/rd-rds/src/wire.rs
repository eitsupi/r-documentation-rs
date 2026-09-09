//! Shared serialized-wire state for strict decoding and bounded inspection.
//!
//! This module is crate-private deliberately. It owns wire bookkeeping that
//! must remain identical when a future inspector stops before constructing an
//! [`RObject`](crate::RObject): flags, references, encoding context, lengths,
//! depth, and element accounting.

use std::sync::Arc;

use crate::{
    ByteCursor, EnvHandle, Error, Limits, NativeEncodingPolicy, NativeEncodingSource, Persisted,
    RStr, SexpKind, Symbol,
};

pub(crate) const TYPE_MASK: u32 = 0xff;
pub(crate) const ATTRIBUTES_BIT: u32 = 1 << 9;
pub(crate) const TAG_BIT: u32 = 1 << 10;
pub(crate) const LEVELS_SHIFT: u32 = 12;

pub(crate) const NILSXP: u8 = 0;
pub(crate) const SYMSXP: u8 = 1;
pub(crate) const LISTSXP: u8 = 2;
pub(crate) const CLOSXP: u8 = 3;
pub(crate) const ENVSXP: u8 = 4;
pub(crate) const PROMSXP: u8 = 5;
pub(crate) const LANGSXP: u8 = 6;
pub(crate) const SPECIALSXP: u8 = 7;
pub(crate) const BUILTINSXP: u8 = 8;
pub(crate) const CHARSXP: u8 = 9;
pub(crate) const LGLSXP: u8 = 10;
pub(crate) const INTSXP: u8 = 13;
pub(crate) const REALSXP: u8 = 14;
pub(crate) const CPLXSXP: u8 = 15;
pub(crate) const STRSXP: u8 = 16;
pub(crate) const DOTSXP: u8 = 17;
pub(crate) const VECSXP: u8 = 19;
pub(crate) const EXPRSXP: u8 = 20;
#[cfg(test)]
pub(crate) const EXTPTRSXP: u8 = 22;
pub(crate) const RAWSXP: u8 = 24;
pub(crate) const S4SXP: u8 = 25;
pub(crate) const BASEENV_SXP: u8 = 241;
pub(crate) const EMPTYENV_SXP: u8 = 242;
pub(crate) const PACKAGESXP: u8 = 248;
pub(crate) const NAMESPACESXP: u8 = 249;
pub(crate) const BASENAMESPACE_SXP: u8 = 250;
pub(crate) const MISSINGARG_SXP: u8 = 251;
pub(crate) const UNBOUNDVALUE_SXP: u8 = 252;
pub(crate) const GLOBALENV_SXP: u8 = 253;
pub(crate) const NILVALUE_SXP: u8 = 254;
pub(crate) const REFSXP: u8 = 255;
pub(crate) const PERSISTSXP: u8 = 247;

pub(crate) const NA_INTEGER: i32 = i32::MIN;
pub(crate) const NA_REAL_BITS: u64 = 0x7ff0_0000_0000_07a2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ItemFlags {
    raw: u32,
    type_code: u8,
}

impl ItemFlags {
    pub(crate) fn from_raw(raw: u32) -> Self {
        Self {
            raw,
            type_code: (raw & TYPE_MASK) as u8,
        }
    }

    pub(crate) fn type_code(self) -> u8 {
        self.type_code
    }

    pub(crate) fn kind(self) -> SexpKind {
        SexpKind::from_type_code(self.type_code)
    }

    pub(crate) fn has_attributes(self) -> bool {
        self.raw & ATTRIBUTES_BIT != 0
    }

    pub(crate) fn has_tag(self) -> bool {
        self.raw & TAG_BIT != 0
    }

    pub(crate) fn levels(self) -> u32 {
        self.raw >> LEVELS_SHIFT
    }

    pub(crate) fn ref_index_inline(self) -> u32 {
        self.raw >> 8
    }

    #[cfg(test)]
    pub(crate) fn is_object(self) -> bool {
        self.raw & (1 << 8) != 0
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RefEntry {
    Symbol(Symbol),
    Persisted(Persisted),
    Env(EnvHandle),
}

#[derive(Default)]
struct RefTable {
    entries: Vec<RefEntry>,
}

impl RefTable {
    fn register(&mut self, entry: RefEntry, limits: Limits, offset: usize) -> Result<(), Error> {
        let limit = limits.max_references_value();
        if self.entries.len() >= limit {
            return Err(Error::ReferenceLimitExceeded { limit, offset });
        }
        self.entries.push(entry);
        Ok(())
    }

    fn resolve(&self, index: u32, offset: usize) -> Result<&RefEntry, Error> {
        if index == 0 {
            return Err(Error::RefIndexOutOfRange {
                index,
                len: self.entries.len(),
                offset,
            });
        }
        self.entries
            .get(index as usize - 1)
            .ok_or(Error::RefIndexOutOfRange {
                index,
                len: self.entries.len(),
                offset,
            })
    }
}

/// Wire bookkeeping shared by strict decoding and future partial inspection.
pub(crate) struct WireState {
    refs: RefTable,
    limits: Limits,
    total_elements: usize,
    native_encoding_source: NativeEncodingSource,
}

impl WireState {
    pub(crate) fn new(
        limits: Limits,
        native_encoding: Option<String>,
        native_encoding_policy: NativeEncodingPolicy,
    ) -> Self {
        Self {
            refs: RefTable::default(),
            limits,
            total_elements: 0,
            native_encoding_source: match native_encoding {
                Some(name) => NativeEncodingSource::Header(Arc::from(name)),
                None => match native_encoding_policy {
                    NativeEncodingPolicy::RejectUnknown => NativeEncodingSource::Unknown,
                    NativeEncodingPolicy::AssumeUtf8 => NativeEncodingSource::AssumedUtf8,
                },
            },
        }
    }

    pub(crate) fn read_flags(&mut self, cursor: &mut ByteCursor<'_>) -> Result<ItemFlags, Error> {
        Ok(ItemFlags::from_raw(cursor.read_be_u32()?))
    }

    pub(crate) fn decode_char_item(&mut self, cursor: &mut ByteCursor<'_>) -> Result<RStr, Error> {
        let flags = self.read_flags(cursor)?;
        if flags.type_code() != CHARSXP {
            return Err(Error::UnsupportedSexp {
                kind: flags.kind(),
                type_code: flags.type_code(),
                offset: cursor.position().saturating_sub(4),
            });
        }
        self.decode_char_with_flags(cursor, flags)
    }

    pub(crate) fn decode_char_with_flags(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
    ) -> Result<RStr, Error> {
        let len = cursor.read_be_i32()?;
        if len == -1 {
            return Ok(RStr::Na);
        }
        if len < 0 {
            return Err(Error::NegativeLength {
                len,
                offset: cursor.position().saturating_sub(4),
            });
        }

        let encoding = decode_encoding(flags);
        let bytes = cursor.read_exact(len as usize)?;
        Ok(RStr::new(
            bytes,
            encoding,
            self.native_encoding_source().clone(),
        ))
    }

    pub(crate) fn register(&mut self, entry: RefEntry, offset: usize) -> Result<(), Error> {
        self.refs.register(entry, self.limits, offset)
    }

    pub(crate) fn resolve(&self, index: u32, offset: usize) -> Result<&RefEntry, Error> {
        self.refs.resolve(index, offset)
    }

    pub(crate) fn native_encoding_source(&self) -> &NativeEncodingSource {
        &self.native_encoding_source
    }

    pub(crate) fn max_vector_len(&self) -> usize {
        self.limits.max_vector_len_value()
    }

    pub(crate) fn read_vector_len(&mut self, cursor: &mut ByteCursor<'_>) -> Result<usize, Error> {
        let offset = cursor.position();
        let len = cursor.read_be_i32()?;
        if len == -1 {
            let len = read_long_len(cursor)?;
            return Err(Error::LongVectorUnsupported { len, offset });
        }
        if len < 0 {
            return Err(Error::NegativeLength { len, offset });
        }
        let len = len as usize;
        if len > self.limits.max_vector_len_value() {
            return Err(Error::VectorLengthLimitExceeded {
                limit: self.limits.max_vector_len_value(),
                length: len,
                offset,
            });
        }
        self.account_elements(len, offset)?;
        Ok(len)
    }

    pub(crate) fn read_long_len(&mut self, cursor: &mut ByteCursor<'_>) -> Result<u64, Error> {
        read_long_len(cursor)
    }

    pub(crate) fn check_depth(&self, depth: u32) -> Result<(), Error> {
        if depth > self.limits.max_depth_value() {
            Err(Error::DepthLimitExceeded {
                limit: self.limits.max_depth_value(),
            })
        } else {
            Ok(())
        }
    }

    pub(crate) fn account_elements(&mut self, count: usize, offset: usize) -> Result<(), Error> {
        let total = self.total_elements.saturating_add(count);
        if total > self.limits.max_total_elements_value() {
            return Err(Error::TotalElementsLimitExceeded {
                limit: self.limits.max_total_elements_value(),
                total,
                offset,
            });
        }
        self.total_elements = total;
        Ok(())
    }
}

fn read_long_len(cursor: &mut ByteCursor<'_>) -> Result<u64, Error> {
    let upper = cursor.read_be_i32()? as u32 as u64;
    let lower = cursor.read_be_i32()? as u32 as u64;
    Ok((upper << 32) | lower)
}

pub(crate) fn is_nil(flags: ItemFlags) -> bool {
    matches!(flags.type_code(), NILSXP | NILVALUE_SXP)
}

pub(crate) fn is_dotted_pair(flags: ItemFlags) -> bool {
    matches!(
        flags.type_code(),
        LISTSXP | LANGSXP | CLOSXP | PROMSXP | DOTSXP
    )
}

pub(crate) fn decode_encoding(flags: ItemFlags) -> crate::REncoding {
    let levels = flags.levels();
    if levels & (1 << 3) != 0 {
        crate::REncoding::Utf8
    } else if levels & (1 << 2) != 0 {
        crate::REncoding::Latin1
    } else if levels & (1 << 1) != 0 {
        crate::REncoding::Bytes
    } else {
        crate::REncoding::Native
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_decode_wire_bits() {
        let flags = ItemFlags::from_raw(0x0004_0713);
        assert_eq!(flags.type_code(), VECSXP);
        assert!(flags.is_object());
        assert!(flags.has_attributes());
        assert!(flags.has_tag());
        assert_eq!(flags.levels(), 0x40);
        assert_eq!(ItemFlags::from_raw(0x0000_05ff).ref_index_inline(), 5);
    }

    #[test]
    fn reference_limit_is_checked_before_registration() {
        let mut state = WireState::new(
            Limits::default().max_references(0),
            None,
            NativeEncodingPolicy::RejectUnknown,
        );
        let error = state
            .register(RefEntry::Env(EnvHandle::Other), 12)
            .expect_err("zero reference limit should reject registration");
        assert_eq!(
            error,
            Error::ReferenceLimitExceeded {
                limit: 0,
                offset: 12,
            }
        );
        assert!(matches!(
            state.resolve(1, 16),
            Err(Error::RefIndexOutOfRange { len: 0, .. })
        ));
    }
}
