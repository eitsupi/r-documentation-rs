# Stability policy

This policy applies to all five workspace crates, which release in lockstep at the same `0.2.x` version.

## Compatibility

Breaking source or public API changes require a minor version bump. Patch releases preserve source and public API compatibility.

Observable behavior covered by a crate's normative contract is also preserved across patch releases, with one exception. A patch release may correct behavior that did not conform to the normative contract published with the **previous** release. Such a conformance correction must be demonstrated against the same pinned external oracle version, must not add or revise a normative rule or an intentional deviation, and must change behavior only from a contract-forbidden result to the result that contract requires.

Changing a normative contract, changing the pinned oracle version, adding or removing an intentional deviation, resolving previously provisional or unspecified behavior, or choosing differently among several contract-permitted results requires a minor release. A maintainer may also use a minor release for an unusually broad conformance correction.

Classification always uses the contract as published in the previous release, never a contract edited for the release being prepared. Editing a contract and then declaring the implementation a patch-level conformance fix is not permitted.

Serialized representations are not covered by these rules and carry only the guarantees stated per crate below; `rd-ast`'s serde representation in particular is same-version round-trip only. Breaking changes, MSRV raises, and feature or default changes are recorded in [CHANGELOG.md](CHANGELOG.md). Enums marked `#[non_exhaustive]` require wildcard match arms, and adding variants to them is not treated as a breaking change.

`rd_source::unstable_rlike` is an implementation-sharing surface between the workspace crates. It is public only because Rust has no narrower visibility across crate boundaries, and it is excluded from every guarantee in this policy: it may change or disappear in any release, including a patch release. External consumers must not use it.

## Minimum supported Rust version

The MSRV is Rust 1.88 for all crates and all feature combinations, declared via the workspace `rust-version`. MSRV raises are allowed and are recorded in the changelog.

## What “supported” means

Supported means intended and covered by tests in the current version. It does not expand the compatibility guarantees stated above.

## Per-crate classification

### rd-rds

Supported: typed package-metadata and repository-index views exercised by deterministic tests are the recommended surface. References to concrete third-party files describe verified interoperability profiles, not guarantees that upstream paths or schemas remain available or unchanged. Supported (advanced): the `RObject`/`RValue` object model, with variants subject to addition. Documented scope: unsupported SEXPs are hard errors, with a documented exception for selected environment internals consumed as opaque or discarded wire data.

### rd-helpdb

Supported: alias, topic, search, vignette, and demo reading for an explicitly named installed-package directory. Out of scope: R library discovery, including finding libraries or packages on a machine.

### rd-ast

Documented contract: the canonical model, producer obligations, and `Raw`/`Unknown` preservation rules in [crates/rd-ast/CONTRACT.md](crates/rd-ast/CONTRACT.md). Evolving: the fine-grained semantic view APIs, whose breaking changes require a minor version bump. The [serde representation shape](crates/rd-ast/CONTRACT.md#11-stability) is for same-version round-trip only and is not a stable interchange or storage format.

### rd-source

Supported: UTF-8 Rd source parsing to `rd-ast` documents and the fatal-versus-recoverable split. For grammar-native Rd syntax within the documented scope, behavior must match the pinned `tools::parse_Rd` oracle, except where `rd-ast` requirements, deliberate deviations, or provisional behavior are documented in the [normative contract](crates/rd-source/CONTRACT.md), which is versioned with the crate.

Patch releases preserve parse results, diagnostic codes, spans, and recovery positions that already conformed to the previous release's contract. They may correct any of those when the previous behavior violated that contract, under the conformance-correction rule above. Every other change to them requires a minor release.

Diagnostic message text is not a machine-readable contract and may change in any release, as stated in [CONTRACT.md](crates/rd-source/CONTRACT.md). Diagnostic code variants and their documented meanings are covered by the guarantees above even though the diagnostic types are `#[non_exhaustive]`.

### rd-writer

Supported: strict, faithful serialization of serializable `rd_ast::RdDocument` values, including its conditional parse-back equality guarantee. Documents with unsupported or unrepresentable shapes are reported as errors; formatting and byte identity are not guaranteed.

Patch releases may change emitted spelling and formatting, and may correct output that was not a faithful representation of the input document. Making a previously supported, contract-conforming document unserializable, or weakening the documented parse-back equality guarantee, requires a minor release.

For `WriteError::Unserializable`, `WriteError::ast_path()` returns the canonical
location in the input `RdDocument`. Document nodes use `TopLevel(i)`, tagged and
group children use `Child(i)`, and option children use `Option` followed by
`Child(i)`. Errors are anchored to the narrowest responsible AST node, or to the
containing document, tagged node, group, or option when no single child is
responsible.
