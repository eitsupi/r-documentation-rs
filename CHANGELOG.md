# Changelog

This changelog follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Entries are prefixed with the affected crate name, and the workspace releases all crates in lockstep.

## [Unreleased]

## [0.0.1] - 2026-07-20

### Added

- [rd-rds] Initial scoped, read-only RDS reader for installed R package information.
- [rd-ast] Initial canonical, producer-agnostic AST for R's Rd documentation format.
- [rd-helpdb] Initial reader for installed R package help databases, including aliases, topics, and help search.
- [rd-source] Initial parser for Rd source files producing `rd-ast` documents with diagnostics and recovery.
- [rd-writer] Initial serializer producing `.Rd` source text from `rd-ast` documents with a strict parse-back guarantee.
