//! The writer's producer-independent mirror of the source parser's tag table.

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Latex,
    RLike,
    Verbatim,
    Equation,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    Document,
    Latex,
    RLike,
}
#[derive(Clone, Copy)]
pub(crate) struct TagSpec {
    pub args: &'static [Mode],
    pub contexts: &'static [Context],
    pub optional: bool,
    pub required: usize,
}

/// Mirror the parser's rule for structured macros recognized in quoted R code.
pub(crate) fn recognized_in_ordinary_quote(tag: &str) -> bool {
    tag.as_bytes()
        .get(1)
        .is_some_and(|first| matches!(first, b'l' | b'v'))
        && tag_spec(tag).is_some()
}

const L: &[Mode] = &[Mode::Latex];
const R: &[Mode] = &[Mode::RLike];
const V: &[Mode] = &[Mode::Verbatim];
const E: &[Mode] = &[Mode::Equation, Mode::Verbatim];
const H: &[Mode] = &[Mode::Verbatim, Mode::Latex];
const M: &[Mode] = &[Mode::Latex, Mode::Latex];
const T: &[Mode] = &[Mode::Latex, Mode::Latex, Mode::Latex];
const ITEM: &[Mode] = &[Mode::Latex, Mode::Latex];
const FIGURE: &[Mode] = &[Mode::Verbatim, Mode::Verbatim];
const Z: &[Mode] = &[];
const DOC: &[Context] = &[Context::Document];
const LATEX_CONTEXTS: &[Context] = &[Context::Document, Context::Latex];
const ALL_CONTEXTS: &[Context] = &[Context::Document, Context::Latex, Context::RLike];
const INLINE_CONTEXTS: &[Context] = &[Context::Latex, Context::RLike];

pub(crate) fn tag_spec(tag: &str) -> Option<TagSpec> {
    let (args, optional) = match tag {
        r"\name" => (V, false),
        r"\alias" | r"\Rdversion" | r"\synopsis" | r"\RdOpts" | r"\url" | r"\samp" | r"\kbd"
        | r"\env" | r"\option" | r"\out" | r"\dontrun" | r"\verb" | r"\preformatted" => (V, false),
        r"\title" | r"\description" | r"\value" | r"\details" | r"\note" | r"\author"
        | r"\references" | r"\seealso" | r"\format" | r"\source" | r"\encoding" | r"\docType"
        | r"\keyword" | r"\concept" | r"\emph" | r"\strong" | r"\bold" | r"\pkg" | r"\file"
        | r"\email" | r"\var" | r"\doi" | r"\CRANpkg" | r"\command" | r"\sQuote" | r"\dQuote"
        | r"\acronym" | r"\abbr" | r"\cite" | r"\dfn" => (L, false),
        r"\usage" | r"\examples" | r"\code" | r"\donttest" | r"\dontshow" | r"\dontdiff"
        | r"\testonly" => (R, false),
        r"\special" => (R, false),
        r"\href" => (H, false),
        r"\eqn" | r"\deqn" => (E, false),
        r"\link" | r"\linkS4class" => (L, true),
        r"\Sexpr" => (R, true),
        r"\figure" => (FIGURE, true),
        r"\I" => (L, false),
        r"\enc" | r"\if" | r"\method" | r"\S3method" | r"\S4method" | r"\section"
        | r"\subsection" | r"\tabular" => (M, false),
        r"\ifelse" => (T, false),
        r"\itemize" | r"\enumerate" | r"\describe" | r"\arguments" => (L, false),
        r"\item" => (ITEM, false),
        r"\tab" | r"\cr" | r"\R" | r"\dots" | r"\ldots" | r"\sspace" => (Z, false),
        _ => return None,
    };
    let required = if matches!(tag, r"\eqn" | r"\deqn" | r"\figure") {
        1
    } else if tag == r"\item" {
        0
    } else {
        args.len()
    };
    Some(TagSpec {
        args,
        contexts: contexts_for(tag),
        optional,
        required,
    })
}

fn contexts_for(tag: &str) -> &'static [Context] {
    match tag {
        "\\name" | "\\alias" | "\\Rdversion" | "\\synopsis" | "\\RdOpts" | "\\encoding"
        | "\\docType" | "\\keyword" | "\\concept" => DOC,
        "\\title" | "\\description" | "\\value" | "\\details" | "\\note" | "\\author"
        | "\\references" | "\\seealso" | "\\format" | "\\source" | "\\section" | "\\subsection"
        | "\\tabular" | "\\arguments" | "\\item" | "\\tab" | "\\cr" => LATEX_CONTEXTS,
        "\\special" | "\\CRANpkg" | "\\sspace" => INLINE_CONTEXTS,
        _ => ALL_CONTEXTS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rd_ast::RdTag;

    #[test]
    fn covers_every_known_ast_tag() {
        for tag in RdTag::KNOWN {
            assert!(
                tag_spec(tag.as_rd_tag()).is_some()
                    || matches!(tag, RdTag::List | RdTag::IfDef | RdTag::IfNDef),
                "missing spec for {:?}",
                tag
            );
        }
    }
}
