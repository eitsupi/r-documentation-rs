# rd-ast

`rd-ast` is the canonical, producer-agnostic AST for R's Rd documentation format. It is the common target for help-database lowering and Rd source parsing.

## Tree model

The tree is mostly generic. It has closed leaf kinds (`Text`, `RCode`, `Verb`, and `Comment`), a generic `Tagged` node carrying the known `RdTag` vocabulary and preserving unknown tag spellings as `RdTag::Unknown`, positional `Group` nodes, and the lossless `Raw` escape hatch for genuinely non-canonical structures. Producers must preserve ordering, whitespace, options, and unexpected content according to the [producer/consumer contract](CONTRACT.md).

## Semantic views

Views are borrowed interpretations of the canonical tree; they do not replace its syntax-layer storage.

- Document and section views expose standard metadata, repeated metadata, and custom section/subsection trees.
- Link views interpret topic links, `href`, and S4-class links while leaving resolution and rendering to consumers.
- List views expose itemize, enumerate, and describe containers and their items.
- Tabular views expose column alignment, rows, cells, and structural diagnostics.
- Figure views expose figure file arguments and an optional alt-text or expert-options second argument.
- Equation views distinguish inline `eqn` and block `deqn` forms with LaTeX and optional ASCII content.
- Encoding views expose the encoded and ASCII alternatives of `enc`.
- Conditional views expose `if` and `ifelse` branches without selecting a format.
- Method views expose `method`, `S3method`, and `S4method` names and arguments.
- Example-control views classify `dontrun`, `donttest`, `dontshow`, `dontdiff`, and `testonly` wrappers.
- System-macro views recognize the documented sibling sequences for curated system macros.
- Generation-header views recognize roxygen generation comments and preserve the generator name and source paths.

## Features and stability

The optional `serde` feature enables serde derives for AST values. The optional `rds` feature enables lowering from the `rd-rds` object model. The serde representation is not a stable interchange format: it is a same-version implementation detail, as documented by [CONTRACT.md](CONTRACT.md), and should not be treated as cross-version wire compatibility.

See the [workspace stability policy](https://github.com/eitsupi/r-documentation-rs/blob/main/STABILITY.md) for the 0.1.x compatibility and support commitments.

## License

MIT; see [the workspace license](../../LICENSE).
