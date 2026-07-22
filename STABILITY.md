# Stability policy

This policy applies to all five workspace crates, which release in lockstep at the same `0.0.x` version.

## 0.0.x compatibility

Between `0.0.x` releases, there is no source, API, or serialized-representation compatibility guarantee. External consumers should pin an exact `0.0.z` version or use a lockfile. Each release documents the behavior it intends and tests as its documented contract. Breaking changes, MSRV raises, and feature or default changes are recorded in [CHANGELOG.md](CHANGELOG.md). Enums marked `#[non_exhaustive]` require wildcard match arms, and adding variants to them is not treated as a breaking change.

## Minimum supported Rust version

The MSRV is Rust 1.88 for all crates and all feature combinations, declared via the workspace `rust-version`. MSRV raises during `0.0.x` are allowed and are recorded in the changelog.

## What “supported” means

Supported means intended and covered by tests in the current version. It is not a cross-version compatibility promise.

## Per-crate classification

### rd-rds

Supported: typed package-metadata views are the recommended surface. Supported (advanced): the `RObject`/`RValue` object model, with variants subject to addition. Documented scope: unsupported SEXPs are hard errors, with a documented exception for selected environment internals consumed as opaque or discarded wire data.

### rd-helpdb

Supported: alias, topic, search, vignette, and demo reading for an explicitly named installed-package directory. Out of scope: R library discovery, including finding libraries or packages on a machine.

### rd-ast

Documented contract: the canonical model, producer obligations, and `Raw`/`Unknown` preservation rules in [crates/rd-ast/CONTRACT.md](crates/rd-ast/CONTRACT.md). Evolving: the fine-grained semantic view APIs during `0.0.x`. The [serde representation shape](crates/rd-ast/CONTRACT.md#11-stability) is for same-version round-trip only and is not a stable interchange or storage format.

### rd-source

Supported: UTF-8 Rd source parsing to `rd-ast` documents and the fatal-versus-recoverable split. Within a release, diagnostics, spans, and recovery behave as specified by the [normative contract](crates/rd-source/CONTRACT.md), which is versioned with the crate. Across `0.0.x` releases, the set of diagnostic codes, diagnostic message wording, and exact span and recovery positions may change, so consumers must not depend on them cross-version. Message text is never a machine-readable contract even though diagnostic types are `#[non_exhaustive]`.

### rd-writer

Supported: strict, faithful serialization of serializable `rd_ast::RdDocument` values, including its conditional parse-back equality guarantee. Documents with unsupported or unrepresentable shapes are reported as errors; formatting and byte identity are not guaranteed.
