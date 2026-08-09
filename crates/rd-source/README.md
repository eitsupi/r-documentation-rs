# rd-source

`rd-source` parses R documentation source files and produces the shared `rd_ast::RdDocument` model. It is a source producer rather than a second, source-specific document representation.

## Parsing and recovery

The parser accepts UTF-8 Rd source, including R-like and verbatim contexts, comments, groups, options, and recognized Rd tags. A successful parse returns `Parsed`, containing the document and any diagnostics. Recoverable syntax problems are reported as diagnostics with `Severity::Warning` or `Severity::Error` while preserving a recovered document. Fatal input or resource failures return `ParseError` and no partial `Parsed` value; these include invalid UTF-8, embedded NUL bytes, an input larger than 64 MiB, unsupported encoding declarations, and excessive parser nesting.

## Resource limits

The current implementation limits complete input to 64 MiB and parser frame nesting to 128 levels. These are implementation limits, not user-configurable parser options.

## Conformance tests

The curated oracle corpus is catalogued in [`tests/fixtures/cases.toml`](tests/fixtures/cases.toml) and contains 76 cases at the time of writing. The committed fixtures are generated with pinned R scripts and compared with `tools::parse_Rd`; the parser's canonical output is lowered to the same `rd_ast::RdDocument` model used by other producers.

The mass differential test is opt-in and ignored by default. Run it by supplying `RD_SOURCE_CORPUS` as colon-separated roots of `.Rd` files and selecting the test explicitly, for example:

```text
RD_SOURCE_CORPUS=/path/to/R/src/library cargo test -p rd-source --test corpus_differential -- --ignored
```

## Stability

Parsing behavior and the fatal-versus-recoverable split are specified by [`CONTRACT.md`](CONTRACT.md), which requires parity with the pinned `tools::parse_Rd` oracle for grammar-native Rd syntax within the documented scope. Patch releases preserve parse results, diagnostic codes, spans, and recovery positions that already conformed to the previous release's contract, and may correct them where the previous behavior violated it. Diagnostic message text is not a machine-readable contract and may change in any release. See the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).

## License

MIT; see [the workspace license](../../LICENSE).
