# `rd-source` parser contract

This document is the normative diagnostics and error-recovery contract for
the future public `rd-source` parser. It complements
[`rd-ast`'s contract](../rd-ast/CONTRACT.md), which defines producer-agnostic
AST invariants, and `rd-source/DESIGN.md`, which is a non-normative lowering
and implementation design. This document defines input policy, outcomes,
diagnostics, spans, and recovery; it does not duplicate the AST rules.

Capitalized **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are
normative RFC-style keywords.

## 1. Scope and status

The conformance oracle is `tools::parse_Rd(..., encoding = "UTF-8", permissive
= FALSE)`, with the pinned R version used by the fixture suite. The oracle is
evidence about accepted syntax and recovery, not the complete AST contract.
When oracle recovery conflicts with the `rd-ast` contract, `rd-ast`
conformance wins. Every such difference MUST be pinned by a fixture and
described in the differential metadata.

This is a v1 contract for a public source parser. Syntax recovery MUST be
deterministic, and a parser implementation MUST satisfy the acceptance
criteria in section 12.

## 2. Input model

The public input is `&[u8]`, not `&str`. The entire input MUST be valid UTF-8;
embedded NUL MUST be rejected. The parser MUST NOT auto-detect a BOM or an
encoding. A BOM is rejected or consumed only if that behavior is explicitly
added and fixture-tested. An `\encoding{...}` declaration is document
metadata, not permission to reinterpret already supplied bytes. An absent
declaration or a UTF-8-compatible declaration is supported; Latin-1 and other
non-UTF-8 declarations MUST return `UnsupportedEncoding` in v1.

Bytes are required because lexing needs exact byte offsets, malformed input
must be testable, and `InvalidUtf8` is an explicit parser error. An `&str`
entry point would move that required decision outside this crate.

## 3. Outcome model

Successful parsing returns `Ok(Parsed { document, diagnostics })`. A document
with `Error`-severity diagnostics still returns `Ok`. Hard input failures
return `Err(ParseError)` and no partial document. There is no `Fatal`
diagnostic severity; fatal conditions belong in `ParseError`.

## 4. Hard errors

Hard errors return no `RdDocument`. No ordinary delimiter, macro, option, or
arity problem MAY become a hard error.

| Condition | Observed `parse_Rd` behavior | Required v1 behavior |
| --- | --- | --- |
| Invalid UTF-8 | Warns, then can return an R string containing invalid bytes | `Err(ParseError::InvalidUtf8)` |
| Embedded NUL | A raw-connection probe stops at NUL and ignores the suffix | `Err(ParseError::NulByte)` |
| Unsupported encoding | Depends on connection and encoding parameters | `Err(ParseError::UnsupportedEncoding)` |
| Conflicting encoding declaration | Exact behavior requires a focused probe | Hard `UnsupportedEncoding`, rather than lossy conversion |
| Input too large | Not an Rd grammar condition | `Err(ParseError::InputTooLarge)` when an implementation limit is reached |
| Excessive nesting | R behavior is implementation-dependent | `Err(ParseError::NestingLimitExceeded)` |

Invalid UTF-8 and NUL are deliberate hard errors because canonical AST strings
MUST be valid UTF-8 and source content MUST NOT silently disappear.

## 5. Recoverable conditions

Syntax problems MUST normally return `Ok(Parsed)` with an `Error` diagnostic.
The severity communicates that recovery occurred and that the result is not
fully valid input.

| Condition | Observed `parse_Rd` behavior | Required `rd-source` behavior | Diagnostic code |
| --- | --- | --- | --- |
| Unexpected top-level `}` | Warns, discards the token, otherwise empty | Discard it | `UnexpectedClosingDelimiter` |
| Unexpected bare `{` where the grammar forbids a group (for example, at document top level between sections) | Warns, discards both braces, retains contents | Discard both braces; retain contents in the current container | `UnexpectedOpeningDelimiter` |
| Unclosed `{` at EOF | Warns at EOF and acts closed | Synthesize an EOF closing boundary | `UnclosedGroup` |
| Missing required argument | Warns; malformed macro may disappear or become empty parent | Preserve known macro and opened arguments; diagnose absence | `MissingArgument` |
| Unclosed or malformed `[...]` | Warns; may discard macro and leak content | Preserve macro and parsed option content; synchronize deterministically | `UnclosedOption` or `InvalidOptionSyntax` |
| Unknown macro | Warns; permissive false can drop wrapper and leak body | Preserve `Tagged(Unknown)` and diagnose | `UnknownTag` |
| Known macro in disallowed context | Warns and can drop wrapper | Preserve tagged node and diagnose context | `TagNotAllowedHere` |
| Missing or malformed delimiter | Warns and resynchronizes | Preserve identifiable macro and recover at a synchronization point | `UnexpectedToken` |
| Too few positional arguments | Warns for tested zero-argument `\eqn` | Preserve available arguments and diagnose | `InvalidArity` |
| Too many positional-looking groups | Can parse surplus as a silent sibling `Tagged(RdTag::List)` node | Stop at maximum arity; preserve surplus as an ordinary bare group lowered to sibling `Tagged(RdTag::List)` | none for surplus itself |
| Stray macro-like control sequence | Warns or recovers by macro/context | Preserve spelling and identifiable groups as known or unknown tagged node | applicable tag/structure code |

## 6. Recovery semantics

The following operational rules are normative:

1. The parser MUST validate the complete byte input for NUL and UTF-8 before
   constructing canonical nodes. Failure returns a hard error.
2. The parser MUST maintain a container stack containing the document, tagged
   macro, positional group, and option-content container kinds.
3. At EOF, the parser MUST close open containers from innermost to outermost,
   emitting one diagnostic per unmatched opener. The diagnostic's span is the
   unmatched opener's token byte range; an EOF-anchored secondary/evidence span
   MAY be added later if secondary spans are introduced. Accumulated children
   remain.
4. An unexpected `}` MUST be consumed and dropped once when no group is open,
   with `UnexpectedClosingDelimiter`; it MUST NOT become decoded text.
5. A bare `{...}` where the grammar permits inline content is not an error: it
   MUST lower to `Tagged(RdTag::List)` with no diagnostic. When the grammar
   forbids a group, such as a bare `{` at document top level between sections,
   the opening and matching closing braces MUST be consumed and discarded,
   contents MUST be retained in the current container, and
   `UnexpectedOpeningDelimiter` MUST be emitted. A `{` that opens an expected
   positional argument MUST start that argument's `Group`.
6. A missing required argument MUST preserve the known `Tagged` node and MUST
   NOT invent an empty `Group`; only actually parsed groups and content appear
   in its children. Record `MissingArgument { tag, argument_index }`.
7. Once `[` is consumed by an option-accepting macro, option presence MUST be
   `Some(...)`, even when empty or unclosed. Close it at `]`, a grammar-valid
   argument opener that would otherwise swallow the argument, an enclosing
   close, section synchronization, or EOF. Preserve parsed option children.
8. An unknown macro MUST be `RdNode::Tagged(RdTagged::new(RdTag::Unknown(original), ...))`,
   with generic option and group parsing and `UnknownTag` at `Error` severity.
   It MUST never be represented by `Raw` merely because it is unknown.
9. A known macro in the wrong context MUST retain its known tag, parse normal
   syntactic arguments, and emit `TagNotAllowedHere`; its wrapper MUST NOT be
   discarded.
10. A fixed or limited-arity macro MUST consume at most its maximum. Missing
    minimum arguments diagnose; later groups remain sibling
    `Tagged(RdTag::List)` nodes and are silent. Thus `\eqn{a}{b}{c}` has two
    arguments and a silent sibling `Tagged(RdTag::List)` node for `{c}`.
11. Synchronization MUST be attempted in this order: matching close delimiter;
    next required argument opener; newline for a section header or argument;
    next section-level macro at top level; enclosing close; EOF.
12. Every output node MUST satisfy the `rd-ast` contract: valid UTF-8 strings,
    no trimming or merging, ordered comments, positional `Group` nodes,
    unknown `Tagged(Unknown)` nodes, and no `Raw` recovery container.

The exact leaf kind and escape decoding are defined by the applicable AST
contract and the parser's F4 implementation boundary.

## 7. Silently tolerated conditions

These conditions MUST produce neither a hard error nor a diagnostic.

| Condition | Observed `parse_Rd` behavior | Required v1 behavior |
| --- | --- | --- |
| Empty input | A connection can add a newline `TEXT` artifact | Truly empty document |
| Missing final newline | R can synthesize trailing newline text | Do not synthesize content |
| Whitespace-only input | Accepted | Preserve whitespace as `Text` |
| Stray `]` outside option context | Silent text | Ordinary current-mode content |
| Bare trailing backslash | Silent text in the probe | Preserve under escape/mode rules |
| Valid section after preceding text | Silent | Preserve both |
| Surplus group after fixed arity | Can be a silent sibling `Tagged(RdTag::List)` node | Preserve as an ordinary bare group lowered to sibling `Tagged(RdTag::List)` node |
| CRLF | Accepted | One line break for positions; preserve leaf value per escape rules |
| Legal one- or two-argument `\eqn` | Silent | Preserve actual legal arity |
| Empty permitted groups | Silent | Preserve empty `Group` |

## 8. Deliberate deviations from the oracle

The following deviations are intentional: invalid UTF-8 and NUL are hard
errors; unknown-tag wrappers are preserved as `Tagged(Unknown)`; wrong-context
wrappers are preserved; and no trailing newline content is synthesized.

The exact `permissive = TRUE` behavior and the spelling of `RdTag::Unknown`
(with or without a leading backslash) MUST be probed and fixture-pinned before
implementation relies on them **(provisional — pin with fixture)**. The
observed permissive-false probe dropped an unknown wrapper and leaked its body
as text, so that behavior is not an AST target. The spelling convention for
unknown tags likewise remains **(provisional — pin with fixture)**.

## 9. Positions and spans

`SourceSpan` is normative in bytes and resolved for diagnostics:

```rust
pub struct SourceSpan {
    pub bytes: Range<usize>,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

pub struct SourcePosition {
    pub line: u32,   // 1-based
    pub column: u32, // 1-based Unicode-scalar column
}
```

Byte ranges are half-open over original input bytes; `start` is inclusive and
`end` exclusive. Lines and columns are 1-based. Columns count Unicode scalar
values, not UTF-8 bytes. CRLF is one line break. EOF diagnostics use
`len..len`. An unmatched-opener diagnostic is anchored at the opener, with an
optional EOF evidence span if secondary spans are later supported. Line and
column MUST be derived centrally from byte ranges, never maintained
independently by parser branches. A precomputed newline index is sufficient.

## 10. Public API shape

The following sketches define the required shape; exact field visibility MUST
follow `rd-ast` accessor conventions.

```rust
pub fn parse(input: &[u8]) -> Result<Parsed, ParseError>;

pub struct Parsed {
    document: RdDocument,
    diagnostics: Vec<Diagnostic>,
}

pub struct Diagnostic {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    span: SourceSpan,
}

#[non_exhaustive]
pub enum Severity { Warning, Error }

#[non_exhaustive]
pub enum DiagnosticCode {
    UnexpectedClosingDelimiter,
    UnexpectedOpeningDelimiter,
    UnclosedGroup,
    UnclosedOption,
    MissingArgument,
    UnknownTag,
    TagNotAllowedHere,
    InvalidOptionSyntax,
    InvalidArity,
    UnexpectedToken,
}

#[non_exhaustive]
pub enum ParseError {
    InvalidUtf8 { valid_up_to: usize, error_len: Option<usize> },
    NulByte { offset: usize },
    UnsupportedEncoding { name: String, span: Option<SourceSpan> },
    InputTooLarge,
    NestingLimitExceeded { span: SourceSpan },
}
```

`Parsed`, `Diagnostic`, `SourceSpan`, and errors SHOULD expose accessors.
Diagnostic code variants and their documented meanings are semver-stable once
published; message wording is explicitly unstable. `Severity` and
`DiagnosticCode` are `#[non_exhaustive]`. `Warning` is reserved for suspicious
accepted constructs; v1 recovery diagnostics are usually `Error`. R warning
text MUST NOT be the primary API. Future fields such as related spans or
notes MUST be additive.

## 11. Differential-testing contract

Fixtures have three categories:

- `valid`: oracle success, no warnings, exact canonical document parity, and
  zero `rd-source` diagnostics.
- `recoverable`: oracle outcome and warning category/line are recorded, while
  exact source diagnostic code, severity, byte span, and recovered AST shape
  are asserted. Message text is not compared.
- `hard-input`: exact `ParseError` variant and offset are asserted; oracle
  behavior is documentation only where it truncates or returns invalid text.

A comparison axis separates oracle-comparison policy from source validity.
The default (oracle-parity) is the behavior described above.
Intentional-divergence cases keep source_status = "valid" semantics on
the source side (zero diagnostics) but assert the oracle tree and the
divergent source expectation separately, through oracle_obligations and
source_obligations, and record a divergence_reason. Curated system-macro
aliases (section 13) use this category.

Planned `cases.toml` metadata includes:

```toml
oracle_status = "ok"
oracle_warning_kinds = "unknown_macro"
oracle_warning_lines = "2"
source_status = "recovered"
expected_diagnostics = "error:unknown_tag:9..17"
recovery_obligations = "unknown_tag_preserved"
```

`generate.R` MUST use `withCallingHandlers` for every warning, `tryCatch` for
errors, and serialize a tree only when one was returned. It MUST write a
metadata oracle beside the RDS, normalize messages into repository-owned
warning categories, and retain raw messages only as version-pinned evidence.
The valid category keeps the any-warning-aborts rule; recoverable cases do
not. Each case MUST have a timeout. Parity with R warning text is explicitly
not a goal.

## 12. Parser acceptance criteria

1. `parse(&[u8]) -> Result<Parsed, ParseError>` is public and documented.
2. Valid fixtures return `Ok`, zero diagnostics, and the expected canonical AST.
3. Recoverable fixtures return `Ok`, expected diagnostics, and exact recovery shapes.
4. Invalid UTF-8 and NUL return the exact hard-error variant and offset.
5. No syntax-only fixture returns `Err`.
6. No malformed-syntax recovery path emits `Raw` merely for recovery.
7. Unknown tags are `Tagged(Unknown)` with exact fixture-pinned spelling.
8. Unclosed containers close at EOF and retain accumulated content.
9. Unexpected closing delimiters are consumed once without losing following content.
10. Diagnostic byte spans are in bounds and align to token boundaries where applicable.
11. Mapping covers LF, CRLF, multibyte UTF-8, and EOF.
12. The parser never panics on arbitrary byte input.
13. Fuzz/property tests verify invalid encoding yields a declared hard error; otherwise parsing terminates and every AST string is valid UTF-8.
14. Oracle message text is not asserted.
15. Known deviations from R recovery are fixture-pinned and documented.

## 13. Macro handling

Rd macros fall into three normative classes in v1:

1. **Grammar-native macros.** Tags in R's Rd grammar (and in the rd-ast
closed vocabulary) parse to their known tags with exact oracle parity.
Section 6 applies unchanged.
2. **Curated system-macro aliases.** A small, explicit source-parser profile
of system macros from share/Rd/macros/system.Rd — each with a name,
arity, argument mode, and semantic tag — is recognized directly as
first-class tags. The oracle expands these macros into a USERMACRO leaf
followed by the expansion, so each alias is an intentional, documented
divergence: the parser MUST NOT replicate the expansion, whose \Sexpr
calls, stage=build options, and srcref provenance are R-evaluation
artifacts rather than source semantics. Each alias MUST be pinned by an
intentional-divergence fixture that records the oracle tree and the
divergent source expectation separately (section 11). The v1 curated alias
set is \doi -> Doi, \CRANpkg -> CranPkg, \sspace -> Sspace, and \I -> I.
The \I argument mode follows the surrounding Latex or R-like mode;
\sspace is zero-argument and does not consume following braces.
3. **General user macros.** There is no expansion engine in v1. An unknown
invocation parses per section 6 rule 8 (Tagged(Unknown(spelling)) plus
an UnknownTag diagnostic) with the one-group fallback of rule 10; for a
multi-argument invocation the later groups therefore become sibling
LIST nodes — content is preserved, association with the macro is not.

\newcommand and \renewcommand definitions do not update a macro
environment in v1: a definition parses as an unknown macro, and an invocation
of a file-locally-defined macro is an unknown macro. Package-level macro
loading (man/macros, RdMacros) is out of scope for parse(&[u8]); a
future entry point MAY accept an injected macro environment mirroring
parse_Rd's macros=, with package-layout resolution remaining the
caller's responsibility.

Mass-corpus consequence: oracle trees containing USERMACRO are deferred
from differential comparison as whole files today; the planned replacement is
a comparison layer that normalizes known system-macro expansions so those
files re-enter coverage. Help-database producers keep emitting the observed
Raw USERMACRO shape (the rd-ast raw-classification guard); consumers
needing producer-independent semantics get them from semantic views, not
from this parser imitating R's expansion.

## 14. Deferred beyond v1

The following are deferred: general user-macro expansion,
macro-environment injection, and `\newcommand` definition semantics (curated
system-macro aliases are in scope per section 13);
non-UTF-8 transcoding; encoding auto-detection; R warning-wording
parity; related spans and fix-its; spans inside `RdDocument`; permissive and
strict modes; semantic checker rules; a stable diagnostic wire format;
per-R-version recovery parity; and resource-limit customization.
