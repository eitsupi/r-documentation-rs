# rd-rds

`rd-rds` is a scoped, read-only reader for installed-R-package information and
selected CRAN-like repository indexes. It is not a general R serialization
library and never silently accepts an unknown SEXP. Its compression support is
implemented in pure Rust, so the crate links no C library. See the [workspace
README](../../README.md) for repository status and crate relationships.

The API has three layers:

- `parse` reads a decompressed XDR serialization stream only.
- `file::from_bytes` and `file::read` accept the complete envelope and apply
  bounded decompression. Supported envelopes are raw `X\n` XDR, gzip, xz,
  bzip2, and zstd (when the corresponding feature is enabled).
- `package` provides validated convenience views for `Meta/package.rds` and
  CRAN-like `PACKAGES.rds` matrices.
- `matrix::CharacterMatrix` provides a validated, owned view of general R
  character matrices, including matrices without `dimnames`.

With the opt-in `lazyload` feature, [`lazyload`] provides bounded access to an
installed package's `R/<pkg>.rdx` and `R/<pkg>.rdb` pair. It retains stored
variables in index order (including duplicate names), resolves name lookups
with last-wins semantics, and reads direct records without an R session. The
record layer recognizes uncompressed and zlib records. bzip2 and xz entries
are recognized in the index and always report an explicit unsupported error;
their codecs are outside this milestone. Compound persistence references are
described by their eager and lazy record locations but are not resolved. The
API returns both the exact addressed bytes and the decompressed payload, with
independent 256 MiB default bounds. Raw records are already XDR bytes and do
not carry a length prefix; zlib records carry a four-byte declared length.

With the same feature, [`package::InstalledCodeDb`] provides the package-level
API for that pair. The caller supplies the installed package directory; the
reader selects `R/<basename>.rdx` and `.rdb` and does not discover libraries or
scan runtime exports. [`InstalledCodeDb::stored_bindings`] is the complete
`variables` map in index order, including duplicate names. Consequently this
view's completeness domain is `CodeDatabaseVariables`, not exports or runtime
bindings. [`InstalledCodeDb::inspect_stored_binding`] returns structured
unknown/ambiguous errors instead of choosing among duplicate names, reads only
a unique direct record, and returns bounded closure prefix metadata. Closure
bodies are reported as `BodyValidation::NotValidated`.
The provenance includes both selected paths, compression, and an opaque
`CodeDbGeneration` derived from best-effort file metadata. It is an identity
hint rather than a content hash or a transaction guarantee.

The `lazyload` feature includes the `gzip` feature because installed package
`.rdx` indexes use the standalone gzip envelope in the normal package
profile. Callers that enable `lazyload` therefore also get gzip `.rds`
handling; the other standalone codecs remain independently selectable.
The low-level `lazyload::decode_stored_record` helper applies the same bounded
record decoder to an already isolated `(offset, length)` byte slice.

Record reads take a metadata snapshot when the database opens and compare it
before and after each read. On Unix this includes device and inode, and on
other platforms it uses file length and modification time when available.
This is best-effort detection of replacement or concurrent modification and
cannot provide a transaction guarantee against races after the final check.

The [`rd-helpdb`](../rd-helpdb/README.md) crate uses the file layer for
standalone help-database RDS files, and [`rd-ast`](../rd-ast/README.md) can
lower supported decoded documentation objects into the common document model.

## Namespace metadata

[`package::NamespaceMetadata`] provides an owned view of declarations from a
decoded `Meta/nsInfo.rds` object. Its exports, imports, S3 registrations, and
S4 declarations are static metadata: they are not runtime namespace exports,
stored lazy-load bindings, evaluated export patterns, or `.onLoad` results.
Each known field is independently represented by [`package::MetadataField`],
so a malformed S3 schema does not hide valid declared exports.
Declared exports are returned as [`package::NamespaceExport`] values: the
source binding and namespace-facing name are preserved separately, so an
assignment-shaped export such as `export(public = internal)` is not confused
with an ordinary `export(name)`. Empty export-name attributes use the source
name. The installed-package fixture also exercises the R-written
`list("utils", except = ...)` import shape and an aliased `importFrom`.

A consumer such as a mini-roxygen provider can retain its existing policy
boundary while replacing ad-hoc S3 extraction with positive evidence:

```rust
use std::collections::BTreeSet;
use rd_rds::package::{MetadataField, NamespaceMetadata};

fn generic_evidence(object: &rd_rds::RObject) -> BTreeSet<String> {
    match NamespaceMetadata::from_object(object)
        .ok()
        .map(|metadata| metadata.s3_generic_evidence().clone())
    {
        Some(MetadataField::Present(generics)) => generics.into_iter().collect(),
        Some(MetadataField::Missing)
        | Some(MetadataField::Invalid(_))
        | Some(MetadataField::UnsupportedSchema { .. })
        | None => BTreeSet::new(),
        _ => BTreeSet::new(),
    }
}
```

The caller still decides how missing metadata, invalid schemas, base-generic
catalogs, shadowing, and library precedence should affect its provider.

## Closure prefix inspection

The decoder also contains a crate-private bounded inspector for consumers that
need to classify a serialized object before materializing it. For a closure it
walks attributes, environment fields, and the formal/default pairlist while
sharing strict decoding's reference registration/resolution, encoding, depth,
and element accounting. It also applies inspection-specific byte and
formal-count limits.
It reports formal names in wire order (including `...`, duplicate names, and
non-syntactic UTF-8 names), distinguishes missing defaults from present
defaults including `NULL`, and stops immediately after observing the body tag.
The body payload is intentionally not validated. Prefix failures retain their
phase and byte offset in an unavailable result after the root kind is known;
failures before the root flags remain top-level errors. This is an internal
inspection boundary, not a general R object walker or a replacement for
`parse`. Deterministic plain and compiler-produced format-2/format-3 fixtures,
along with generated diagnostic, ALTREP, S4, namespace, and persisted-reference
cases, are produced in the source repository by
`tests/fixtures/generate_closure_inspection_fixture.R`.

## Runnable examples

```text
cargo run -p rd-rds --example inspect_packages -- /path/to/PACKAGES.rds
cargo run -p rd-rds --example inspect_rds -- /path/to/archive.rds
```

`inspect_packages` demonstrates the typed, stable package-index view.
`inspect_rds` provides a bounded advanced inspection of unfamiliar decoded
objects, including shapes that are not package matrices.

## Repository-index interoperability

The supported contract is the tested decoding behaviour described in the
[workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md),
not the continued availability or unchanged schema of files hosted by third
parties. Deterministic fixtures cover these CRAN profiles:

- `src/contrib/PACKAGES.rds`: xz envelope, serialization format 2, and the
  17-column main-index schema.
- `src/contrib/Archive/<package>/PACKAGES.rds`: gzip envelope, serialization
  format 3, and the 15-column package-archive schema.
- `src/contrib/Meta/archive.rds`: gzip envelope, serialization format 3, and
  a named list of `file.info()`-shaped data frames.

Real CRAN examples were compared cell-for-cell with R 4.6.1 `readRDS()` on
2026-08-04; decoded cell values matched in all three profiles. R-universe was
manually verified on 2026-08-05: source indexes used gzip, the Windows and
macOS binary-repository indexes used zstd, and the observed schema had 15
columns with `SHA256` in place of CRAN's `MD5sum`. These observations fall
within the reader's general matrix, encoding, and compression behaviour, but
the test suite contains no R-universe-specific fixture.

These statements describe observed interoperability at the stated dates. They
do not guarantee that an external service retains the same paths, schemas,
compression, or serialization behaviour.

### Upstream archive semantics

These are upstream semantics, not reader guarantees. As observed on
2026-08-04, CRAN's per-package `Archive/<package>/PACKAGES.rds` excludes the
current package version, and its rows are in archival rather than
semantic-version order. Consumers must not infer inclusion of the current
release or version precedence from row position. This is a recently
introduced and undocumented CRAN facility and may change or disappear
independently of `rd-rds`.

### String encoding metadata

`RStr::encoding()` reports the CHARSXP encoding flag stored in the serialized
data. R-universe files are generated by a JavaScript serializer rather than by
R, and currently flag every string as UTF-8, including ASCII-only strings,
while R's `Encoding()` reports those strings as `"unknown"` after `readRDS()`.
Encoding labels can therefore differ even when decoded string contents are
identical; this is not a decoding incompatibility.

R serialization format 2 has no native-encoding field in its header. A
non-ASCII CHARSXP marked Native is therefore ambiguous. The reader preserves
the bytes lazily for retained `RStr` values rather than guessing a locale: with
the default policy, conversion by `RStr::as_str()` or a typed view rejects the
value. A `SYMSXP` print name is converted during parsing instead, so a symbol
name that cannot be decoded fails with `Error::InvalidSymbolName` at parse time
under either policy. A caller with an independent UTF-8 contract may opt in with
`ReadOptions::native_encoding_policy(NativeEncodingPolicy::AssumeUtf8)` (or the
corresponding `parse_with_options` API with `ParseOptions`). The opt-in still
validates retained `RStr` bytes with `str::from_utf8` when conversion occurs and
never performs lossy replacement. The policy applies only when the header
field is absent, which means format 2; a format-3 header value is always
authoritative. `RStrValue::native_encoding_source()` distinguishes a
header-declared encoding from a caller assumption, while
`RStrValue::header_native_encoding()` reports header evidence only.

Unknown or unsupported SEXP values reachable from the decoded result are
hard decode errors; they are never silently converted to a known value. The
one exception is environment internals: environments are collapsed to opaque
handles, and a limited set of verified value shapes inside them (for example
complex, raw, and S4 objects) is wire-consumed and discarded rather than
rejected. Decoder defaults are a depth limit of
5,000, a vector limit of 8,000,000 elements, and a total-element limit of
16,000,000, plus a reference-table limit of 16,000,000 entries. The
reference-table cap can be tightened independently with
`Limits::max_references`; it is checked before a decoded reference is
registered. The file layer defaults to 256 MiB compressed and decompressed
input caps.

`RObject` and `RValue` access is a supported advanced API. Their fields are
encapsulated and accessed through constructors and accessors. Enum variants
may be added in minor releases, so consumers must use wildcard match arms;
the public enums are non-exhaustive. The typed `package` and `matrix` views are
the stable convenience surface for ordinary consumers.

## Stability

Typed package-metadata views are the recommended supported surface. The `RObject`/`RValue` object model is supported as an advanced surface, with variants subject to addition; unsupported SEXPs are hard errors except for selected environment internals consumed as opaque or discarded wire data. See the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).

## License

MIT; see [the workspace license](../../LICENSE).
