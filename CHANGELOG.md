# Changelog

## [Unreleased]

### Changed

- Correct the versioning and stability policy for the `0.1.x` release series: breaking changes require a minor version bump, while patch releases preserve compatibility.
- Share the R-like lexical transition engine between [rd-source] and [rd-writer] through the new `#[doc(hidden)]` `unstable_rlike` surface on [rd-source].

### Fixed

- [rd-source] and [rd-writer] Preserve single-quoted R raw strings, including dashed and uppercase forms, without treating their contents as Rd syntax.
- [rd-source] and [rd-writer] End a pending backslash escape at a newline inside an ordinary quoted string, matching R: the quote closes at the following quote character instead of leaving the group unterminated.
- [rd-source] and [rd-writer] Reset partial raw-string closer progress at a newline, matching R's requirement that a closer be contiguous: `)-` before a newline no longer combines with `--"` after it.

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
