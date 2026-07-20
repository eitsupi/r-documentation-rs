use super::*;

/// Lossily flattens `nodes` into a single string, discarding all markup
/// structure.
///
/// This is a deliberately simple, total function, **not** an Rd2txt-style
/// renderer: it does not add spacing, line breaks, or any formatting
/// around the markup it strips, it does not resolve `\Sexpr`/`\if`/links,
/// and it does not know that e.g. `\strong{...}` should render with
/// emphasis markers in plain-text output. Callers that need rendering
/// should build that separately; this function only concatenates raw
/// leaf text in source order:
///
/// - [`RdNode::Text`], [`RdNode::RCode`], and [`RdNode::Verb`] append
///   their string unchanged.
/// - [`RdNode::Comment`] contributes nothing (comments are not part of
///   rendered content).
/// - [`RdNode::Tagged`] ignores `option` and recurses into `children` in
///   order (so e.g. a `\link[pkg]{topic}`'s `[pkg]` option is *not*
///   included, only `topic`).
/// - [`RdNode::Raw`] ignores `option` and `attributes` and recurses into
///   `children` in order.
///
/// No whitespace trimming or normalization is performed -- the result can
/// contain the exact runs of newlines/spaces `parse_Rd()` preserved in
/// `TEXT` leaves. Callers that want normalized (single-spaced, trimmed)
/// text should post-process the result themselves, e.g. via
/// `s.split_whitespace().collect::<Vec<_>>().join(" ")`.
///
/// The AST itself remains lossless; this function is where the crate's
/// one deliberately lossy flattening lives.
pub fn text_contents(nodes: &[RdNode]) -> String {
    let mut out = String::new();
    push_text_contents(nodes, &mut out);
    out
}

fn push_text_contents(nodes: &[RdNode], out: &mut String) {
    for node in nodes {
        match node {
            RdNode::Text(text) | RdNode::RCode(text) | RdNode::Verb(text) => out.push_str(text),
            RdNode::Comment(_) => {}
            RdNode::Tagged(tagged) => push_text_contents(tagged.children(), out),
            RdNode::Group(group) => push_text_contents(group.children(), out),
            RdNode::Raw(raw) => push_text_contents(raw.children(), out),
        }
    }
}
