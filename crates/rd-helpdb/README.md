# rd-helpdb

`rd-helpdb` reads the compiled help database of an installed R package and provides alias, topic, topic metadata, help-search, vignette, and demo access.

## Overview

An installed package help database consists of `help/aliases.rds`, the `help/<pkg>.rdx` index and `help/<pkg>.rdb` record pair, `Meta/hsearch.rds`, and optional `Meta/vignette.rds` and `Meta/demo.rds` indexes. `rd-helpdb` reads the standalone RDS files, exposes validated typed vignette and demo entries, and reads the compressed records addressed by the `.rdx` index; it does not discover package directories through R's `.libPaths()`.

Standalone `.rds` envelope handling and the bounded `.rdx`/`.rdb` reader are
delegated to [`rd-rds`](../rd-rds/README.md). `PackageHelpDb` keeps the public
topic, reference, and alias behavior of earlier releases, including index
order and last-wins lookup. The compatibility helpers `read_rds_file` and
`decode_rdb_record` remain available for callers that need the lower-level
adapters. Consumers that need a canonical document model can lower decoded
help objects into [`rd-ast`](../rd-ast/README.md).

## Topic metadata without compiled help

`HelpTopicIndex::read_installed(pkg_dir)` reads `Meta/Rd.rds` without opening
the help database or `aliases.rds`. It returns `None` for a missing file,
`Some(index)` for present metadata (including zero rows), and an error for
I/O, RDS decoding, or required-schema failures. Use
`read_installed_with_options` to customize the underlying `rd-rds` file and
decode limits.

The view preserves row order and alias groups, including duplicate and NA
aliases. `find_alias` returns the first matching metadata row. This is
independent of `PackageHelpDb::resolve_alias`, which reads `aliases.rds` and
uses the last duplicate occurrence. Missing names, titles, or file names in
the first matching row do not cause lookup to select a later row.

Each entry's `name`, `title`, and `file` distinguish absent columns, NA values,
malformed fields, and decoded text through `HelpTopicText`. Invalid optional
fields leave other metadata available; `as_str()` returns only usable text.
The required `Aliases` column and data-frame structure are validated.
`topic_key()` takes the basename of the stored `File` value and strips one
`.Rd` or `.rd` suffix, matching R's help-database keys. It preserves the
stored value and does not check whether the resulting topic exists.

The [help-with-fallback example](examples/help_with_fallback.rs) combines
metadata and compiled help. Its consumer policy keeps the metadata title
when the help files are absent or a selected topic fails to decode:

```sh
cargo run -p rd-helpdb --example help_with_fallback -- /path/to/package alias
```

## Features

The `xz`, `bzip2`, and `zstd` features are enabled by default and control
optional compression envelopes accepted for standalone `.rds` files such as
`aliases.rds` and the `Meta/*.rds` indexes. Gzip support is always effective
for installed-package `.rdx` indexes: the mandatory `rd-rds/lazyload` feature
includes `rd-rds/gzip` because that is the normal installed-package profile.

The `gzip` feature remains as a compatibility forwarding feature for callers
that name it explicitly, but enabling it does not change the effective codec
set when `lazyload` is enabled. A minimal build that keeps gzip for help
databases while omitting the optional standalone codecs can therefore use:

```toml
[dependencies]
rd-helpdb = { version = "0.5.0-rc.1", default-features = false }
```

The internal `rd-rds/lazyload` feature is always enabled because compiled help
databases use that bounded reader; it includes the gzip envelope needed by
normal installed-package `.rdx` files. The standalone codec features still
control the additional `.rds` envelopes, while `.rdb` record decompression is
mandatory for this format and is always compiled in. Thus
`--no-default-features` removes the optional xz, bzip2, and zstd standalone
codecs (and does not remove the required zlib record decoder).

## Stability

Alias, topic, topic metadata, search, vignette, and demo reading for an explicitly named installed-package directory is supported. Discovering R libraries or packages on a machine is out of scope; see the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).

## License

MIT; see [the workspace license](../../LICENSE). See the [workspace README](../../README.md) for repository status and layout.
