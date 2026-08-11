# Changelog

## [Unreleased]

## [0.3.1] - 2026-08-11

A consumer that does not disable default features is unaffected by this
release: the effective default codec set of every crate is unchanged. The one
configuration whose behavior changes is recorded under Changed below.

### Fixed

- [rd-helpdb] and [rd-ast] depended on [rd-rds] with its default features enabled, so a consumer could not exclude a codec even by disabling default features on its own `rd-rds` dependency: Cargo unions features, and either of these crates re-enabled all four. A binary that never needed xz was therefore linked against the system liblzma and failed to start where that library was absent. Both crates now take `rd-rds` with default features disabled and forward the codec selection instead, so disabling default features on every edge a consumer depends on now excludes the codec (#25).

### Added

- [rd-helpdb] Add `gzip`, `xz`, `bzip2`, and `zstd` features forwarding to [rd-rds]. They select the compression envelopes accepted for standalone `.rds` files only; `.rdb` record zlib decompression is required by the help-database format and remains unconditional, so `--no-default-features` does not drop the `flate2` dependency (#25).
- [rd-ast] Add the same four codec features. They forward only when the `rds` feature is enabled, so a plain AST build still has no [rd-rds] dependency (#25).

### Changed

- [rd-helpdb] and [rd-ast] keep all four codec features in `default`, so a consumer that does not disable default features is unaffected (#25).
- [rd-ast] A consumer that previously used `default-features = false, features = ["rds"]` received all four codecs transitively and now receives none. Such a consumer must name the codecs it needs; otherwise a compressed standalone `.rds` file is reported as an unsupported envelope at runtime (#25).

## [0.3.0] - 2026-08-11

This is a minor release because the value of [rd-writer]'s already-published
`WriteError::Unserializable.path` changed. There are no breaking source or
public API changes: code that compiles against 0.2.0 still compiles, but code
that resolves a write error's `path` against its own document must be updated.

### Changed

- [rd-writer] Report `WriteError::Unserializable`'s `path` as a location in the input document rather than in the writer's traversal. A consumer that resolves these paths against its own `RdDocument` must update: a top-level node is now `TopLevel(i)` instead of `Child(i)`, and the virtual `Child(0)` the writer previously interposed when entering the single argument of a tag whose group the parser flattened is gone, so such a child is now `Child(i)` instead of `Child(0)/Child(i)`. Errors are also anchored to the narrowest responsible node instead of an enclosing container: an invalid option now reports the offending option descendant rather than the option as a whole, a tag carrying an option it does not accept now reports the option location, a wrong-kind positional child reports that child rather than its tag, and a conditional whose body leaves `#endif` off a line start reports the body group. For failures that were already `WriteError::Unserializable`, the reported `UnserializableKind` is unchanged; the `\item` correction below is the only error reclassification. The set of documents that fail to serialize and the bytes emitted for documents that succeed are unchanged (#21).
- [rd-writer] State the canonical path grammar and the error-anchoring rules in [STABILITY.md](STABILITY.md): a path may end in a bare `Option` segment, which identifies the option itself and does not denote an `RdNode`, and `UnrepresentableLeafBoundary` is anchored at the later of the two adjacent leaves. For the 0.3.x series only, the classification of a failed write between `WriteError::Verification` and `WriteError::Unserializable` is declared provisional: a patch release may reclassify `Verification` as `Unserializable` when the input has no faithful Rd source representation, but not the reverse. Several unrepresentable inputs still reach `Verification`, where `ast_path()` returns `None` (#23).

### Fixed

- [rd-writer] A `\item` that was otherwise serializable but carried an option previously had that option silently dropped and failed with `WriteError::Verification` without an AST location. It is now rejected as `WriteError::Unserializable` with `UnserializableKind::InvalidTagShape` at the bare `Option` path. R rejects `\item[...]` in the two-group contexts and treats the brackets as ordinary sibling text in itemize context, so the shape is unrepresentable (#22).

### Added

- [rd-writer] Add `WriteError::ast_path()`, returning the canonical location in the input document for `WriteError::Unserializable` and `None` for `WriteError::Io` and `WriteError::Verification`. This is the supported way to consume an `Unserializable` path without reconstructing the writer's traversal rules from the tag it happens to be writing (#21).

## [0.2.0] - 2026-08-09

This is a minor release because [rd-source] now requires oracle parity for
grammar-native Rd syntax, which its contract previously left open, and because
three inputs consequently parse differently. There are no breaking source or
public API changes. Under the revised policy, later corrections of this kind
may ship in patch releases.

### Changed

- [rd-source] Require a parser implementation to match the pinned `tools::parse_Rd` oracle on acceptance and recovery for grammar-native Rd syntax within the documented scope. The contract previously described the oracle as evidence rather than a requirement, which left the three parse-behavior corrections below formally unspecified.
- Revise the stability policy accordingly: a patch release may correct behavior that violated the normative contract published with the previous release, provided the pinned oracle is unchanged and no contract rule is added or revised. Changing a contract, changing the pinned oracle version, or resolving previously unspecified behavior still requires a minor release (#15).
- Share the R-like lexical transition engine between [rd-source] and [rd-writer] through the new `unstable_rlike` implementation-sharing surface on [rd-source]. The two crates previously carried separate copies of it, which is why the raw-string defect below was present in both and invisible to the round-trip tests. This surface is excluded from every stability guarantee and must not be used by external consumers (#16).

### Fixed

Each of these was verified against R 4.6.1. Affected documents parse to
different nodes or produce different diagnostics after upgrading.

- [rd-source] Treat single-quoted R raw strings (`r'(...)'`, `R'[...]'`, and their dashed forms) as opaque, like the double-quoted forms. A bare `%` inside such a string previously started an Rd comment and a backslash sequence could be read as Rd syntax (#14).
- [rd-source] and [rd-writer] End a pending backslash escape at a newline inside an ordinary quoted string. A backslash at end of line inside a quoted string previously produced a spurious `UnclosedGroup` diagnostic, and the input now parses without it. For [rd-writer], a document that previously failed to serialize with `UnterminatedRLikeState` now serializes successfully (#16).
- [rd-source] and [rd-writer] Reset partial raw-string closer progress at a newline, because R requires a closer to be contiguous. A closer split across a newline previously terminated the raw string, and such input now produces `UnclosedGroup`. For [rd-writer], a document that previously serialized is now correctly rejected with `UnterminatedRLikeState` (#16).
- [rd-writer] Preserve the contents of single-quoted R raw strings instead of escaping them as ordinary quoted content. `f(x = r'(100%\q)')` was emitted as `f(x = r'(100\%\\q)')`, which recovers the R value `100\%\\q` instead of `100%\q` when parsed back, because the Rd layer never unescapes raw-string contents (#14).

### Added

- [rd-rds] Document CRAN and manually verified R-universe repository-index interoperability profiles, including package-index schema and envelope variations.
- [rd-rds] Add deterministic fixtures and tests for `PACKAGES.rds` envelope and schema variations and `Meta/archive.rds` archive-metadata format variation.
- [rd-rds] Add the `inspect_packages` and `inspect_rds` examples for inspecting supported `.rds` files from the command line.

## [0.1.0] - 2026-07-26

### Added

- [rd-rds] Add a validated general character-matrix view with optional dimension names.
- [rd-helpdb] Add typed readers for installed-package vignette and demo indexes.

## [0.0.1] - 2026-07-20

### Added

- [rd-rds] Initial scoped, read-only RDS reader for installed R package information.
- [rd-ast] Initial canonical, producer-agnostic AST for R's Rd documentation format.
- [rd-helpdb] Initial reader for installed R package help databases, including aliases, topics, and help search.
- [rd-source] Initial parser for Rd source files producing `rd-ast` documents with diagnostics and recovery.
- [rd-writer] Initial serializer producing `.Rd` source text from `rd-ast` documents with a strict parse-back guarantee.
