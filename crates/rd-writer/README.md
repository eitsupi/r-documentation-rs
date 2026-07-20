# rd-writer

`rd-writer` serializes canonical `rd_ast::RdDocument` values back to Rd source
text. It is a faithful emitter: node order and stored whitespace are retained,
but pretty-printing and byte identity are not promised.

If `write_document(document)` succeeds, parsing the returned source with
`rd-source` produces a diagnostic-free document equal to `document`. Documents
that cannot be represented by Rd source are rejected with a hard error.

The weekly R corpus job also performs a diagnostic-free writer round-trip check
against the pinned R source tree, inventorying expected unserializable documents.

```rust
use rd_ast::{RdDocument, RdNode, RdTag};
use rd_writer::write_document;

let document = RdDocument::from(vec![RdNode::tagged(
    RdTag::Name,
    None,
    vec![RdNode::Verb("example".into())],
)]);
let source = write_document(&document)?;
# Ok::<_, rd_writer::WriteError>(source)
```

## Stability

See the repository [stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md).
