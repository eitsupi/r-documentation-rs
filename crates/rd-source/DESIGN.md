# rd-source lowering design

## 1. Scope and conformance oracle

`rd-source` parses `.Rd` source text into `rd_ast::RdDocument`. Its conformance oracle is `tools::parse_Rd`, exercised through the differential harness in `tests/`. The future parser is a source producer of the same AST; it is not a second, source-specific document model.

`crates/rd-ast/CONTRACT.md` defines producer-agnostic invariants: canonical leaf strings, ordering and whitespace, comment representation, group obligations, option presence, and equivalence between producers. This design document defines source-grammar-specific behavior: lexer interpretation, parser modes, tag argument modes, source delimiters, and oracle-observed leaf flush boundaries. The division is intentional: a rule about `\usage` being R-like belongs here, while a rule that all producers preserve child order belongs in the AST contract.

## 2. Lexer/parser boundary

The accepted lexer contract, implemented in `crates/rd-source/src/lexer.rs`, is deliberately small:

- lexing is context-independent and lossless;
- every token carries a byte range into the original input;
- punctuation, control sequences, escapes, comments, newlines, whitespace, and ordinary text are classified without parser context;
- lexing never errors.

The parser owns meaning. `Comment`, brace, and bracket tokens are not final semantics: the active parser frame may reinterpret them. In particular, a percent token is a comment in comment-enabled modes but literal content in an equation frame, and a bracket is an option only when the relevant `TagSpec` allows one. Leaf segmentation follows oracle flush boundaries, not token classification. The parser therefore preserves each token's source slice until it has enough frame and syntactic context to emit an AST node.

## 3. Parser frames and mode definitions

The parser uses recursive descent with a mode stack. The minimum mode set is:

```text
Latex
RLike
Verbatim
Equation
```

`Equation` is distinct from ordinary `Verbatim` because `\verb` and `\preformatted` treat `%` as a comment opener, while `\eqn` and `\deqn`'s first arguments treat it as literal; their optional second arguments use ordinary `Verbatim` behavior instead. Each argument frame contains independent brace depth and the following metadata:

```text
ArgumentFrame {
    mode,
    terminating_brace_depth,
    emitted_leaf_kind,
    comments_enabled,
    markup_enabled,
    brace_policy,
}
```

When a nested macro or group returns, the parent frame's mode is restored. Brace depth is never shared between argument frames.

## 4. TagSpec correspondence table

The parser centralizes tag behavior in metadata rather than scattering a large match through the parser body:

```text
TagSpec {
    option_policy,
    arguments: [ArgumentSpec],
    allowed_contexts,
}
```

Context-sensitive tags such as `\method` and `\item` use a dedicated parser only where metadata cannot express the context.

| Tag or context | Option mode | Positional argument modes | AST and parsing notes |
|---|---|---|---|
| document root | — | `Latex` | Section-boundary whitespace is `TEXT`. |
| `\name` | — | `Verbatim` | Emits `VERB`. |
| `\alias` | — | `Verbatim` | `%` is a comment and may consume content through the closing brace. |
| `\encoding` | — | `Latex` | Emits `TEXT` (oracle-pinned by mode-tag-matrix). |
| `\Rdversion` | — | `Verbatim` | Emits `VERB` (oracle-pinned by mode-tag-matrix). |
| `\docType` | — | `Latex` | Emits `TEXT` (oracle-pinned by the composite-dataset fixture). |
| `\keyword`, `\concept` | — | `Latex` / restricted text | Both emit `TEXT` (oracle-pinned by mode-tag-matrix); the permitted markup range is provisional — pin with fixture. |
| `\title` | — | `Latex` | Inline markup is allowed. |
| `\description`, `\details`, `\value`, `\note`, `\author`, `\references`, `\seealso`, `\format`, `\source` | — | `Latex` | Ordinary Rd markup. |
| `\section`, `\subsection` | — | title=`Latex`, body=`Latex` | Each positional argument is a `Group`. |
| `\usage` | — | `RLike` | Ordinary content emits `RCODE`. |
| `\examples` | — | `RLike` | Ordinary content emits `RCODE`. |
| `\synopsis` | — | `Verbatim` | Emits `VERB` (oracle-pinned by mode-tag-matrix). |
| `\code` | — | `RLike` | Emits `RCODE`; recognized nested markup may be parsed. |
| `\special` | — | `RLike` | One required argument whose body emits `RCODE`; valid in Latex and R-like parents, but not as a top-level document section. Surplus brace groups remain siblings in the surrounding parent mode. |
| `\Sexpr` | `Latex` text | `RLike` | Option is `TEXT`; body is `RCODE`. |
| `\dontrun` | — | `Verbatim` | Body emits `VERB` (oracle-pinned by composite-code-heavy). |
| `\donttest` | — | `RLike` | Body emits `RCODE`; comments remain enabled (oracle-pinned by composite-code-heavy). |
| `\dontshow` | — | `RLike` | Body emits `RCODE` (oracle-pinned by `dont-family`). |
| `\dontdiff` | — | `RLike` | Body emits `RCODE`, like `\donttest` and `\dontshow`. |
| `\testonly` | — | `RLike` | Body emits `RCODE`; this remains a distinct tag, not an alias of `\dontshow`. |
| `\preformatted`, `\verb` | — | `Verbatim` | Body emits `VERB`; control sequences are normally literal. |
| `\eqn` | — | arg1=`Equation`, optional arg2=`Verbatim` | Positional arguments are `Group`; arg1 preserves all four escape spellings and treats `%` as literal, while arg2 uses ordinary `Verbatim` escape and comment behavior. |
| `\deqn` | — | arg1=`Equation`, optional arg2=`Verbatim` | Same as `\eqn`. |
| `\tabular` | — | columns=`Latex`, body=`Latex` | `\tab` and `\cr` are zero-child tagged nodes. |
| `\link` | `Latex` text | `Latex` | Bracket content is the option; body accepts markup. |
| `\linkS4class` | optional `Latex` text | `Latex` | Bracket content is optional; R 4.6.1 accepts structured markup there (the fixture pins plain text). |
| `\samp`, `\kbd`, `\env`, `\option`, `\out` | — | `Verbatim` | One required literal argument; control-sequence-like text remains literal. |
| `\command`, `\sQuote`, `\dQuote`, `\acronym`, `\abbr`, `\cite`, `\dfn` | — | `Latex` | One required structured text argument. |
| `\figure` | — | arg1=`Verbatim`, optional arg2=`Verbatim` | In an R-like parent, only arg1 attaches; the second brace group remains parent `RCODE` (pinned by `figure-rlike-arity`). |
| `\CRANpkg` | — | `Latex` | One required structured package-name argument; valid in Latex and R-like parents, but not at document root. A first-class source tag replaces R's USERMACRO + `\href`/`\pkg` expansion. |
| `\sspace` | — | none | Zero arguments; valid in Latex and R-like parents, but not at document root. Braces following it are surplus content in the surrounding parent mode (sibling LIST groups in Latex parents; ordinary parent RCODE text in R-like parents). The output-dependent spacing operation remains explicit. |
| `\I` | — | parent-dependent: `Latex` or `RLike` | One required argument parsed in the surrounding semantic mode. It is an RdTextFilter marker, not italic or `AsIs`; valid in document, Latex, and R-like contexts. |
| `\enc` | — | arg1=`Latex`, arg2=`Latex` | Two required structured arguments. |
| `\if` | — | arg1=`Latex`, arg2=`Latex` | Two required structured arguments. |
| `\ifelse` | — | arg1=`Latex`, arg2=`Latex`, arg3=`Latex` | Three required structured arguments. |
| `\method`, `\S3method`, `\S4method` in usage | — | arg1=`Latex`, arg2=`Latex` | Oracle-verified by method-mode-boundaries: each marker has two untagged groups whose leaves are `TEXT`, and the following signature plus newline remains one parent `RCODE` leaf. |
| `\item` in `\arguments`, `\value`, or `\describe` bodies | two | arg1/arg2=`Latex` | The current direct Latex argument policy is restored when the frame closes; see `value-item-boundaries.Rd` and `item-context-restoration.Rd`. |
| `\item` in `\itemize` or `\enumerate` bodies | zero | none | A following braced group is a sibling `LIST` node. |
| `\item` elsewhere in Latex arguments | unknown | recovery only | This emits an unknown-macro diagnostic and uses the existing literal `TEXT` plus sibling `LIST` recovery. |
| Ordinary inline macros (`\emph`, `\strong`, `\var`, `\pkg`, `\file`, and similar) | — | `Latex` | Defined by the tag metadata table. |
| `\url` | — | `Verbatim` | Emits `VERB` (oracle-pinned by composite-markup). |
| `\href` | — | arg1=`Verbatim`, arg2=`Latex` | Each positional argument is an untagged group; arg1 is one `VERB` leaf and arg2 is recursively parsed markup (pinned by `href-argument-modes`). |
| `\R`, `\dots`, `\ldots` | — | none | Zero-child tags. A following bare `{}` is a sibling empty `LIST` in `Latex`, but remains `RCODE` content in an R-like frame. |
| Unknown or user macro | pinned | one-argument `Latex` fallback | Fallback is pinned by CONTRACT §13: no expansion in v1; unknown invocations keep the one-argument fallback and the diagnostic. |

The invalid-context `\item` recovery shape and missing-arity recovery remain provisional
recovery-corpus material; the canonical warning-free contract is limited to the
direct-container policies above.

## 5. Token-to-node operational rules

## 5a. Pinned R-like quoted-macro contract

Inside an ordinary single-, double-, or backtick-quoted R-like string, a
control sequence is structured only when its spelling is known to the tag
specification and its first character after the backslash is `l` or `v`.
This recognizes links, `\var`, `\verb`, and `\ldots`; other known macros such
as `\emph`, `\code`, and `\href` remain literal RCODE text without a
diagnostic. Quote characters remain in adjacent RCODE leaves and the quote
state resumes after a nested tag. An escaped backslash remains literal.

An R `#` comment beginning in normal R-like state is opaque through its
terminating newline for Rd markup recognition, including control sequences in
that line. Unescaped braces remain structural for depth tracking as described
below. R raw strings are fully opaque. The contract is pinned by
`rlike-link-quotes`, `rlike-quoted-macro-whitelist`,
`rlike-link-opaque-regions`, `href-argument-modes`, and
`conditional-links`.

R-like `#` comments count unescaped braces and end at the owning close brace;
a pending raw prefix does not survive into a comment. An unescaped `%` inside
such a comment is literal RCODE content, not an Rd comment opener, per
Section 7. Braces after that `%` still update depth and may close the owning
argument (pinned by `rlike-hash-comment-percent` and
`rlike-hash-comment-percent-owning-brace`).

Warning-producing recovery cells (quoted `\href` inside `\code`, and unknown
`l`-prefixed macros inside quotes) remain provisional and are excluded from
the canonical corpus.

Each frame owns a leaf buffer. The following rules are operational: their precondition is a token and the active frame, and their result is an append, flush, or node emission.

```text
1. Each frame owns one leaf buffer.
2. An ordinary token is appended to the buffer for the active mode.
3. Escape(k) is decoded or retains its source spelling per Section 7's
   mode/lexical-state rule, then appended to that same buffer; it does not
   flush.
4. A Newline token appends a single normalized "\n" to the leaf buffer (a CRLF
   source newline contributes "\n", not "\r\n") and then flushes.
5. Comment flushes the buffer and emits one Comment node only when Section 7
   classifies it as an Rd comment. In an Equation frame or an RLike quote,
   raw-string, or `#`-comment lexical state, it is reinterpreted as literal
   content without flushing, including re-lexing its remainder as specified
   in Section 7.
6. A Comment node, when emitted, contains the source range including its
   leading '%'.
7. A recognized tagged node or group flushes the buffer before parsing.
8. A fresh buffer starts after every tagged node or group.
9. Argument or document termination flushes a non-empty buffer only.
10. An empty buffer emits no leaf, and emitted leaves are never merged.
```

For a recognized control sequence, consult its `TagSpec` before interpreting the next bracket or brace. A nested recognized macro changes structure only after its frame has been selected; otherwise punctuation remains content or depth bookkeeping according to the current mode.

## 6. Leaf segmentation

The parser flushes at newline-after-append, immediately before an emitted Rd comment, immediately before a tagged node or group, and at argument or document end. It never creates a leaf from an empty buffer and never merges adjacent leaves of the same kind after emission. This intentionally preserves oracle boundaries, including adjacent `TEXT` leaves.

```text
first line\nsecond line
```

becomes:

```text
TEXT("first line\n")
TEXT("second line")
```

Escapes do not flush:

```text
a\%b\{c\}
```

becomes:

```text
TEXT("a%b{c}")
```

An emitted Rd comment does flush, and the following newline is a separate leaf:

```text
a% comment
b
```

is approximately:

```text
TEXT("a")
COMMENT("% comment")
TEXT("\n")
TEXT("b")
```

The adjacent `TEXT` nodes are intentional and are protected by the AST contract's no-merge rule. Multiple lines in `\preformatted` likewise become multiple `VERB` leaves, one per newline flush boundary.

All source whitespace remains in order. At the root, the mode and leaf kind are `Latex` and `TEXT`; section gaps, indentation, and a final newline are not discarded. Canonical leaf values normalize CRLF to `\n`, matching the crlf-document oracle; the lexer's byte ranges still cover the original `\r\n` bytes, so diagnostics and any future span reporting retain the source form. Losslessness is a lexer-level property; canonical lowering follows the oracle. Opening and closing argument braces are delimiters rather than children, but newlines immediately inside them belong to the argument.

## 7. Escapes and comments

Escape processing depends on the active parser mode and, in `RLike` mode, the
current lexical state. The four recognized escape kinds have the following
decoded values:

| Escape token | Decoded value |
|---|---|
| `Escape(Percent)` | `%` |
| `Escape(LBrace)` | `{` |
| `Escape(RBrace)` | `}` |
| `Escape(Backslash)` | `\` |

The parser retains the token's complete source spelling instead of its decoded
value in exactly these cases:

- every escape in `Equation` mode;
- every escape while an `RLike` frame is inside an R raw string;
- `Escape(LBrace)` and `Escape(RBrace)` while an `RLike` frame is inside an
  ordinary single-, double-, or backtick-quoted string.

All other occurrences of the four escape kinds use the decoded value from the
table above. In particular, ordinary `Verbatim` content and an `\eqn` or
`\deqn` optional second argument decode all four kinds. In an ordinary quoted
R-like string, brace escapes retain their source spelling, while percent and
backslash escapes decode normally.

The operational rule is:

```text
if token is Escape(k):
    if mode is Equation
       or RLike state is RawString
       or (RLike state is OrdinaryQuote
           and k is LBrace or RBrace):
        append source_spelling(token) to the current leaf buffer
    else:
        append decoded_value(k) to the current leaf buffer

    do not change brace depth
    do not start an Rd comment
    do not emit a separate node
    do not flush
```

Whether decoded or retained, an escape token is content rather than a group
delimiter or comment opener: thus `\%` is not a comment opener in a
comments-enabled frame, and `\{` or `\}` is not a group or argument delimiter.

An unescaped `%` is a comment opener in `Latex` and ordinary `Verbatim`
frames. In normal `RLike` state it opens an Rd comment, including the first
`%` of an unescaped `%in%`; inside an ordinary R quote, an R raw string, or an
R `#` comment, it is literal RCODE content. In an `Equation` frame — the first
argument of `\eqn` or `\deqn` — it is literal VERB content. The optional
second argument is an ordinary `Verbatim` frame, so an unescaped `%` opens a
comment there.

```text
comments-enabled frame, Comment(range):
    flush current leaf buffer
    emit Comment(source[range])

Equation frame or opaque RLike lexical state, Comment(range):
    append "%" literally
    re-lex the remainder as ordinary content
```

The comment node includes the leading `%` but never the newline. The next `Newline` token is retained in the current mode's buffer and immediately flushed, producing for example `COMMENT("% inside")` followed by `TEXT("\n")`, `RCODE("\n")`, or `VERB("\n")`.

## 8. Brace/group handling

Brace meaning is determined by both mode and syntactic position.

In `Latex`, `{...}` immediately after a macro is a positional argument delimiter. Every positional slot of a multi-argument macro becomes an `RdGroup`, including an empty slot. A bare `{...}` is the `LIST` pseudo-tag, represented as `RdTag::List`, and is distinct from an untagged positional `RdGroup`. Bare braces nest recursively.

In `Verbatim`, `Equation`, and ordinary `RLike` code (including an R `#`
comment), unescaped braces remain literal leaf characters while updating
depth for argument termination. Inside an active `RLike` quoted string or raw
string, an unescaped brace is appended as content without changing depth —
that lexical state's own terminator, not brace counting, ends the string:

```text
LBrace inside an argument:
    depth += 1
    append "{"

RBrace when depth > argument_base_depth:
    depth -= 1
    append "}"

RBrace when depth == argument_base_depth:
    flush
    terminate the argument
```

This retains the inner braces of `\code{f({1})}` and `\preformatted{{x}}`. An active R-like frame still parses a recognized nested Rd macro structurally; `\method` and `\dontrun` are examples. In `Equation`, `\alpha` and other unrecognized control sequences remain literal backslash spellings; the four explicit escapes retain their source spelling too (see Section 7).

An empty group is significant:

```text
{} -> RdNode::Group(RdGroup::new(vec![]))
```

An empty tag body may instead have zero tag children, according to that tag's arity. The parser must distinguish that case from an empty positional slot, which emits `Group([])`.

## 9. Options and three-valued presence

Normal option recognition is specified for `\link`, `\Sexpr`, and the context-dependent `\item` family. `USERMACRO` option behavior is deferred. `\RdOpts` is not itself a bracket-option tag; its body is `VERB` option text.

```text
after a recognized control sequence:
    consult TagSpec.option_policy

if option_policy allows an option
   and the next syntactic token is LBracket:
       parse the bracket content as the option
else:
       treat LBracket and RBracket as ordinary content
```

The option body is parsed in `Latex` mode and stored in `RdTagged::option` as nodes, not flattened text (oracle-pinned by option-markup, including a nested `\emph` node). The warning-free canonical corpus pins absent and nonempty presence for `\link` (option-presence):

```text
no brackets        -> None
[nonempty content] -> Some(parsed_nodes)
```

The `[] -> Some(vec![])` branch is retained as an AST requirement, but `tools::parse_Rd` does not provide a warning-free normal-case source for it on the pinned R version; malformed empty-option probes belong to recovery coverage.

The option-spacing fixture confirms the tight `\link[x]` form. A space before `[` is rejected with a warning by the pinned oracle, so whitespace-before-opener behavior remains provisional and is excluded from the warning-free canonical corpus.

## 10. Error-recovery interaction points

Diagnostics and error recovery are a separate pending design task. This document fixes only the points where recovery interacts with lowering:

- malformed or empty options such as `\link[]` and `\Sexpr[]`;
- macro-environment injection for otherwise unrecognized invocations; the unknown-tag fallback shape itself is pinned by CONTRACT Section 13;
- partial-tree recovery differences across R versions;
- keeping recovery fixtures separate from the canonical corpus;
- timeouts while generating oracle output.

Normal structural conformance must not silently acquire version-specific recovery behavior. Recovery fixtures need their own expectations and metadata.

## 11. Oracle version and probe procedure

The oracle is generated by `tests/fixtures/generate.R`. Its header and generation code establish these exact settings: `encoding=UTF-8`, `keep.source=TRUE`, `permissive=FALSE`, uncompressed XDR version 3, and srcref-sanitized output. The current corpus was generated with R 4.6.1.

Every future fixture records the R version that generated it. When behavior is uncertain, first run a small `Rscript` probe with the same parse settings, then pin the result in a fixture only after observing its structure and runtime. Probes that can hang, especially malformed bracket options, receive a timeout and remain outside the canonical normal-case corpus. If R versions differ, preserve the version range and separate normal structural conformance from recovery conformance.

The normative regeneration and mass-corpus triage policy is in [tests/fixtures/README.md](tests/fixtures/README.md).

## 12. Known uncertainties / deferred behavior

The following behavior remains deferred:

- general user-macro expansion and macro-environment injection (curated system-macro aliases are pinned by CONTRACT Section 13);
- the partial-AST shape for malformed options;
- whether `\link[]` and `\Sexpr[]` are valid syntax in each supported R version;
- whitespace before an option opener;
- the complete context grammar of `\item[...]`;
- strict modes for metadata sections beyond the normal cases pinned by the mode matrix;
- arity inference for unknown macros;
- error-recovery differences between R versions;
- the diagnostic type and span representation.

The F4b fixture expansion pinned the following coverage:

- `dont-family`: `\dontrun`, `\donttest`, and `\dontshow` bodies are respectively `VERB`, `RCODE`, and `RCODE`.
- `tabular-modes`: column and cell content are `TEXT`, nested `\emph` is tagged, and `\tab`/`\cr` are zero-child nodes across body newlines.
- `section-leading-trailing-newline`: section body boundary newlines are retained as exact `TEXT` leaves, unlike the tight section.
- `control-sequence-in-verbatim`: unknown `\foo`/`\bar` spellings remain literal in `preformatted`, `verb`, and equation `VERB` leaves.

- `mode-tag-matrix`: leaf kinds for title, description, usage, examples, code, verb, preformatted, alias, keyword, concept, synopsis, encoding, docType, and Rdversion.
- `comment-by-mode`: `%` and `\%` in `TEXT`, `RCODE`, ordinary `VERB`, and equation `VERB`.
- `brace-by-mode`: nested and escaped braces in R code, an unmatched `}` inside a quoted R string, verbatim, and equation content.
- `escape-boundaries`: all four escapes in every leaf kind without a flush (decoded or retained per Section 7's mode/lexical-state rule).
- `option-presence`: warning-free absent and nonempty `\link` options, with empty-option rejection recorded as recovery-only.
- `option-markup`: a nested `\emph` in a `\link` option is retained by the oracle as option nodes.
- `option-spacing`: tight `\link[x]` recognition; whitespace before `[` is warning-producing in the pinned oracle.
- `method-mode-boundaries`: `\method`, `\S3method`, and `\S4method` arguments are `TEXT` groups and following signatures are parent `RCODE` leaves.

The F4b canonical corpus is complete; remaining unpinned behaviors (empty options, whitespace-before-option-opener, `\href` multi-mode, keyword/concept markup range, and other rows still marked provisional) are deferred to the recovery corpus or later design tasks.
