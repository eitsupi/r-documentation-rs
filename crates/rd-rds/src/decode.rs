use std::sync::Arc;

use crate::wire::{
    BASEENV_SXP, BASENAMESPACE_SXP, BUILTINSXP, CHARSXP, CPLXSXP, EMPTYENV_SXP, ENVSXP, EXPRSXP,
    GLOBALENV_SXP, INTSXP, ItemFlags, LGLSXP, LISTSXP, MISSINGARG_SXP, NA_INTEGER, NA_REAL_BITS,
    NAMESPACESXP, NILSXP, NILVALUE_SXP, PACKAGESXP, PERSISTSXP, RAWSXP, REALSXP, REFSXP, RefEntry,
    S4SXP, SPECIALSXP, STRSXP, SYMSXP, UNBOUNDVALUE_SXP, VECSXP, WireState, is_dotted_pair, is_nil,
};
use crate::{
    Attribute, Attributes, ByteCursor, EnvHandle, Error, Header, Limits, Persisted, RObject, RStr,
    RValue, SexpKind, Symbol,
};

pub fn parse(bytes: &[u8]) -> Result<RObject, Error> {
    parse_with_options(bytes, ParseOptions::default())
}

pub fn parse_with_limits(bytes: &[u8], limits: Limits) -> Result<RObject, Error> {
    parse_with_options(bytes, ParseOptions::default().limits(limits))
}

/// Options controlling decompressed RDS parsing.
#[derive(Debug, Clone, Copy, Default)]
#[must_use]
pub struct ParseOptions {
    limits: Limits,
    native_encoding_policy: NativeEncodingPolicy,
}

impl ParseOptions {
    /// Sets the resource limits used while decoding.
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Sets the policy for native strings when the header field is absent,
    /// which means format 2; retained `RStr` values are validated when
    /// converted, while `SYMSXP` print names are converted during parsing.
    pub fn native_encoding_policy(mut self, policy: NativeEncodingPolicy) -> Self {
        self.native_encoding_policy = policy;
        self
    }

    pub(crate) fn limits_value(self) -> Limits {
        self.limits
    }

    pub(crate) fn native_encoding_policy_value(self) -> NativeEncodingPolicy {
        self.native_encoding_policy
    }
}

/// Controls how a native CHARSXP is interpreted when the RDS header field is
/// absent, which means format 2. Parsing retains bytes lazily for `RStr` values:
/// conversion by [`crate::RStr::as_str`] or a typed view then performs validation
/// or rejection. A `SYMSXP` print name is converted during parsing instead, so a
/// symbol name that cannot be decoded fails immediately with
/// [`crate::Error::InvalidSymbolName`] under either policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum NativeEncodingPolicy {
    /// Preserve bytes for retained `RStr` values without assuming an encoding;
    /// conversion later rejects non-ASCII native strings in format 2 when no
    /// header encoding is available. `SYMSXP` print names are decoded during
    /// parsing.
    #[default]
    RejectUnknown,
    /// Treat native strings as UTF-8 in format 2 when the header has no
    /// encoding, for callers with an external UTF-8 contract. Conversion later
    /// validates retained `RStr` bytes without lossy replacement; `SYMSXP`
    /// print names are decoded during parsing.
    AssumeUtf8,
}

/// Parses a decompressed XDR stream with explicit options.
pub fn parse_with_options(bytes: &[u8], options: ParseOptions) -> Result<RObject, Error> {
    let mut cursor = ByteCursor::new(bytes);
    let header = Header::parse(&mut cursor)?;
    Decoder::new(
        options.limits_value(),
        header.native_encoding,
        options.native_encoding_policy_value(),
    )
    .decode_root(&mut cursor)
}

/// Traversal mode for the core decoder.
///
/// `Strict` is the public entry point's behavior: any SEXP type outside the
/// modeled [`RValue`] set is rejected. `Discard` is used only while walking
/// the item fields of a non-singleton environment (`enclos`/`frame`/
/// `hashtab`/`attrib`): it performs the same structural walk (with the same
/// reference-table side effects) but additionally tolerates SEXP types whose
/// layout is verified but not otherwise modeled, discarding their decoded
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Strict,
    Discard,
}

struct Decoder {
    state: WireState,
}

impl Decoder {
    fn new(
        limits: Limits,
        native_encoding: Option<String>,
        native_encoding_policy: NativeEncodingPolicy,
    ) -> Self {
        Self {
            state: WireState::new(limits, native_encoding, native_encoding_policy),
        }
    }

    fn decode_root(&mut self, cursor: &mut ByteCursor<'_>) -> Result<RObject, Error> {
        self.decode_object(cursor, 0, Mode::Strict)
    }

    fn decode_object(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        depth: u32,
        mode: Mode,
    ) -> Result<RObject, Error> {
        self.check_depth(depth)?;
        let flags = self.read_flags(cursor)?;
        self.decode_object_with_flags(cursor, flags, depth, mode)
    }

    fn decode_object_with_flags(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
        depth: u32,
        mode: Mode,
    ) -> Result<RObject, Error> {
        // Types with wire layouts that don't fit the generic
        // "value, then optionally-gated attributes" shape below (bare
        // singleton tags, or an environment whose attrib field is
        // unconditional rather than flag-gated) are dispatched here and
        // return directly, regardless of mode.
        match flags.type_code() {
            REFSXP => return self.decode_ref(cursor, flags),
            ENVSXP => return self.decode_env(cursor, depth),
            GLOBALENV_SXP => return Ok(env_object(EnvHandle::Global)),
            BASEENV_SXP | BASENAMESPACE_SXP => return Ok(env_object(EnvHandle::Base)),
            EMPTYENV_SXP => return Ok(env_object(EnvHandle::Empty)),
            _ => {}
        }

        let value = match flags.type_code() {
            NILSXP | NILVALUE_SXP => RValue::Null,
            CHARSXP => RValue::Character(vec![self.decode_char_with_flags(cursor, flags)?]),
            STRSXP => RValue::Character(self.decode_character_vector(cursor)?),
            LGLSXP => RValue::Logical(self.decode_logical_vector(cursor)?),
            INTSXP => RValue::Integer(self.decode_integer_vector(cursor)?),
            REALSXP => RValue::Real(self.decode_real_vector(cursor)?),
            VECSXP => RValue::List(self.decode_list(cursor, depth, mode)?),
            SYMSXP => RValue::Symbol(self.decode_symbol_with_flags(cursor, flags)?),
            PERSISTSXP => RValue::Persisted(self.decode_persisted(cursor)?),
            PACKAGESXP | NAMESPACESXP => {
                RValue::Environment(self.decode_package_or_namespace(cursor)?)
            }
            other => {
                if mode == Mode::Discard {
                    return self.decode_discard(cursor, flags, depth);
                }
                return Err(Error::UnsupportedSexp {
                    kind: SexpKind::from_type_code(other),
                    type_code: other,
                    offset: cursor.position().saturating_sub(4),
                });
            }
        };

        let attributes = if flags.has_attributes() {
            self.decode_attributes(cursor, depth + 1, mode)?
        } else {
            Attributes::default()
        };

        Ok(RObject::from_parts(value, attributes))
    }

    /// Decodes a non-singleton `ENVSXP`: `locked` (raw i32, registered
    /// immediately after), then `enclos`/`frame`/`hashtab`/`attrib`, all
    /// unconditionally present and all decoded (and discarded) in
    /// [`Mode::Discard`]. Environments are opaque by design: only their
    /// wire bytes and reference-table side effects matter.
    fn decode_env(&mut self, cursor: &mut ByteCursor<'_>, depth: u32) -> Result<RObject, Error> {
        let _locked = cursor.read_be_i32()?;
        self.state
            .register(RefEntry::Env(EnvHandle::Other), cursor.position())?;
        for _ in 0..4 {
            let _ = self.decode_object(cursor, depth + 1, Mode::Discard)?;
        }
        Ok(env_object(EnvHandle::Other))
    }

    /// Decodes the shared `PACKAGESXP`/`NAMESPACESXP` payload: the same
    /// "string vec" format used by `PERSISTSXP`, registered in the
    /// reference table after the payload is read.
    fn decode_package_or_namespace(
        &mut self,
        cursor: &mut ByteCursor<'_>,
    ) -> Result<EnvHandle, Error> {
        let _ = self.decode_string_vec(cursor)?;
        self.state
            .register(RefEntry::Env(EnvHandle::Other), cursor.position())?;
        Ok(EnvHandle::Other)
    }

    /// Handles the SEXP types that are only tolerated in [`Mode::Discard`]:
    /// their wire layout is verified but they have no [`RValue`]
    /// representation, so the decoded value is always discarded in favor of
    /// [`RValue::Null`]. Types whose layout is not verified still fail with
    /// [`Error::UnsupportedSexp`].
    fn decode_discard(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
        depth: u32,
    ) -> Result<RObject, Error> {
        if is_dotted_pair(flags) {
            // Dotted pairs handle their own (optional) attributes and tag
            // internally, so they never fall through to the generic
            // trailing-attributes handling below.
            self.discard_pairlist_chain(cursor, flags, depth)?;
            return Ok(RObject::from_parts(RValue::Null, Attributes::default()));
        }

        match flags.type_code() {
            UNBOUNDVALUE_SXP | MISSINGARG_SXP => {
                return Ok(RObject::from_parts(RValue::Null, Attributes::default()));
            }
            SPECIALSXP | BUILTINSXP => {
                let len = self.read_vector_len(cursor)?;
                let _ = cursor.read_exact(len)?;
            }
            CPLXSXP => {
                let len = self.read_vector_len(cursor)?;
                for _ in 0..len {
                    let _ = cursor.read_exact(16)?;
                }
            }
            RAWSXP => {
                let len = self.read_vector_len(cursor)?;
                let _ = cursor.read_exact(len)?;
            }
            EXPRSXP => {
                // Same framing as VECSXP: a length followed by that many items.
                let _ = self.decode_list(cursor, depth, Mode::Discard)?;
            }
            S4SXP => {
                // No body content beyond the generic trailing attributes.
            }
            other => {
                return Err(Error::UnsupportedSexp {
                    kind: SexpKind::from_type_code(other),
                    type_code: other,
                    offset: cursor.position().saturating_sub(4),
                });
            }
        }

        let attributes = if flags.has_attributes() {
            self.decode_attributes(cursor, depth + 1, Mode::Discard)?
        } else {
            Attributes::default()
        };

        Ok(RObject::from_parts(RValue::Null, attributes))
    }

    /// Discards a dotted-pair chain (`LISTSXP`/`LANGSXP`/`CLOSXP`/
    /// `PROMSXP`/`DOTSXP`) iteratively over the CDR links, so long
    /// pairlists don't add stack depth. Each link decodes an optional
    /// attributes item, an optional tag item, and the CAR, all generically
    /// in [`Mode::Discard`]; the CDR either continues the loop (another
    /// dotted-pair link), stops (NIL), or is decoded once more as an
    /// improper-list tail.
    fn discard_pairlist_chain(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
        depth: u32,
    ) -> Result<(), Error> {
        let mut flags = flags;
        loop {
            self.account_elements(1, cursor.position())?;
            if flags.has_attributes() {
                let _ = self.decode_attributes(cursor, depth + 1, Mode::Discard)?;
            }
            if flags.has_tag() {
                let _ = self.decode_object(cursor, depth + 1, Mode::Discard)?;
            }
            let _ = self.decode_object(cursor, depth + 1, Mode::Discard)?;

            let cdr_flags = self.read_flags(cursor)?;
            if is_dotted_pair(cdr_flags) {
                flags = cdr_flags;
                continue;
            }
            if is_nil(cdr_flags) {
                return Ok(());
            }
            let _ = self.decode_object_with_flags(cursor, cdr_flags, depth + 1, Mode::Discard)?;
            return Ok(());
        }
    }

    fn read_flags(&mut self, cursor: &mut ByteCursor<'_>) -> Result<ItemFlags, Error> {
        self.state.read_flags(cursor)
    }

    fn decode_ref(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
    ) -> Result<RObject, Error> {
        let inline_index = flags.ref_index_inline();
        let index = if inline_index == 0 {
            cursor.read_be_i32()? as u32
        } else {
            inline_index
        };

        match self
            .state
            .resolve(index, cursor.position().saturating_sub(4))?
        {
            RefEntry::Symbol(symbol) => Ok(RObject::from_parts(
                RValue::Symbol(symbol.clone()),
                Attributes::default(),
            )),
            RefEntry::Persisted(persisted) => Ok(RObject::from_parts(
                RValue::Persisted(persisted.clone()),
                Attributes::default(),
            )),
            RefEntry::Env(handle) => Ok(env_object(*handle)),
        }
    }

    fn decode_list(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        depth: u32,
        mode: Mode,
    ) -> Result<Vec<RObject>, Error> {
        let len = self.read_vector_len(cursor)?;
        (0..len)
            .map(|_| self.decode_object(cursor, depth + 1, mode))
            .collect()
    }

    fn decode_logical_vector(
        &mut self,
        cursor: &mut ByteCursor<'_>,
    ) -> Result<Vec<Option<bool>>, Error> {
        let len = self.read_vector_len(cursor)?;
        (0..len)
            .map(|_| {
                Ok(match cursor.read_be_i32()? {
                    NA_INTEGER => None,
                    0 => Some(false),
                    _ => Some(true),
                })
            })
            .collect()
    }

    fn decode_integer_vector(
        &mut self,
        cursor: &mut ByteCursor<'_>,
    ) -> Result<Vec<Option<i32>>, Error> {
        let len = self.read_vector_len(cursor)?;
        (0..len)
            .map(|_| {
                let value = cursor.read_be_i32()?;
                Ok((value != NA_INTEGER).then_some(value))
            })
            .collect()
    }

    fn decode_real_vector(
        &mut self,
        cursor: &mut ByteCursor<'_>,
    ) -> Result<Vec<Option<f64>>, Error> {
        let len = self.read_vector_len(cursor)?;
        (0..len)
            .map(|_| {
                let bits = cursor.read_be_u64()?;
                Ok((bits != NA_REAL_BITS).then_some(f64::from_bits(bits)))
            })
            .collect()
    }

    fn decode_character_vector(&mut self, cursor: &mut ByteCursor<'_>) -> Result<Vec<RStr>, Error> {
        let len = self.read_vector_len(cursor)?;
        (0..len).map(|_| self.decode_char_item(cursor)).collect()
    }

    fn decode_char_item(&mut self, cursor: &mut ByteCursor<'_>) -> Result<RStr, Error> {
        self.state.decode_char_item(cursor)
    }

    fn decode_char_with_flags(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
    ) -> Result<RStr, Error> {
        self.state.decode_char_with_flags(cursor, flags)
    }

    fn decode_symbol_with_flags(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        _flags: ItemFlags,
    ) -> Result<Symbol, Error> {
        let print_name = self.decode_char_item(cursor)?;
        let text = print_name
            .as_str()
            .ok_or(Error::InvalidSymbolName)?
            .map_err(|_| Error::InvalidSymbolName)?;
        let symbol = Symbol::new(Arc::<str>::from(text.as_ref()));
        self.state
            .register(RefEntry::Symbol(symbol.clone()), cursor.position())?;
        Ok(symbol)
    }

    fn decode_persisted(&mut self, cursor: &mut ByteCursor<'_>) -> Result<Persisted, Error> {
        let values = self.decode_string_vec(cursor)?;
        let persisted = Persisted::new(values);
        self.state
            .register(RefEntry::Persisted(persisted.clone()), cursor.position())?;
        Ok(persisted)
    }

    /// Decodes the "string vec" payload shared by `PERSISTSXP` and
    /// `PACKAGESXP`/`NAMESPACESXP`: a discarded i32 placeholder, then an i32
    /// count (with the usual -1 long-vector escape), then that many
    /// `CHARSXP` items.
    fn decode_string_vec(&mut self, cursor: &mut ByteCursor<'_>) -> Result<Vec<RStr>, Error> {
        let _placeholder = cursor.read_be_i32()?;
        let offset = cursor.position();
        let len = cursor.read_be_i32()?;
        let len = if len == -1 {
            let len = self.state.read_long_len(cursor)?;
            return Err(Error::PersistedLongVectorUnsupported { len, offset });
        } else if len < 0 {
            return Err(Error::NegativeLength { len, offset });
        } else {
            len as usize
        };

        if len > self.state.max_vector_len() {
            return Err(Error::VectorLengthLimitExceeded {
                limit: self.state.max_vector_len(),
                length: len,
                offset,
            });
        }
        self.state.account_elements(len, offset)?;
        (0..len)
            .map(|_| self.decode_char_item(cursor))
            .collect::<Result<Vec<_>, _>>()
    }

    fn decode_attributes(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        depth: u32,
        mode: Mode,
    ) -> Result<Attributes, Error> {
        self.check_depth(depth)?;
        let flags = self.read_flags(cursor)?;
        if is_nil(flags) {
            return Ok(Attributes::default());
        }
        let attributes = self.decode_attribute_pairlist_with_flags(cursor, flags, depth, mode)?;
        Ok(Attributes::new(attributes))
    }

    fn decode_attribute_pairlist_with_flags(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        flags: ItemFlags,
        depth: u32,
        mode: Mode,
    ) -> Result<Vec<Attribute>, Error> {
        // The cell's flags were consumed by the caller immediately before
        // this call; keep their offset so tag errors point at the offending
        // pairlist cell even after nested attributes advance the cursor.
        let flags_offset = cursor.position().saturating_sub(4);
        if flags.type_code() != LISTSXP {
            return Err(Error::UnsupportedSexp {
                kind: flags.kind(),
                type_code: flags.type_code(),
                offset: flags_offset,
            });
        }

        self.account_elements(1, cursor.position())?;

        if flags.has_attributes() {
            let _ = self.decode_attributes(cursor, depth + 1, mode)?;
        }

        let name = if flags.has_tag() {
            self.decode_attribute_tag(cursor, depth + 1)?
        } else {
            return Err(Error::InvalidAttributeTag {
                offset: flags_offset,
            });
        };

        let value = self.decode_object(cursor, depth + 1, mode)?;
        let cdr_flags = self.read_flags(cursor)?;
        let mut attributes = vec![Attribute::new(name, value)];

        if !is_nil(cdr_flags) {
            attributes.extend(self.decode_attribute_pairlist_with_flags(
                cursor,
                cdr_flags,
                depth + 1,
                mode,
            )?);
        }

        Ok(attributes)
    }

    fn decode_attribute_tag(
        &mut self,
        cursor: &mut ByteCursor<'_>,
        depth: u32,
    ) -> Result<Symbol, Error> {
        self.check_depth(depth)?;
        let flags = self.read_flags(cursor)?;
        match flags.type_code() {
            SYMSXP => self.decode_symbol_with_flags(cursor, flags),
            REFSXP => {
                let inline_index = flags.ref_index_inline();
                let index = if inline_index == 0 {
                    cursor.read_be_i32()? as u32
                } else {
                    inline_index
                };
                match self
                    .state
                    .resolve(index, cursor.position().saturating_sub(4))?
                {
                    RefEntry::Symbol(symbol) => Ok(symbol.clone()),
                    RefEntry::Persisted(_) | RefEntry::Env(_) => Err(Error::InvalidAttributeTag {
                        offset: cursor.position().saturating_sub(4),
                    }),
                }
            }
            _ => Err(Error::InvalidAttributeTag {
                offset: cursor.position().saturating_sub(4),
            }),
        }
    }

    fn read_vector_len(&mut self, cursor: &mut ByteCursor<'_>) -> Result<usize, Error> {
        self.state.read_vector_len(cursor)
    }

    fn check_depth(&self, depth: u32) -> Result<(), Error> {
        self.state.check_depth(depth)
    }

    fn account_elements(&mut self, count: usize, offset: usize) -> Result<(), Error> {
        self.state.account_elements(count, offset)
    }
}

/// Decodes a `CHARSXP` encoding from the `levels` mask bits per R's
/// `InCharSXP`: UTF-8 (bit 3) takes priority, then Latin-1 (bit 2), then
/// bytes (bit 1), else native. The ASCII marker (bit 6) is a non-exclusive
/// hint, not a distinct encoding, so it naturally falls through to Native.
fn env_object(handle: EnvHandle) -> RObject {
    RObject::from_parts(RValue::Environment(handle), Attributes::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::REncoding;
    use crate::wire::{ATTRIBUTES_BIT, CLOSXP, EXTPTRSXP, TAG_BIT};
    use std::{fs, io::Read, path::PathBuf};

    use flate2::read::GzDecoder;

    fn item(bytes: &[u8]) -> Result<RObject, Error> {
        item_with_limits(bytes, Limits::default())
    }

    fn item_with_limits(bytes: &[u8], limits: Limits) -> Result<RObject, Error> {
        let mut cursor = ByteCursor::new(bytes);
        Decoder::new(limits, None, NativeEncodingPolicy::RejectUnknown).decode_root(&mut cursor)
    }

    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data")
    }

    fn fixture(name: &str) -> RObject {
        let bytes = fs::read(fixture_dir().join(name)).expect("fixture bytes");
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .expect("fixture gzip stream");
        parse(&decompressed).expect(name)
    }

    fn rstr(value: &RStr) -> String {
        value.as_str().unwrap().unwrap().into_owned()
    }

    fn strings(value: &RObject) -> Vec<String> {
        let RValue::Character(values) = value.value() else {
            panic!("expected character vector, got {value:?}");
        };
        values.iter().map(rstr).collect()
    }

    fn list(value: &RObject) -> &[RObject] {
        let RValue::List(values) = value.value() else {
            panic!("expected list, got {value:?}");
        };
        values
    }

    fn persisted(value: &RObject) -> &Persisted {
        let RValue::Persisted(value) = value.value() else {
            panic!("expected persisted value, got {value:?}");
        };
        value
    }

    fn env_handle(value: &RObject) -> EnvHandle {
        let RValue::Environment(handle) = value.value() else {
            panic!("expected environment, got {value:?}");
        };
        *handle
    }

    fn symbol_name(value: &RObject) -> &str {
        let RValue::Symbol(symbol) = value.value() else {
            panic!("expected symbol, got {value:?}");
        };
        symbol.as_str()
    }

    #[test]
    fn decodes_flags_word() {
        let flags = ItemFlags::from_raw(0x0004_0713);
        assert_eq!(flags.type_code(), VECSXP);
        assert!(flags.is_object());
        assert!(flags.has_attributes());
        assert!(flags.has_tag());
        assert_eq!(flags.levels(), 0x40);

        let ref_flags = ItemFlags::from_raw(0x0000_05ff);
        assert_eq!(ref_flags.type_code(), REFSXP);
        assert_eq!(ref_flags.ref_index_inline(), 5);
    }

    #[test]
    fn decodes_na_string_integer_and_logical() {
        let charsxp_na = [0, 0, 0, CHARSXP, 0xff, 0xff, 0xff, 0xff];
        let value = item(&charsxp_na).unwrap();
        assert_eq!(value.value(), &RValue::Character(vec![RStr::Na]));

        let int_vec = [0, 0, 0, INTSXP, 0, 0, 0, 1, 0x80, 0, 0, 0];
        let value = item(&int_vec).unwrap();
        assert_eq!(value.value(), &RValue::Integer(vec![None]));

        let logical_vec = [0, 0, 0, LGLSXP, 0, 0, 0, 2, 0x80, 0, 0, 0, 0, 0, 0, 1];
        let value = item(&logical_vec).unwrap();
        assert_eq!(value.value(), &RValue::Logical(vec![None, Some(true)]));
    }

    #[test]
    fn charsxp_encoding_levels_bits() {
        // UTF-8 levels bit (1 << 3) takes priority.
        let flags: u32 = 9 | (8 << 12);
        let mut bytes = flags.to_be_bytes().to_vec();
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(b'a');
        let value = item(&bytes).unwrap();
        let RValue::Character(strs) = value.value() else {
            panic!("expected character vector, got {value:?}");
        };
        assert_eq!(strs[0].encoding(), Some(REncoding::Utf8));

        // The ASCII marker bit (1 << 6) is a non-exclusive hint, not a
        // distinct encoding, so it falls through to Native.
        let flags: u32 = 9 | (64 << 12);
        let mut bytes = flags.to_be_bytes().to_vec();
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(b'a');
        let value = item(&bytes).unwrap();
        let RValue::Character(strs) = value.value() else {
            panic!("expected character vector, got {value:?}");
        };
        assert_eq!(strs[0].encoding(), Some(REncoding::Native));
        assert_eq!(strs[0].as_str().unwrap().unwrap().as_ref(), "a");
    }

    #[test]
    fn native_symbol_names_are_decoded_during_format_v2_parsing() {
        // Handwritten because R cannot easily serialize a non-ASCII Native
        // symbol deterministically for a fixture.
        fn stream(print_name: &[u8]) -> Vec<u8> {
            let mut bytes = vec![b'X', b'\n', 0, 0, 0, 2, 0, 4, 6, 1, 0, 3, 5, 0];
            bytes.extend_from_slice(&u32::from(SYMSXP).to_be_bytes());
            bytes.extend_from_slice(&u32::from(CHARSXP).to_be_bytes());
            bytes.extend_from_slice(&(print_name.len() as i32).to_be_bytes());
            bytes.extend_from_slice(print_name);
            bytes
        }

        let valid_utf8 = stream("é".as_bytes());
        assert_eq!(parse(&valid_utf8), Err(Error::InvalidSymbolName));

        let symbol = parse_with_options(
            &valid_utf8,
            ParseOptions::default().native_encoding_policy(NativeEncodingPolicy::AssumeUtf8),
        )
        .expect("AssumeUtf8 should decode a valid Native symbol name");
        assert_eq!(symbol_name(&symbol), "é");

        let invalid_utf8 = stream(&[0xff]);
        assert_eq!(
            parse_with_options(
                &invalid_utf8,
                ParseOptions::default().native_encoding_policy(NativeEncodingPolicy::AssumeUtf8),
            ),
            Err(Error::InvalidSymbolName)
        );
    }

    #[test]
    fn untagged_attribute_cell_with_nested_attributes_reports_cell_offset() {
        let mut bytes = Vec::new();
        // Root: logical vector carrying an attribute pairlist.
        bytes.extend_from_slice(&(u32::from(LGLSXP) | ATTRIBUTES_BIT).to_be_bytes());
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.extend_from_slice(&1i32.to_be_bytes());
        let cell_flags_offset = bytes.len();
        // Attribute cell with nested attributes but no tag: decoding the
        // nested attributes advances the cursor well past the cell's flags.
        bytes.extend_from_slice(&(u32::from(LISTSXP) | ATTRIBUTES_BIT).to_be_bytes());
        bytes.extend_from_slice(&(u32::from(LISTSXP) | TAG_BIT).to_be_bytes());
        bytes.extend_from_slice(&u32::from(SYMSXP).to_be_bytes());
        bytes.extend_from_slice(&(u32::from(CHARSXP) | (8 << 12)).to_be_bytes());
        bytes.extend_from_slice(&1i32.to_be_bytes());
        bytes.push(b'x');
        bytes.extend_from_slice(&u32::from(NILVALUE_SXP).to_be_bytes());
        bytes.extend_from_slice(&u32::from(NILVALUE_SXP).to_be_bytes());

        let err = item(&bytes).unwrap_err();
        assert!(
            matches!(err, Error::InvalidAttributeTag { offset } if offset == cell_flags_offset),
            "expected InvalidAttributeTag at {cell_flags_offset}, got {err:?}"
        );
    }

    #[test]
    fn singleton_env_byte_level_decoding() {
        for (byte, expected) in [
            (253u8, EnvHandle::Global),
            (241u8, EnvHandle::Base),
            (242u8, EnvHandle::Empty),
            (250u8, EnvHandle::Base),
        ] {
            let bytes = [0, 0, 0, byte];
            let mut cursor = ByteCursor::new(&bytes);
            let value = Decoder::new(Limits::default(), None, NativeEncodingPolicy::RejectUnknown)
                .decode_root(&mut cursor)
                .unwrap();
            assert_eq!(value.value(), &RValue::Environment(expected));
            assert_eq!(cursor.remaining(), 0);
        }
    }

    #[test]
    fn environment_frame_pairlist_cells_count_toward_total_elements_limit() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::from(ENVSXP).to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // enclos

        bytes.extend_from_slice(&u32::from(LISTSXP).to_be_bytes());
        for index in 0..3 {
            bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // CAR
            let cdr = if index == 2 { 0 } else { u32::from(LISTSXP) };
            bytes.extend_from_slice(&cdr.to_be_bytes());
        }
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // hashtab
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // attrib

        let error = item_with_limits(&bytes, Limits::default().max_total_elements(2))
            .expect_err("frame pairlist should exceed the element limit");
        assert!(matches!(
            error,
            Error::TotalElementsLimitExceeded { limit: 2, .. }
        ));
    }

    #[test]
    fn compliant_environment_frame_pairlist_decodes_to_other() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&u32::from(ENVSXP).to_be_bytes());
        bytes.extend_from_slice(&0i32.to_be_bytes());
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // enclos
        bytes.extend_from_slice(&u32::from(LISTSXP).to_be_bytes());
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // CAR
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // CDR
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // hashtab
        bytes.extend_from_slice(&u32::from(0u8).to_be_bytes()); // attrib

        let value = item_with_limits(&bytes, Limits::default().max_total_elements(1))
            .expect("compliant environment should decode");
        assert_eq!(env_handle(&value), EnvHandle::Other);
    }

    #[test]
    fn refsxp_out_of_range_index_is_reported() {
        let err = item(&[0, 0, 1, REFSXP]).unwrap_err();
        assert_eq!(
            err,
            Error::RefIndexOutOfRange {
                index: 1,
                len: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn reference_limit_is_enforced_before_symbol_registration() {
        let bytes = [
            0, 0, 0, SYMSXP, // symbol
            0, 0, 0, CHARSXP, // print name
            0, 0, 0, 1, b'a',
        ];
        let error = item_with_limits(&bytes, Limits::default().max_references(0))
            .expect_err("zero reference limit should reject the symbol");
        assert!(matches!(
            error,
            Error::ReferenceLimitExceeded { limit: 0, .. }
        ));
    }

    #[test]
    fn persistsxp_long_vector_escape_is_reported() {
        let err = item(&[
            0, 0, 0, PERSISTSXP, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 1, 0, 0, 0, 2,
        ])
        .unwrap_err();
        assert_eq!(
            err,
            Error::PersistedLongVectorUnsupported {
                len: 0x1_0000_0002,
                offset: 8
            }
        );
    }

    #[test]
    fn unsupported_type_is_reported() {
        let err = item(&[0, 0, 0, EXTPTRSXP]).unwrap_err();
        assert_eq!(
            err,
            Error::UnsupportedSexp {
                kind: SexpKind::ExtPtr,
                type_code: EXTPTRSXP,
                offset: 0
            }
        );
    }

    #[test]
    fn strict_mode_rejects_dotted_pair_at_top_level() {
        let err = item(&[0, 0, 0, CLOSXP]).unwrap_err();
        assert_eq!(
            err,
            Error::UnsupportedSexp {
                kind: SexpKind::Closure,
                type_code: CLOSXP,
                offset: 0
            }
        );
    }

    #[test]
    fn decodes_aliases_vector_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("aliases_vector_v{version}.rds"));
            assert_eq!(
                strings(&root),
                vec![
                    "minimal",
                    "multialias",
                    "multialias",
                    "multialias",
                    "multialias"
                ]
            );
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec![
                    "minimal",
                    "multialias",
                    "multialias-method",
                    "multialias.default",
                    "print.multialias"
                ]
            );
        }
    }

    #[test]
    fn decodes_shared_symbols_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("shared_symbols_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["a", "b", "d"]
            );
            let items = list(&root);
            assert_eq!(items.len(), 3);

            assert_eq!(
                items[0]
                    .class()
                    .unwrap()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["widget"]
            );
            assert_eq!(
                strings(items[0].attributes().get("note").unwrap()),
                vec!["first"]
            );

            let b_items = list(&items[1]);
            assert_eq!(
                items[1]
                    .class()
                    .unwrap()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["widget"]
            );
            assert_eq!(
                strings(items[1].attributes().get("note").unwrap()),
                vec!["third"]
            );
            assert_eq!(
                items[1]
                    .names()
                    .unwrap()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["c"]
            );
            assert_eq!(
                b_items[0]
                    .class()
                    .unwrap()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["widget"]
            );
            assert_eq!(
                strings(b_items[0].attributes().get("note").unwrap()),
                vec!["second"]
            );

            assert_eq!(strings(&items[2]), vec!["x"]);
            assert_eq!(
                items[2]
                    .class()
                    .unwrap()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["widget"]
            );
            assert_eq!(
                strings(items[2].attributes().get("note").unwrap()),
                vec!["fourth"]
            );
        }
    }

    #[test]
    fn decodes_persistsxp_basic_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("persistsxp_basic_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["env", "tail"]
            );
            let items = list(&root);
            assert_eq!(
                persisted(&items[0])
                    .as_slice()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["srcref-env"]
            );
            assert_eq!(strings(&items[1]), vec!["tail-marker"]);
        }
    }

    #[test]
    fn decodes_persistsxp_twice_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("persistsxp_twice_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["first", "second", "after"]
            );
            let items = list(&root);
            let first = persisted(&items[0]);
            let second = persisted(&items[1]);
            assert_eq!(
                first.as_slice().iter().map(rstr).collect::<Vec<_>>(),
                vec!["srcref-env"]
            );
            assert_eq!(
                second.as_slice().iter().map(rstr).collect::<Vec<_>>(),
                vec!["srcref-env"]
            );
            assert!(!first.ptr_eq(second));
            assert_eq!(strings(&items[2]), vec!["tail-marker"]);
        }
    }

    #[test]
    fn decodes_persistsxp_multi_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("persistsxp_multi_v{version}.rds"));
            let items = list(&root);
            assert_eq!(
                persisted(&items[0])
                    .as_slice()
                    .iter()
                    .map(rstr)
                    .collect::<Vec<_>>(),
                vec!["a", "b", "c"]
            );
            assert_eq!(strings(&items[1]), vec!["tail-marker"]);
        }
    }

    #[test]
    fn decodes_singleton_envs_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("singleton_envs_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["global", "base", "empty"]
            );
            let items = list(&root);
            assert_eq!(env_handle(&items[0]), EnvHandle::Global);
            assert_eq!(env_handle(&items[1]), EnvHandle::Base);
            assert_eq!(env_handle(&items[2]), EnvHandle::Empty);
        }
    }

    #[test]
    fn decodes_plain_env_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("plain_env_v{version}.rds"));
            assert_eq!(env_handle(&root), EnvHandle::Other);
            assert!(root.attributes().is_empty());
        }
    }

    #[test]
    fn decodes_env_with_closure_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("env_with_closure_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["env", "tail"]
            );
            let items = list(&root);
            assert_eq!(items[0].value(), &RValue::Environment(EnvHandle::Other));
            assert_eq!(strings(&items[1]), vec!["tail-marker"]);
        }
    }

    #[test]
    fn decodes_shared_env_refs_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("shared_env_refs_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["vec", "sym_a", "env_first", "sym_b", "env_second"]
            );
            let items = list(&root);
            assert_eq!(
                items[0].value(),
                &RValue::Integer(vec![Some(4), Some(2), Some(7)])
            );
            assert_eq!(symbol_name(&items[1]), "dup_sym");
            assert_eq!(env_handle(&items[2]), EnvHandle::Other);
            assert_eq!(symbol_name(&items[3]), "dup_sym");
            assert_eq!(env_handle(&items[4]), EnvHandle::Other);
        }
    }

    /// ALTREP is deliberately out of scope for this decoder: real help DBs
    /// never contain it in value trees, so it fails with
    /// `Error::UnsupportedSexp` rather than being modeled.
    #[test]
    fn altrep_is_rejected() {
        let bytes = fs::read(fixture_dir().join("altrep_intseq_v3.rds")).expect("fixture bytes");
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .expect("fixture gzip stream");
        let err = parse(&decompressed).unwrap_err();
        assert_eq!(
            err,
            Error::UnsupportedSexp {
                kind: SexpKind::Other(238),
                type_code: 238,
                offset: 23
            }
        );
    }

    #[test]
    fn decodes_namespace_refs_fixtures() {
        for version in [2, 3] {
            let root = fixture(&format!("namespace_refs_v{version}.rds"));
            assert_eq!(
                root.names().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                vec!["ns_first", "ns_second", "tail"]
            );
            let items = list(&root);
            assert_eq!(items[0].value(), &RValue::Environment(EnvHandle::Other));
            assert_eq!(items[1].value(), &RValue::Environment(EnvHandle::Other));
            assert_eq!(strings(&items[2]), vec!["tail-marker"]);
        }
    }

    #[test]
    fn decodes_rd_fixtures_as_rd_class_lists() {
        for name in ["rd_minimal", "rd_aliases", "rd_arguments", "rd_seealso"] {
            for version in [2, 3] {
                let root = fixture(&format!("{name}_v{version}.rds"));
                assert!(matches!(root.value(), &RValue::List(_)));
                assert_eq!(
                    root.class().unwrap().iter().map(rstr).collect::<Vec<_>>(),
                    vec!["Rd"]
                );
                if name == "rd_seealso" {
                    assert!(root.attributes().get("srcref").is_some());
                }

                // parse_Rd invariant: every list node carries an "Rd_tag"
                // attribute (text leaves are Character vectors with Rd_tag
                // too). Checked robustly rather than exhaustively: at least
                // the first element has it, and at least one element among
                // the root's children has it.
                let items = list(&root);
                assert!(!items.is_empty(), "{name}_v{version}: empty root list");
                let mut tagged_count = 0usize;
                for (index, item) in items.iter().enumerate() {
                    if matches!(item.value(), &RValue::List(_)) {
                        let rd_tag = item.attributes().get("Rd_tag");
                        if index == 0 {
                            assert!(
                                rd_tag.is_some(),
                                "{name}_v{version}: first element missing Rd_tag"
                            );
                        }
                        if let Some(rd_tag) = rd_tag {
                            assert!(
                                matches!(rd_tag.value(), &RValue::Character(_)),
                                "{name}_v{version}: Rd_tag value is not a character vector"
                            );
                            tagged_count += 1;
                        }
                    }
                }
                assert!(
                    tagged_count >= 1,
                    "{name}_v{version}: no list element carries Rd_tag"
                );
            }
        }
    }
}
