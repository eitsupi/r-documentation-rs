//! [`RdTag`]: the closed vocabulary of known Rd markup tags.
//!
//! ## Excluded: backslash-escapes
//!
//! Rd source uses `\%`, `\{`, `\}`, and `\\` to escape a literal `%`, `{`,
//! `}`, or `\` inside running text. These are deliberately **not** `RdTag`
//! variants: `parse_Rd()` resolves them to their literal character at parse
//! time and folds the result directly into the surrounding `TEXT` leaf --
//! they never appear as a distinct `Rd_tag`-carrying node in a parsed Rd
//! object (verified empirically against R's `tools::parse_Rd()`). There is
//! therefore nothing for `RdTag` to represent for these escapes; the
//! escaped character is just part of an [`RdNode::Text`](crate::RdNode::Text) string.

/// Defines the [`RdTag`] enum together with its `from_rd_tag` / `as_rd_tag`
/// conversions and the `KNOWN` list, from a single table of
/// `(doc comment, variant name, Rd_tag string)` entries.
///
/// Keeping the table in one place (rather than three separate `match`
/// blocks written by hand) is what keeps `from_rd_tag` and `as_rd_tag`
/// guaranteed to round-trip for every variant: there is exactly one place
/// that spells out the mapping.
macro_rules! rd_tags {
    ($( $( #[doc = $doc:literal] )+ $variant:ident => $s:literal ),+ $(,)?) => {
        /// The closed vocabulary of Rd markup tags, as found in the
        /// `Rd_tag` string attribute R's `parse_Rd()` attaches to each node
        /// of a parsed Rd object.
        ///
        /// Every macro-shaped known tag stores the *exact* wire string
        /// (including the leading backslash, e.g. `"\\title"`) so that
        /// [`RdTag::as_rd_tag`] round-trips through [`RdTag::from_rd_tag`]
        /// without loss.
        ///
        /// Two tag-like strings are handled outside this closed macro
        /// vocabulary:
        ///
        /// - `"LIST"` is not a macro (no backslash): it is the pseudo-tag
        ///   `parse_Rd()` attaches to a bare, un-named `{...}` brace group
        ///   that appears directly in running text (as opposed to the
        ///   *positional argument* groups of multi-argument macros such as
        ///   `\method{generic}{class}`, which carry no `Rd_tag` at all and
        ///   are represented as [`RdNode::Group`](crate::RdNode::Group).
        ///   It is modeled here as [`RdTag::List`].
        /// - `"TEXT"`, `"RCODE"`, `"VERB"`, and `"COMMENT"` are leaf
        ///   pseudo-tags. They are deliberately **not** `RdTag` variants:
        ///   they identify a leaf's *content kind*, not a markup macro, and
        ///   map directly to the [`RdNode`](crate::RdNode) leaf variants
        ///   `Text`, `RCode`, `Verb`, and `Comment` respectively. Note this
        ///   is a distinct concept from `RdTag::Verb`, which is the
        ///   `\verb{...}` *macro* (a `Tagged` node whose child is typically
        ///   a `VERB`-tagged leaf).
        ///
        /// Any other string -- including tags for dynamically-expanded
        /// user macros such as `USERMACRO` (see the note on
        /// [`RdTag::Doi`]) -- is preserved verbatim as [`RdTag::Unknown`].
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[non_exhaustive]
        pub enum RdTag {
            $(
                $( #[doc = $doc] )+
                $variant,
            )+
            /// The bare brace-group pseudo-tag `"LIST"` (no leading
            /// backslash): a `{...}` group with no macro name, appearing
            /// directly in running text.
            List,
            /// An `Rd_tag` string that isn't part of the known vocabulary
            /// above, preserved exactly as encountered.
            Unknown(String),
        }

        impl RdTag {
            /// All known (non-[`Unknown`](RdTag::Unknown)) tag variants,
            /// including [`List`](RdTag::List). Useful for exhaustive
            /// testing and introspection.
            pub const KNOWN: &'static [RdTag] = &[
                $( RdTag::$variant, )+
                RdTag::List,
            ];

            /// Parses the exact `Rd_tag` string as stored by a help
            /// database (i.e. *with* the leading backslash for macros,
            /// e.g. `"\\title"`, `"\\item"`, `"\\Sexpr"`, and bare for the
            /// `"LIST"` pseudo-tag).
            ///
            /// Unrecognized input is preserved exactly via
            /// [`RdTag::Unknown`], so this function never fails.
            pub fn from_rd_tag(tag: &str) -> RdTag {
                match tag {
                    $( $s => RdTag::$variant, )+
                    "LIST" => RdTag::List,
                    other => RdTag::Unknown(other.to_string()),
                }
            }

            /// Returns the canonical wire-format string for this tag.
            ///
            /// Guaranteed to round-trip through [`RdTag::from_rd_tag`] for
            /// every variant, including [`RdTag::Unknown`].
            pub fn as_rd_tag(&self) -> &str {
                match self {
                    $( RdTag::$variant => $s, )+
                    RdTag::List => "LIST",
                    RdTag::Unknown(s) => s,
                }
            }
        }
    };
}

rd_tags! {
    /// `#ifdef target`: conditionally included Rd content for a target.
    IfDef => "#ifdef",
    /// `#ifndef target`: conditionally included Rd content when a target is absent.
    IfNDef => "#ifndef",
    // ---- Section tags -----------------------------------------------
    /// `\name{...}`: the topic's name (required).
    Name => r"\name",
    /// `\alias{...}`: an additional name this topic is looked up under.
    Alias => r"\alias",
    /// `\title{...}`: the topic's one-line title (required).
    Title => r"\title",
    /// `\description{...}`: the topic's short description (required).
    Description => r"\description",
    /// `\usage{...}`: call signature(s), rendered as R code.
    Usage => r"\usage",
    /// `\arguments{...}`: the `\item` list describing each argument.
    Arguments => r"\arguments",
    /// `\value{...}`: description of the return value.
    Value => r"\value",
    /// `\details{...}`: extended description.
    Details => r"\details",
    /// `\note{...}`: supplementary remarks.
    Note => r"\note",
    /// `\author{...}`: author information.
    Author => r"\author",
    /// `\references{...}`: bibliographic references.
    References => r"\references",
    /// `\seealso{...}`: cross-references to related topics.
    SeeAlso => r"\seealso",
    /// `\examples{...}`: runnable example R code.
    Examples => r"\examples",
    /// `\keyword{...}`: an R documentation keyword.
    Keyword => r"\keyword",
    /// `\concept{...}`: a free-text concept index term.
    Concept => r"\concept",
    /// `\format{...}`: the structure of documented data.
    Format => r"\format",
    /// `\source{...}`: provenance of documented data.
    Source => r"\source",
    /// `\encoding{...}`: declares the source file's encoding.
    Encoding => r"\encoding",
    /// `\docType{...}`: the documentation object's type (e.g. `package`).
    DocType => r"\docType",
    /// `\Rdversion{...}`: the Rd format version the file was written for.
    Rdversion => r"\Rdversion",
    /// `\synopsis{...}`: an alternate usage synopsis (rare, legacy).
    Synopsis => r"\synopsis",
    /// `\section{title}{...}`: a custom top-level section.
    ///
    /// The section title is carried by a child node (the section's first
    /// argument group), not by this tag itself.
    Section => r"\section",
    /// `\subsection{title}{...}`: a custom section nested within another
    /// section's content.
    Subsection => r"\subsection",

    // ---- List / structure tags --------------------------------------
    /// `\item{...}` or `\item{label}{...}`: a list entry. Used both as a
    /// zero-argument separator inside `\itemize`/`\enumerate` (where the
    /// item's content is the following sibling nodes up to the next
    /// `\item`) and as a two-argument label/description pair inside
    /// `\describe` and `\arguments`.
    Item => r"\item",
    /// `\itemize{...}`: a bulleted list of `\item`s.
    Itemize => r"\itemize",
    /// `\enumerate{...}`: a numbered list of `\item`s.
    Enumerate => r"\enumerate",
    /// `\describe{...}`: a description list of label/definition `\item`s.
    Describe => r"\describe",
    /// `\tabular{cols}{...}`: a table, with `\tab`-separated cells and
    /// `\cr`-terminated rows.
    Tabular => r"\tabular",
    /// `\tab`: a column separator inside `\tabular`.
    Tab => r"\tab",
    /// `\cr`: a row terminator inside `\tabular` (also usable as a plain
    /// line break elsewhere).
    Cr => r"\cr",

    // ---- Inline markup ------------------------------------------------
    /// `\code{...}`: inline R code.
    Code => r"\code",
    /// `\special{...}`: R-like content for a special, non-standard usage form.
    Special => r"\special",
    /// `\verb{...}`: verbatim inline text. Distinct from the `VERB` leaf
    /// pseudo-tag (see the enum-level documentation): a `\verb{...}` macro
    /// is a `Tagged` node whose child is typically a `VERB`-tagged leaf.
    Verb => r"\verb",
    /// `\preformatted{...}`: a verbatim block.
    Preformatted => r"\preformatted",
    /// `\emph{...}`: emphasized (italic) text.
    Emph => r"\emph",
    /// `\strong{...}`: strongly emphasized (bold) text.
    Strong => r"\strong",
    /// `\bold{...}`: bold text.
    Bold => r"\bold",
    /// `\href{url}{text}`: a hyperlink with explicit display text.
    Href => r"\href",
    /// `\url{...}`: a literal URL.
    Url => r"\url",
    /// `\email{...}`: an email address.
    Email => r"\email",
    /// `\file{...}`: a file path.
    File => r"\file",
    /// `\pkg{...}`: a package name.
    Pkg => r"\pkg",
    /// `\samp{...}`: a literal sequence of characters ("sample").
    Samp => r"\samp",
    /// `\sQuote{...}`: single-quoted text.
    SQuote => r"\sQuote",
    /// `\dQuote{...}`: double-quoted text.
    DQuote => r"\dQuote",
    /// `\kbd{...}`: keyboard input.
    Kbd => r"\kbd",
    /// `\var{...}`: a variable name.
    Var => r"\var",
    /// `\env{...}`: an environment variable name.
    Env => r"\env",
    /// `\command{...}`: a command name.
    Command => r"\command",
    /// `\option{...}`: a command-line option name.
    Option => r"\option",
    /// `\acronym{...}`: an acronym.
    Acronym => r"\acronym",
    /// `\abbr{...}`: an abbreviation.
    Abbr => r"\abbr",
    /// `\cite{...}`: a citation reference.
    Cite => r"\cite",
    /// `\dfn{...}`: a defining instance of a term.
    Dfn => r"\dfn",
    /// `\link{topic}` or `\link[pkg]{topic}` (or `\link[pkg:target]{topic}`
    /// via the option string): a cross-reference link. The optional
    /// package/target qualifier is carried in `option`.
    Link => r"\link",
    /// `\linkS4class{class}` or `\linkS4class[pkg]{class}`: a
    /// cross-reference to S4 class documentation.
    LinkS4Class => r"\linkS4class",
    /// `\figure{file}` or `\figure{file}{options}`: an embedded image.
    Figure => r"\figure",
    /// `\doi{id}`: a DOI link.
    ///
    /// Note: in R's own `parse_Rd()`/help-database pipeline, `\doi` (like
    /// `\CRANpkg`, `\sspace`, `\I`, and other convenience macros) is not a grammar
    /// token but a *dynamic user macro* loaded from
    /// `share/Rd/macros/system.Rd`. Its expansion shows up in the parsed
    /// tree as an `Rd_tag = "USERMACRO"` leaf followed by a nested
    /// `\Sexpr` call, not as a node tagged `"\\doi"`. This variant is kept
    /// in the closed vocabulary because a hand-written Rd source parser
    /// (a producer that does not replicate R's macro-expansion engine) may
    /// reasonably choose to recognize `\doi` directly; lowering from an
    /// installed help database will typically produce `RdTag::Unknown("USERMACRO".into())`
    /// plus a `Sexpr` node instead.
    Doi => r"\doi",
    /// `\CRANpkg{package}`: a reference to a package on CRAN.
    ///
    /// R defines this in `share/Rd/macros/system.Rd` and expands it to an
    /// `\href` containing `\pkg`. `rd-source` preserves the source-level
    /// semantic operation directly instead of reproducing that expansion.
    CranPkg => r"\CRANpkg",
    /// `\sspace`: output-dependent sentence spacing.
    ///
    /// R expands this system macro to a LaTeX/non-LaTeX `\ifelse`. Keeping
    /// it as a distinct tag preserves the output-sensitive source intent.
    Sspace => r"\sspace",
    /// `\I{...}`: content marked to be ignored by `RdTextFilter`.
    ///
    /// This system macro expands to its argument unchanged. It does not
    /// request italics, emphasis, or R's `AsIs` class.
    I => r"\I",
    /// `\enc{encoded}{ascii}`: encoding-dependent text with an ASCII
    /// fallback.
    Enc => r"\enc",

    // ---- Math -----------------------------------------------------------
    /// `\eqn{latex}` or `\eqn{latex}{ascii}`: an inline equation.
    Eqn => r"\eqn",
    /// `\deqn{latex}` or `\deqn{latex}{ascii}`: a displayed equation.
    Deqn => r"\deqn",

    // ---- Dynamic / conditional content -----------------------------------
    /// `\Sexpr{code}` or `\Sexpr[options]{code}`: dynamically generated
    /// content, evaluated at build or render time. Options (e.g.
    /// `results=rd,stage=build`) are carried in `option`.
    Sexpr => r"\Sexpr",
    /// `\RdOpts{options}`: sets default `\Sexpr` options for the
    /// remainder of the file.
    RdOpts => r"\RdOpts",
    /// `\if{format}{content}`: content included only for a specific
    /// output format.
    If => r"\if",
    /// `\ifelse{format}{then}{else}`: format-conditional content with a
    /// fallback.
    IfElse => r"\ifelse",
    /// `\out{...}`: raw, format-specific output passed through unescaped.
    Out => r"\out",

    // ---- Examples markers -------------------------------------------------
    /// `\dontrun{...}`: example code that should not be executed.
    DontRun => r"\dontrun",
    /// `\donttest{...}`: example code that should not be tested
    /// automatically (but is shown and may be run manually).
    DontTest => r"\donttest",
    /// `\dontshow{...}`: example code that is executed but not displayed.
    DontShow => r"\dontshow",
    /// `\dontdiff{...}`: example code excluded from diff-based checking.
    DontDiff => r"\dontdiff",
    /// `\testonly{...}`: an alias for `\dontshow`.
    TestOnly => r"\testonly",

    // ---- Method macros (used inside \usage) ------------------------------
    /// `\method{generic}{class}`: an S3 method usage entry.
    Method => r"\method",
    /// `\S3method{generic}{class}`: an S3 method usage entry (equivalent
    /// to `\method`).
    S3Method => r"\S3method",
    /// `\S4method{generic}{signature}`: an S4 method usage entry.
    S4Method => r"\S4method",

    // ---- Special-character macros -----------------------------------------
    /// `\R`: the "R" logo/name.
    R => r"\R",
    /// `\dots`: the `...` ellipsis.
    Dots => r"\dots",
    /// `\ldots`: the `...` ellipsis (alias for `\dots`; kept as a distinct
    /// variant because it is a distinct `Rd_tag` string and must round-trip
    /// exactly).
    LDots => r"\ldots",
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every known variant (including `List`) must round-trip exactly:
    /// `from_rd_tag(tag.as_rd_tag()) == tag`.
    #[test]
    fn known_tags_round_trip() {
        for tag in RdTag::KNOWN {
            let s = tag.as_rd_tag();
            assert_eq!(
                &RdTag::from_rd_tag(s),
                tag,
                "round-trip failed for {tag:?} via {s:?}"
            );
        }
    }

    /// Every known tag's wire string must be recovered exactly by
    /// `as_rd_tag(from_rd_tag(s))`, checked the other way around from a
    /// literal string table (catches accidental typos in the macro
    /// invocation that `known_tags_round_trip` alone wouldn't).
    #[test]
    fn representative_wire_strings_round_trip() {
        let cases = [
            r"\name",
            r"\alias",
            r"\title",
            r"\description",
            r"\usage",
            r"\arguments",
            r"\value",
            r"\details",
            r"\note",
            r"\author",
            r"\references",
            r"\seealso",
            r"\examples",
            r"\keyword",
            r"\concept",
            r"\format",
            r"\source",
            r"\encoding",
            r"\docType",
            r"\Rdversion",
            r"\synopsis",
            r"\section",
            r"\subsection",
            r"\item",
            r"\itemize",
            r"\enumerate",
            r"\describe",
            r"\tabular",
            r"\tab",
            r"\cr",
            r"\code",
            r"\special",
            r"\verb",
            r"\preformatted",
            r"\emph",
            r"\strong",
            r"\bold",
            r"\href",
            r"\url",
            r"\email",
            r"\file",
            r"\pkg",
            r"\CRANpkg",
            r"\sspace",
            r"\I",
            r"\samp",
            r"\sQuote",
            r"\dQuote",
            r"\kbd",
            r"\var",
            r"\env",
            r"\command",
            r"\option",
            r"\acronym",
            r"\abbr",
            r"\cite",
            r"\dfn",
            r"\link",
            r"\linkS4class",
            r"\figure",
            r"\doi",
            r"\enc",
            r"\eqn",
            r"\deqn",
            r"\Sexpr",
            r"\RdOpts",
            r"\if",
            r"\ifelse",
            r"\out",
            r"\dontrun",
            r"\donttest",
            r"\dontshow",
            r"\dontdiff",
            r"\testonly",
            r"\method",
            r"\S3method",
            r"\S4method",
            r"\R",
            r"\dots",
            r"\ldots",
            "LIST",
        ];
        for s in cases {
            let tag = RdTag::from_rd_tag(s);
            assert_ne!(
                tag,
                RdTag::Unknown(s.to_string()),
                "{s:?} unexpectedly Unknown"
            );
            assert_eq!(tag.as_rd_tag(), s, "{s:?} did not round-trip");
        }
    }

    /// Representative help-DB-observed strings from the task description,
    /// spot-checking `from_rd_tag` directly.
    #[test]
    fn from_rd_tag_representative_help_db_strings() {
        assert_eq!(RdTag::from_rd_tag(r"\title"), RdTag::Title);
        assert_eq!(RdTag::from_rd_tag(r"\link"), RdTag::Link);
        assert_eq!(RdTag::from_rd_tag("LIST"), RdTag::List);
        assert_eq!(
            RdTag::from_rd_tag(r"\madeUpMacro"),
            RdTag::Unknown(r"\madeUpMacro".to_string())
        );
    }

    /// `Unknown` must preserve the original string exactly, including
    /// leading/trailing whitespace or unusual casing -- `from_rd_tag` must
    /// never normalize input it doesn't recognize.
    #[test]
    fn unknown_preserves_original_string_exactly() {
        for s in [
            "",
            "unknown",
            r"\Unknown",
            "TEXT",
            "RCODE",
            "VERB",
            "COMMENT",
            "USERMACRO",
        ] {
            assert_eq!(RdTag::from_rd_tag(s), RdTag::Unknown(s.to_string()));
            assert_eq!(RdTag::from_rd_tag(s).as_rd_tag(), s);
        }
    }

    /// Sanity check: the known-tag list has no accidental duplicate wire
    /// strings (which would make `from_rd_tag` ambiguous).
    #[test]
    fn known_tags_have_unique_wire_strings() {
        let mut seen = std::collections::HashSet::new();
        for tag in RdTag::KNOWN {
            assert!(
                seen.insert(tag.as_rd_tag()),
                "duplicate wire string: {:?}",
                tag.as_rd_tag()
            );
        }
    }
}
