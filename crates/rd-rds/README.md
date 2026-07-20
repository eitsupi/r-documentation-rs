# rd-rds

`rd-rds` is a scoped, read-only reader for installed-R-package information. It
is not a general R serialization library and never silently accepts an
unknown SEXP. See the [workspace README](../../README.md) for repository
status and crate relationships.

The API has three layers:

- `parse` reads a decompressed XDR serialization stream only.
- `file::from_bytes` and `file::read` accept the complete envelope and apply
  bounded decompression. Supported envelopes are raw `X\n` XDR, gzip, xz,
  bzip2, and zstd (when the corresponding feature is enabled).
- `package` provides validated convenience views for `Meta/package.rds` and
  CRAN-like `PACKAGES.rds` matrices.

The [`rd-helpdb`](../rd-helpdb/README.md) crate uses the file layer for
standalone help-database RDS files, and [`rd-ast`](../rd-ast/README.md) can
lower supported decoded documentation objects into the common document model.

Unknown or unsupported SEXP values reachable from the decoded result are
hard decode errors; they are never silently converted to a known value. The
one exception is environment internals: environments are collapsed to opaque
handles, and a limited set of verified value shapes inside them (for example
complex, raw, and S4 objects) is wire-consumed and discarded rather than
rejected. Decoder defaults are a depth limit of
5,000, a vector limit of 8,000,000 elements, and a total-element limit of
16,000,000. The file layer defaults to 256 MiB compressed and decompressed
input caps.

`RObject` and `RValue` access is a supported advanced API. Their fields are
encapsulated and accessed through constructors and accessors. Enum variants
may be added in minor releases, so consumers must use wildcard match arms;
the public enums are non-exhaustive. The typed `package` views are the stable
convenience surface for ordinary consumers.

## Stability

Typed package-metadata views are the recommended supported surface. The `RObject`/`RValue` object model is supported as an advanced surface, with variants subject to addition; unsupported SEXPs are hard errors except for selected environment internals consumed as opaque or discarded wire data. See the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).

## License

MIT; see [the workspace license](../../LICENSE).
