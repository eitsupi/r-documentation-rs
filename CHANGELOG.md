# Changelog

## [Unreleased]

## [0.1.0-rc.1] - 2026-07-24

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
