# Rd AST producer/consumer contract

This is the normative contract for `rd-ast` v1. **MUST**, **MUST NOT**,
**REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**,
**RECOMMENDED**, **MAY**, and **OPTIONAL** have the RFC 2119/8174 meanings
when capitalized. A type-enforced guarantee is distinct from a correct
producer obligation: constructors accept arbitrary child shapes, so most
producer obligations are not compiler-enforced.

Every clause is marked as follows:

- **[AST contract]** producer-agnostic requirement.
- **[RDS producer profile]** behavior only of the `rds`-feature lowering.
- **[Consumer guidance]** renderer recommendation, not a structural guarantee.
- **[Non-contract note]** observation, corpus guard, or future plan.

## 1. Scope and terminology

**[AST contract]** `RdDocument` is the common target of the
help-database lowering and the `rd-source` Rd source parser. Both producers MUST
express the same canonical semantics, and consumers MUST NOT need to know
which producer created a document.

**[AST contract]** Canonical nodes are `Text`, `RCode`, `Verb`, `Comment`,
`Tagged`, and `Group`. `Raw` is the lossless escape hatch for a genuinely
non-canonical structure. A view is a borrowed interpretation, never
replacement storage.

**[Non-contract note]** The crate is independent of both RDS decoding and a
source parser. Source locations are outside the v1 AST.

## 2. Conformance model: canonical nodes and Raw fallback

**[AST contract]** An unknown tag MUST become `RdNode::Tagged` with
`RdTag::Unknown` containing the original tag string. Unknown spelling alone
MUST NOT produce `Raw`.

**[AST contract]** `Raw` MUST be reserved for genuinely non-canonical
structures (for example unexpected attributes or an untagged non-list value).
When used, `Raw` preserves opaque node payloads as well as children and
attributes; a producer MUST NOT silently discard a value merely because it
cannot be represented as Rd children.

**[RDS producer profile]** The RDS lowering maps unrecognized `Rd_tag` strings
to `Unknown`; malformed/duplicated tag attributes and rejected attribute
shapes use lossless `Raw`. `srcref` is discarded only on a successful
structured path; Raw retains it and nested attributes.

## 3. Text and escape semantics

**[AST contract]** Canonical strings MUST contain the canonical leaf value
defined by the applicable producer profile. When an input format has escape
spellings, a producer MUST apply that format's grammar- and context-specific
escape rules: it MUST decode a spelling when the grammar resolves it to a
literal character, and MUST retain the spelling when the grammar makes that
spelling part of the canonical leaf value. A producer MUST NOT decode a
spelling that its grammar preserves or reintroduce a spelling after resolving
it to a literal character. The AST contract does not impose a blanket rule
that every recognized escape spelling is decoded.

**[RDS producer profile]** The pinned `parse_Rd()` fixtures contain both
decoded literal characters and contextually retained escape spellings. RDS
lowering MUST preserve the resulting leaf string exactly and MUST NOT perform
an additional escape-decoding or escape-encoding pass.

**[AST contract]** Producers MUST NOT apply Unicode normalization.

## 4. Unicode, encoding, and NA

**[AST contract]** Canonical `String` values MUST be valid UTF-8. Producers
MUST convert supported input encodings and MUST NOT use lossy replacement.

**[RDS producer profile]** Latin-1 input is converted to UTF-8. ASCII Native
and Native explicitly identified as UTF-8 are accepted; non-UTF-8 Native,
invalid UTF-8, and Bytes are hard `LowerError::InvalidStringEncoding`
failures.

**[RDS producer profile]** In a structured character leaf, `NA` elements are
omitted and decoded elements are concatenated in order. In a `RawRdValue`
character or persisted vector, each `NA` remains `None` at its original
position.

**[Non-contract note]** `RawRdReal::Finite` being finite is not
type-enforced, although the RDS lowering classifies non-finite inputs into
dedicated variants.

## 5. Ordering, whitespace, and comments

**[AST contract]** Correct producers MUST preserve source order of document
nodes, children, options, and Raw vector elements.

**[AST contract]** Producers MUST NOT trim whitespace or merge text nodes.
Whitespace-only `Text` nodes are retained content.

**[AST contract]** Comments supplied by a producer MUST remain
`RdNode::Comment` nodes at their source position. A producer emitting one MUST
include its leading `%`; the `Comment(String)` constructor cannot enforce it.

**[RDS producer profile]** Comment preservation depends on the input object:
`parse_Rd()` must retain comments (for example via `keep.source`) for lowering
to receive them. Lowering does not invent absent comments.

**[Consumer guidance]** The 0.4.x `text_contents` projection skips comments
and is lossy. It is named `text_contents_lossy` in 0.5.0. Exact
whitespace/comment rendering SHOULD walk the tree.

## 6. Positional groups

**[AST contract]** Every untagged positional-argument group MUST be
`RdNode::Group`, not `List` or `Raw` merely because it has no tag.

**[RDS producer profile]** An untagged list-valued object with no rejected
attributes and no `Rd_option` lowers to `Group`, including nested groups.

## 7. Options

**[AST contract]** `RdTagged::option` is the lossless syntax layer and MUST be
canonical storage. `None` means absent; `Some(vec![])` means present and
empty; producers MUST preserve that distinction.

**[AST contract]** `RdOptionList` and typed option views are consumer views and
MUST NOT replace or mutate syntax-layer storage.

**[AST contract]** A parsed `RdOptionList` retains access to its original
positioned option or body sequence through `nodes_ref()` and `range()`. Pair
indices are parser-local metadata, not `Child` path segments or substring
source spans. `Sexpr` options use an `Option` container; `RdOpts` bodies use
the tagged node's child container.

**[Consumer guidance]** Consumers SHOULD retain syntax nodes for round-trips
and diagnostics, and use typed views for parsed pairs and typed overrides.

**[RDS producer profile]** `Rd_option` list content is lowered into the option
field; malformed option shapes remain losslessly Raw rather than disappearing.
In particular, a malformed non-list/non-character `Rd_option` value survives
as a Raw option node with its value in the Raw payload.

## 8. Unknown tags and Raw handling

**[Consumer guidance]** Renderers SHOULD dispatch known tags and provide an
explicit fallback for `Unknown`, retaining its option and children. They
SHOULD NOT drop an unknown-tag subtree.

**[Consumer guidance]** A renderer seeing Raw SHOULD preserve or diagnose its
children, payload, and attributes and SHOULD NOT claim semantic rendering of
the opaque shape. Raw is not the unknown-tag category.

**[AST contract]** System-macro sequence views are the sole v1 exception to
the rule that semantic views never interpret Raw nodes: they may interpret
only a Raw `USERMACRO` marker satisfying the corpus-pinned raw-classification
guard and a structurally validated documented definition/expansion profile. All other Raw
nodes remain opaque.

## 9. Malformed-tree tolerance and validation boundary

**[AST contract]** AST data types MUST carry malformed or unexpected child
shapes losslessly; public constructors intentionally do not validate shapes.

**[Consumer guidance]** Lossy accessors MAY skip mismatches. Consumers needing
diagnostics SHOULD use `inspect_*`, which reports `RdShapeError` without
changing the tree.

**[Consumer guidance]** Singleton accessors are lossy and first-wins on
duplicates; strict counterparts report duplicates as errors. Repeatable
sections preserve source order. Custom section traversal is depth-first
preorder, and `nesting` is syntactic information rather than rendering policy.
Matching `Raw` nodes are strict errors and MUST NOT be interpreted. Orphan
subsections are outside the section view's scope.

**[AST contract]** System-macro views operate on sibling sequences. Curated
`Doi`, `CranPkg`, `Sspace`, and `I` nodes consume one sibling; a validated
help-database `USERMACRO` `doi`, `CRANpkg`, or `sspace` representation consumes
the marker and its one exact expansion sibling. Lossy traversal collapses only
fully validated sequences and otherwise keeps every original node visible;
strict traversal reports definition, expansion, and unsupported-representation
mismatches without consuming an unvalidated expansion. The v1 help-database
projection does not recover `\I`, whose expansion is spliced into surrounding
text without a recoverable sibling boundary. Curated `RdTag::I` remains
available through this view family.

**[AST contract]** Link/list/table/equation/option validation belongs to views
and MUST NOT be inferred from `RdTagged` alone.

**[AST contract]** Example-control views preserve the direct children and exact
tag identity of `\dontrun`, `\donttest`, `\dontshow`, `\dontdiff`, and
`\testonly`.

**[Consumer guidance]** These views classify source-level wrapper shape only —
execution, visibility, testing, output-comparison, and wrapper-pattern
heuristics belong to consumers. `\dontshow` and `\testonly` remain distinct
kinds and MUST NOT be normalized together.

**[AST contract]** Inline-span views classify exactly `\emph`, `\strong`,
`\bold`, `\code`, `\special`, `\verb`, `\url`, `\email`, `\file`, `\pkg`,
`\samp`, `\sQuote`, `\dQuote`, `\kbd`, `\var`, `\env`, `\command`,
`\option`, `\acronym`, `\abbr`, `\cite`, and `\dfn`. They preserve direct
children and exact tag identity and assign no typography, quoting,
code-formatting, or URL behavior. One-argument macros lose their argument
group in the AST, so empty and missing arguments are not distinguished. The
views never interpret matching `Raw` nodes.

**[AST contract]** Text-symbol views preserve the exact spellings `\R`,
`\dots`, and `\ldots`, with plain-text fallbacks `"R"` and `"..."`.
Typographic substitution is consumer policy, and the views never interpret
matching `Raw` nodes.

**[AST contract]** Conditional views expose `\if` as exactly two and `\ifelse`
as exactly three positional groups with an exact `Text`-scalar format
projection. Branch selection is consumer policy and branch contents are not
validated. `\enc` exposes exactly two groups — the encoded and ASCII
alternatives — and encoding-side selection is consumer policy. The method family (`\method`, `\S3method`, `\S4method`)
preserves exact spelling and two `Text`-scalar arguments; the following usage
signature is not part of the view. All three views never interpret matching
`Raw` nodes.

**[AST contract]** Figure views expose one or two verbatim groups with exact
`Verb`-scalar projections. The expert-options discriminator is the
`options:` prefix followed by whitespace; attribute parsing and image alt
policy are consumer policy. In an R-like `\figure`, the view owns only its
first brace argument and MUST NOT absorb the sibling second brace. S4-class
link views preserve class contents and optional package syntax without
generating `-class` topics or resolving links; the bracket option is not
restricted to a single `Text` node. Both views never interpret matching
`Raw` nodes.

**[Consumer guidance]** The generation-header view scans only leading top-level
`Comment` nodes interleaved with whitespace-only `Text`. A generated marker
must exactly conform to `% Generated by <name>: do not edit by hand`, with a
non-empty name containing no colon or line break. `roxygen2` maps to the
first-class `Roxygen2` generator; any other conforming name is preserved
verbatim as `Unknown`, and unknown generators still count as generated. The
first recognized marker determines the generator. Source-file lines and `%   `
continuations remain the exact roxygen convention understood by this view.
Source paths are borrowed, retain source order and spelling, and are not
normalized. The aggregate header has no fabricated path: `generator_path()`
identifies the marker comment and `source_origins()` associates every source
file value with the comment that produced it. Non-leading, nested, and
near-matching header text are not interpreted. Roxygen fenced-code-block
recognition remains converter policy outside this view.

**[Consumer guidance]** Lifecycle-badge views scan every canonical top-level
`\description` (an explicit exception to the singleton first-wins rule) and
report all lifecycle-named figures in depth-first source order. Filename
recognition uses a case-sensitive `lifecycle-` basename prefix,
case-insensitive known-stage classification, and accepts any or no extension;
unknown stage spellings remain explicit. Canonical badge-shape evidence is
optional and does not change permissive recognition. Traversal follows
canonical `Tagged` and `Group` children but not options or `Raw` content;
matching `Raw` nodes and malformed lifecycle figure candidates are diagnostics
in strict inspection and are never interpreted.

## 10. Structural locations and inspection API

The following rules define the 0.5.0 location and inspection boundary. They
are intentionally separate from the 0.4.x implementation names; the complete
source-level migration inventory is in
[`docs/rd-ast-0.5-migration.md`](../../docs/rd-ast-0.5-migration.md).

**[AST contract]** `RdAstPath` is the canonical, producer-independent path
through an `RdDocument`. Its segment vocabulary is `TopLevel(usize)`,
`Child(usize)`, and `Option`; the empty path is the document root. Valid
structural forms are the empty root or one `TopLevel` followed by zero or more
node steps. A node step is `Child`, or `Option` optionally followed by
`Child` steps. `TopLevel` is valid only at the document boundary, `Child` is
valid only in the current node or option container, and an option cannot be
followed directly by another `Option`. The referenced container and index
MUST also exist in the document snapshot. A path ending in `Option` identifies
a present option container and does not identify a node; an option child is
reached with `Option` followed by `Child(usize)`.

**[AST contract]** `RdAstPath` is snapshot-local. A path is meaningful only
for the document snapshot from which it was obtained. The type does not and
need not prevent applying a path to another document or to a later edited
snapshot. It is suitable as a `Hash`/`Eq`/`Ord` key, while its display form is
diagnostic text rather than a serialized protocol.

**[RDS producer profile]** RDS lowering positions such as attributes,
attribute values, list elements, and character-vector elements belong to a
separate `LowerPath` used by `LowerLocation`. They MUST retain their detail in
lowering errors and MUST NOT be silently converted into `RdAstPath` segments.

**[AST contract]** `RdNodeRef`, `RdNodesRef`, and `RdOptionRef` carry borrowed
nodes or node sequences together with their canonical location. Slicing a
`RdNodesRef` MUST preserve absolute sibling indices: a slice of `3..5` has a
first item at `Child(3)`, including after further slicing. `RdSiblingRange` is
an absolute half-open sibling range within one container, may be empty, and is
also snapshot-local. It is not a source range and cannot span containers.

**[AST contract]** The structural `RdDocument::walk` traversal visits every
stored `RdNode` exactly once in preorder. For each node, descendants in a
present option are visited before descendants in the node's children, and
each subtree is traversed before the next sibling. This applies to `Raw` as
well as `Tagged`, `Group`, and `Unknown` nodes. Raw payloads and attributes
are not `RdNode` values and are not traversed. The walk is not a semantic Raw
pruner: a consumer that treats Raw as opaque MUST recurse explicitly and stop
at the Raw node. A flat loop with `continue` does not prune later descendants.

**[AST contract]** Semantic inspection is cursor-based in 0.5.0. Public
node-level inspection MUST obtain its location from `RdNodeRef`; the API MUST
NOT invite callers to attach an arbitrary canonical path to a detached node.
An isolated test fixture MAY use a one-node document. Detached inspection with
a canonical path is not part of this contract.

**[AST contract]** Strict methods retain the `inspect_*` naming. Every
node-level cursor inspector has the result family
`Result<Option<View>, RdShapeError>` or
`Result<Option<View>, RdOptionError>`, with lifetime parameters as required
by the borrowed view. A nonmatching node MUST return `Ok(None)`. A matching
malformed or `Raw` node MUST return the applicable shape or option error,
including for former `RdTagged` methods whose wrong-tag result was an error.
Document-level inspection and stateful sequence iterators retain their
separately documented result families.

Best-effort accessors that skip, flatten, or select first values gain a
`*_lossy` name. Their existing first-wins, source-order, skip, and flatten
behavior remains unchanged. `text_contents_lossy` explicitly remains a
comment-skipping and markup-flattening projection. `as_*` variant tests and
deliberately scalar-only projections are not renamed solely because they
return `Option`.

**[AST contract]** A view representing one node exposes `path()`. A view
representing a sequence exposes its positioned node sequence and a
`RdSiblingRange`. A view representing multiple consumed nodes exposes an
`anchor_path()` only as a diagnostic anchor and separately exposes the source
nodes or range. Table rows/cells, delimited items, and system-macro matches
MUST NOT claim that one path represents their whole multi-node value. Empty
cells MUST use an empty range and an existing anchor where available; they
MUST NOT invent a child node. A general `RdLocated` trait is not required.

**[AST contract]** `RdTableRow::nodes_ref()` covers the row body and internal
`Tab` separators but excludes its terminal `Cr`; `RdTableCell::nodes_ref()`
covers the cell body in the table body container. `RdDelimitedItem` exposes
the marker anchor, body sequence, and marker-plus-body `source_nodes()`.
`RdSystemMacroMatch` exposes the first-node anchor and all consumed siblings
through `source_nodes()`.

**[AST contract]** `leaf_byte_range()` is a byte range within a canonical
UTF-8 leaf, not an original source byte range. A multibyte character occupies
its full canonical byte interval; for example, in `léc`, the interval for
`é` is `1..3`. Producer character-vector indices and source-map byte ranges
are separate coordinates.

**[Consumer guidance]** Successful inspection validates only the requested
view shape. It does not validate the complete document, consume every markup
node, or replace parser diagnostics. Consumers MUST keep Raw opaque unless a
documented view explicitly permits a validated Raw sequence.

## 11. Source spans

**[AST contract]** The v1 AST has no source-span or `srcref` API. Consumers
MUST NOT rely on spans in `RdDocument` or `RdNode`.

**[AST contract]** A source parser MAY provide a separate public opaque source
map beside its `Parsed` result. Such a map is not part of `RdDocument` or
`RdNode` and does not change this no-spans rule.

**[RDS producer profile]** Raw-embedded `srcref` is producer provenance for
Raw losslessness, not a source-span API. Recognized `srcref` is discarded on
successful structured lowering.

## 12. Stability

**[AST contract]** Rust consumers MUST account for `#[non_exhaustive]` enums
and SHOULD use public accessors rather than private layout. Additive variants
or fields are permitted by this policy.

**[Non-contract note]** Serde-enabled AST values round-trip within the same
crate version, as asserted by the serde tests. This serialization is
unversioned persisted interchange: cross-version wire compatibility is
explicitly not guaranteed, and it is unsuitable for long-lived storage until
a schema/version envelope exists. Borrowed semantic views and the typed option
layer have no serde support and are outside serde scope. The JSON wire shape
is independent of the R-level tag spellings returned by `RdTag::as_rd_tag`.

**[Non-contract note]** `RawRdReal::Finite` is not type-enforced; its
finite-value invariant is producer behavior only.

## 13. Producer-profile equivalence checklist

**[AST contract]** Help-DB lowering and the `rd-source` parser are equivalent
for canonical semantics when they agree on leaf kinds and canonical leaf
strings, tag identity (including exact Unknown spelling), option
presence/content, group boundaries, order, supplied comments, and whitespace
placement.

**[AST contract]** This equivalence is scoped to canonical semantic nodes.
Raw provenance payloads and Raw internals are excluded, including the
corpus-pinned `USERMACRO` Raw shape and `RawRdValue` internals.

**[Non-contract note]** General `USERMACRO` remains non-canonical and its
corpus classification stays a regression guard. The four curated system-macro
profiles have the section-8/section-9 sequence-view treatment; help-database
`\I` is not recovered.
