# rd-helpdb

`rd-helpdb` reads the compiled help database of an installed R package and provides alias, topic, help-search, vignette, and demo access.

## Overview

An installed package help database consists of `help/aliases.rds`, the `help/<pkg>.rdx` index and `help/<pkg>.rdb` record pair, `Meta/hsearch.rds`, and optional `Meta/vignette.rds` and `Meta/demo.rds` indexes. `rd-helpdb` reads the standalone RDS files, exposes validated typed vignette and demo entries, and reads the compressed records addressed by the `.rdx` index; it does not discover package directories through R's `.libPaths()`.

Standalone `.rds` envelope handling is delegated to [`rd-rds`](../rd-rds/README.md). Consumers that need a canonical document model can lower decoded help objects into [`rd-ast`](../rd-ast/README.md).

## Features

The `gzip`, `xz`, `bzip2`, and `zstd` features are enabled by default. They control only the compression envelopes accepted for standalone `.rds` files such as `aliases.rds`, `<pkg>.rdx`, and the `Meta/*.rds` indexes. For example, a consumer that only needs gzip-enveloped `.rds` files can use:

```toml
[dependencies]
rd-helpdb = { version = "0.3.1", default-features = false, features = ["gzip"] }
```

The `gzip` feature is unrelated to the zlib record stream inside `<pkg>.rdb`. `.rdb` record decompression is mandatory for this format and is always compiled in, so `--no-default-features` does not remove the `flate2` dependency.

## Stability

Alias, topic, search, vignette, and demo reading for an explicitly named installed-package directory is supported. Discovering R libraries or packages on a machine is out of scope; see the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).

## License

MIT; see [the workspace license](../../LICENSE). See the [workspace README](../../README.md) for repository status and layout.
