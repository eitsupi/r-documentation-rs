# `rd-ast` 0.5.0 API migration

This document is the migration contract for the planned 0.5.0 breaking
release. It describes the public API changes required to make structural
locations available to consumers while preserving the canonical Rd syntax
model. It is a design and migration document; it does not change the 0.4.x
implementation.

## Release boundary

The 0.5.0 change is an API change around an existing tree. Producers continue
to build the same canonical nodes (`Text`, `RCode`, `Verb`, `Comment`,
`Tagged`, `Group`, and `Raw`) with the same ordering, whitespace, option
presence, escape, and recovery semantics. The release does not add standard
topic-section classification, associate a `\\method` with a following usage
signature, interpret general user macros, or move the RDS lowering to another
crate.

The release makes structural location explicit. A consumer can obtain a
borrowed cursor for a node, inspect it without manufacturing a path, and keep
the original node or sibling range when a view represents more than one node.
`rd-source` is planned to record a source map beside the document in a
follow-up PR. That source map will be a parser result, not data embedded in
`RdDocument` or `RdNode`.

## Location vocabulary

### `RdAstPath`

`RdAstPath` replaces `RdPath` as the canonical location type. The path
segment vocabulary is:

```text
segment := TopLevel(index) | Child(index) | Option
```

The valid structural forms are the empty root or one `TopLevel(i)` segment
followed by zero or more node steps. A node step is `Child(i)`, or `Option`
optionally followed by `Child(i)` steps for a node inside that option:

```text
path         := root | TopLevel(i) tail
root         := empty
tail         := empty | Child(i) tail | Option option_tail
option_tail  := empty | Child(i) tail
```

These productions describe the segment shape; the referenced container and
index must also exist in the document snapshot. In particular, `TopLevel` is
valid only at the document boundary, `Child` is valid only after a node or
option container, and an option cannot be followed directly by another
`Option`. The empty path is the document root. `TopLevel(i)` selects a
document node; `Child(i)` selects a node in the current tagged, group,
raw-option, or raw-child container; and `Option` selects a present option
container. A path ending in `Option` identifies the option container and does
not identify an `RdNode`.

Paths are snapshot-local coordinates. A path obtained from one
`RdDocument` must not be used as a location in another document or after a
conceptual edit. The type does not attempt to enforce this relationship.
`RdAstPath` is the key type for AST diagnostics, writer errors, cursor
locations, and source-map lookup, and supports `Hash`, `Eq`, and `Ord`. Its display form
is for diagnostics and is not a serialized protocol.

`LowerPath` is a separate producer-internal type. RDS attribute, attribute
value, list-element, and character-vector positions remain available in
lowering errors through `LowerLocation`, but they are not AST paths and must
not be converted automatically into one. In particular, a character-vector
index is not a `Child` index and is not an original source byte offset.

### Cursors and sibling ranges

`RdNodeRef<'a>` combines an `&'a RdNode` with its `RdAstPath`. It exposes the
node, path, option, children, and cursor-based semantic inspection methods.
`RdNodesRef<'a>` represents a node sequence in one container and preserves the
absolute sibling index when it is sliced. For example, slicing children `3..5`
and calling `get(0)` still yields a cursor at `Child(3)`, including after
multiple slices. `RdOptionRef<'a>` represents a present option container and
its positioned children. Absent and present-empty options remain distinct.

`RdSiblingRange` represents an absolute half-open sibling index range in one
container. It may be empty, and it is also snapshot-local. It cannot represent
a path interval across containers. This type is deliberately not named
`RdAstRange`: it is not a source byte range and cannot be passed to a source
map as one.

`document.walk()` is a structural preorder over every stored `RdNode`:

```text
node, option descendants, child descendants
```

Each option subtree is visited before the node's child subtree. The same rule
applies to `Tagged` and `Raw`; Raw's stored option and children are structural
nodes and are visited, while Raw payloads and attributes are not nodes and are
never exposed by the walk. The walk visits every reachable node once and
provides the path that identifies each node in that document snapshot.

The walk is not a semantic Raw filter. A loop that uses `continue` after
seeing a Raw node will still receive its descendants. A consumer that treats
Raw as opaque must recurse through `top_level()`, `option()`, and `children()`
and stop at the Raw boundary. A consumer such as a renderer may instead
inspect Raw's stored children as a fallback. Neither policy changes the AST.

### View locations

Every successful view must expose the location information appropriate to what
it represents:

| View shape | Required location | Meaning |
| --- | --- | --- |
| One tagged node or group | `path()` | The represented AST node |
| A borrowed node sequence | `nodes_ref()` and `RdSiblingRange` | The original container and absolute sibling indices |
| A multi-node match | `anchor_path()` plus source nodes/range | A diagnostic anchor and the complete consumed sequence |
| A text projection | `leaf_byte_range()` when applicable | UTF-8 byte range in the canonical leaf value |
| Several source facts | Per-fact paths | Each comment, marker, or source node that supports the result |

`path()` must not be used to imply that a multi-node view is one node. Table
cells and rows, delimited items, and system-macro matches therefore retain
their range or source-node information; an `anchor_path()` is only a
diagnostic anchor. Empty cells have an empty sibling range and an anchor for
an existing boundary node when one exists. They must not invent a child node.
`RdLocated` is not introduced as a catch-all trait in this release.

Single-node semantic views keep their location-sensitive derived equality: two
otherwise identical views at different canonical paths are unequal. Aggregate
views whose provenance is exposed per fact use their established value
projection for equality; in particular, `RdGenerationHeader` compares its
generator and source-file values while excluding `generator_path()` and
`source_origins()` metadata. `RdSystemMacroMatch` likewise compares its
canonical anchor, semantic value, origin, and consumed count while excluding
the positioned `source_nodes()` metadata. It intentionally does not promise
`Eq`.

`leaf_byte_range()` is measured in the canonical leaf's UTF-8 bytes, not in
the original source bytes. For canonical text `léc`, the byte range of `é` is
`1..3`; source spelling, escapes, CRLF, and decoded text are handled by the
`rd-source` map instead.

## Inspection and lossiness

The normal inspection entry point is a cursor. Public node-level methods that
currently accept `&RdPath` or `&RdPath`-equivalent `base_path` arguments move
to `RdNodeRef` and no longer allow a caller to attach an arbitrary canonical
path to a detached node. Existing private validators may retain path
arguments internally. A standalone fixture can be inspected by putting its
node in a one-element `RdDocument`; that fixture convenience does not make
the resulting path valid for another document.

Strict inspection keeps the `inspect_*` name. Every node-level cursor
inspector has the result family
`Result<Option<View>, RdShapeError>` or
`Result<Option<View>, RdOptionError>` (with the lifetime parameters required
by the borrowed view). A nonmatching node always returns `Ok(None)`. A
matching malformed or Raw node returns the applicable shape or option error;
this includes former `RdTagged` methods whose current wrong-tag result is an
error. Document-level inspection and stateful sequence iterators retain their
separately documented result families. Successful inspection does not
validate the whole document, consume all markup, or change the parser
diagnostics.

The old best-effort names gain a `_lossy` suffix. Their first-wins, skip,
flatten, and text-projection behavior is preserved. Variant tests such as
`as_tagged()` and deliberately text-only projections are not renamed merely
because they return `Option`.

## Complete inspection migration inventory

The left column names the 0.4.x public entry point. The right column describes
its 0.5.0 destination. Accessor names on the resulting view follow the same
rules even where the table groups several methods.

### Document-level views

| 0.4.x entry points | 0.5.0 destination |
| --- | --- |
| `title`, `description`, `usage`, `value`, `name`, `details`, `note`, `author`, `references`, `see_also`, `examples`, `format`, `source`, `encoding`, `doc_type`, `rd_version`, `synopsis` | `*_lossy()` returns the located singleton view; `inspect_*()` keeps strict validation and returns the same located success type |
| `aliases`, `keywords`, `concepts` | `*_lossy()` yields lossy text projections; `inspect_aliases`, `inspect_keywords`, `inspect_concepts` yield located views and per-item errors |
| `sections` | `sections_lossy()` yields opaque `RdSection` values with private fields, `path()`, and positioned title/body accessors |
| `arguments` | `arguments_lossy()` yields opaque `RdArgument` values with `path()`, accessors, and `name_ref()`/`description_ref()` |
| `section_tree` | `section_tree_lossy()` yields located `RdSectionVisit` values; syntactic `kind` and `nesting` remain, and positioned title/body accessors are added |
| `RdDocument::generation_header` | `generation_header_lossy()` remains a document-level heuristic; its result exposes the locations of the generator marker and each source-file comment rather than claiming one node path |
| `RdDocument::lifecycle_badges` | `lifecycle_badges_lossy()` remains the best-effort document-level collector; each badge gains its underlying location, while `inspect_lifecycle_badges` keeps the diagnostic-bearing result |
| `rd_ast::text_contents`, `RdAlias::text_contents`, `RdKeyword::text_contents`, `RdConcept::text_contents`, `RdSectionVisit::text_contents` | `text_contents_lossy` at the free-function and view-method entry points; comment skipping and markup flattening remain explicit |
| `inspect_title`, `inspect_description`, `inspect_usage`, `inspect_value`, `inspect_name`, `inspect_details`, `inspect_note`, `inspect_author`, `inspect_references`, `inspect_see_also`, `inspect_examples`, `inspect_format`, `inspect_source`, `inspect_encoding`, `inspect_doc_type`, `inspect_rd_version`, `inspect_synopsis` | Same names, strict result, located success value |
| `inspect_aliases`, `inspect_keywords`, `inspect_concepts` | Same names, strict located item results |
| `inspect_sections`, `inspect_section_tree` | Same names, strict container/item validation with located views |
| `inspect_arguments` | Same two-level result contract: container validation result plus per-item result iterator; each successful `RdArgument` is located |

Singleton duplicate handling, repeatable-section order, custom-section
depth-first order, and the distinction between `RdSectionKind` and future
standard-section classification are unchanged.

### Node-level views

| Current entry points | 0.5.0 destination |
| --- | --- |
| `RdTagged::{inspect_link, inspect_href}`, `RdNode::{inspect_s4_class_link}` | `RdNodeRef::{inspect_link, inspect_href, inspect_s4_class_link}`; strict results are located |
| `RdNode::{s4_class_link, inline_span, text_symbol, conditional, example_control, figure, method, enc}` | `RdNodeRef::{s4_class_link_lossy, inline_span_lossy, text_symbol_lossy, conditional_lossy, example_control_lossy, figure_lossy, method_lossy, enc_lossy}`; existing `None` and projection behavior is preserved |
| `RdTagged::inspect_list` | `RdNodeRef::inspect_list`; `RdList::children` gains a positioned sequence accessor, delimited items expose `anchor_path()`, `body_ref()`, and `source_nodes()`, and described items retain their single-node `path()` |
| `RdTagged::inspect_tabular` | `RdNodeRef::inspect_tabular`; table/row/cell views distinguish anchors from sibling ranges, including empty cells. Rows expose `anchor_path()` and a `nodes_ref()` range that excludes terminal `Cr`; cells expose `anchor_path()` and `nodes_ref()` |
| `RdNode::inspect_enc` | `RdNodeRef::inspect_enc`; encoded/ascii accessors gain positioned sequence forms |
| `RdTagged::inspect_equation` | `RdNodeRef::inspect_equation`; latex/ascii projections retain positioned source nodes |
| `RdNode::{inspect_inline_span, inspect_text_symbol}` | `RdNodeRef::{inspect_inline_span, inspect_text_symbol}` with the same strict result; body accessors gain `_ref` forms |
| `RdNode::inspect_conditional` | `RdNodeRef::inspect_conditional`; branch and format accessors gain positioned forms, including synthetic conditional groups |
| `RdNode::inspect_example_control` | `RdNodeRef::inspect_example_control`; direct body remains exact and positioned |
| `RdNode::inspect_figure` | `RdNodeRef::inspect_figure`; the first argument remains owned by the figure and the sibling second argument remains separate |
| `RdNode::inspect_method` | `RdNodeRef::inspect_method`; generic and qualifier projections retain source cursors; a following usage signature remains a sibling |
| `RdTagged::{inspect_sexpr, inspect_rd_opts}` | `RdNodeRef::{inspect_sexpr, inspect_rd_opts}` returning the existing option errors; code/options and their positioned children remain available |
| `RdDocument::inspect_dynamic_markup` | Remains a stateful structural iterator. It carries positioned events and preserves state update and event order |
| `RdDocument::inspect_lifecycle_badges` | Remains strict at the document level; each badge retains its underlying figure location and diagnostics |
| `RdDocument::system_macro_items`, `RdDocument::inspect_system_macro_items` | Remain root sibling-sequence views with positioned consumed nodes; matches expose `anchor_path()`, `source_nodes()`, and `consumed()`, while strict traversal never consumes an unvalidated expansion |
| `RdNodesRef::{system_macro_items, inspect_system_macro_items}` | Provide the same views for any positioned sibling sequence, including slices, without accepting caller-supplied paths |

`text_contents` becomes `text_contents_lossy`. It continues to skip comments
and flatten content according to its existing contract. No strict whole-tree
text reconstruction is implied by the rename.

Generation-header results remain aggregate values without a fabricated header
path. `RdGenerationHeader::generator_path()` identifies the marker comment,
and `source_origins()` returns each source-file value together with the
comment path that produced it. Values, order, duplicates, and wrapping rules
remain unchanged.

### Option parsing

`RdOptionList::parse(nodes, path)` is an implementation detail and is no longer
public. Consumers obtain an `RdOptionRef` from a positioned node cursor and
call `RdOptionRef::parse()`. This operation parses a comma-separated list of
plain `key=value` pairs without quoting, escaping, or nesting. Parsed pair
order, duplicate handling, and `RdOptionError` details remain unchanged;
malformed syntax and non-text children are errors, while unknown keys and
invalid typed values remain soft diagnostics. Pair indices are not sibling
paths, and this release does not promise a source span for an option-pair
substring. `RdOptionList::nodes_ref()` and `sibling_range()` return the original
positioned option or body sequence and its absolute sibling range; `Sexpr`
options use an `Option` container while `RdOpts` bodies use the tagged node's
child container.

## Planned source-map contract

The following is a planned design for a follow-up `rd-source` source-map PR.
It describes the intended future API and is not part of the current normative
`rd-source` contract.

`rd-source::Parsed` is expected to gain a private field whose type is the
public, opaque `rd_source::RdSourceMap`, plus a `source_map()` accessor. The
`RdSourceMap` type would be publicly re-exported and nameable, but its fields
and representation would remain private; consumers would use its public
methods. `into_parts()` would remain a two-tuple `(RdDocument,
Vec<Diagnostic>)` for consumers that intentionally discard provenance. A
separate `into_parts_with_source_map()` would return all three values.

The planned `RdSourceMap::span(&RdAstPath)` would perform exact path lookup.
It would not return a parent's span for an unregistered path. The planned map
would cover the document root, every node actually emitted into the final AST
(including unknown, recovered, and synthetic conditional target/body `Group`
nodes), and every present option container. The parser would not emit or
invent an `RdNode` for a missing argument, so no path or map entry would exist
for one. Hard parse errors would return no document or map.

Each planned `SourceSpan` would be the smallest original-source range covering
the source consumed to construct that AST structure. It would be a byte range
in the original input with the existing one-based line and Unicode-scalar
column rules. It would not be a token stream, edit script, or guarantee that
every byte in the range belongs to the node. Source-syntax groups and options
would include their actual opening and closing delimiters. Synthetic
conditional groups would map to the real directive regions that produced them,
need not be brace-shaped, and might be zero-width when the corresponding
region is empty. Missing or virtual delimiters would never be invented and
would contribute no source bytes; decoded escapes would include their
original spelling; and CRLF would include both bytes. Recovery would end at
the actual synchronization point or EOF and would not include an unconsumed
following section. When recovery promotes children after discarding a bare
brace, child spans would remain tied to their own source while paths followed
their final AST indices.

The planned `Parsed` extension would keep its existing `Clone` and `PartialEq`
behavior: equality would compare the document and diagnostics and exclude the
source map. This would be an existing comparison projection, not a promise of
source-provenance equality. Thus LF and CRLF inputs could compare equal as
`Parsed` values while their source-map spans differed. `RdSourceMap` would not
need public `PartialEq` merely to test this behavior.

The planned map would intentionally not promise exact spans for flattened
argument contents, empty-cell insertion points, option-pair substrings,
decoded character-to-source mappings, or synthetic consumer IR. Those would
remain candidates for a later release after consumer evidence.

## Consumer examples

### Planned source-map usage after the follow-up

```rust
let parsed = rd_source::parse(input)?;
for node in parsed.document().walk() {
    if let Some(link) = node.inspect_link()? {
        let source = parsed.source_map().span(link.path());
        // `source` identifies the original link syntax.
    }
}
```

### Raw fallback versus Raw opacity

```rust
fn render(nodes: &rd_ast::RdNodesRef<'_>) {
    for node in nodes {
        if let Some(raw) = node.node().as_raw() {
            render_raw_fallback(raw);
            continue; // this explicit recursion chooses the Raw boundary
        }
        render_node(node);
    }
}
```

`document.walk()` is still appropriate for diagnostics that need every stored
node, including Raw descendants. Filtering a Raw node in a flat walk does not
prune its descendants.

### Empty cells and option nodes

An empty table cell is represented by an empty `RdSiblingRange` plus an anchor
for an existing separator or row boundary. It is not represented by a fake
`Child` path. A present empty option is represented by an `RdOptionRef` whose
position is `path().with_option()` and whose positioned children are empty;
an absent option has no option cursor.

### Stateful `Sexpr` traversal

`inspect_dynamic_markup()` remains a stateful iterator. Consumers must process
events in order: an `RdOpts` event updates the effective option state, and the
following `Sexpr` event observes that state. A generic structural walk does
not perform this state transition, deduplicate system-macro expansions, or
replace the dynamic-markup API.

## Migration rules

Consumers should first replace hand-built paths with `document.top_level()` or
`document.walk()` and use the path carried by each cursor. Replace direct
public fields on `RdArgument` and `RdSection` with accessors and use `_ref`
accessors when a recursive converter needs provenance. Keep `into_parts()`
when provenance is intentionally unused. After the source-map follow-up lands,
adapters that need provenance can use the new three-part projection.

The migration changes receiver and result types, path names, view opacity,
and lossy method names. It does not require a consumer to adopt a cursor for
low-level `RdDocument::nodes()`, `RdTagged::children()`, or existing
`IntoIterator` implementations. `typst-doc` can continue its low-level AST
projection, while `rd2qmd` and other best-effort renderers can use positioned
sequence accessors without cloning a document or manufacturing empty paths.
Detached inspection is deliberately not added in 0.5.0. If consumer migration
demonstrates a real detached use case, it must be designed with a non-canonical
location contract in a later release.
