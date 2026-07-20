//! Lossy convenience and strict inspection views layered on [`RdDocument`].
//!
//! The normative validation boundary and consumer guidance are in the crate's
//! included `CONTRACT.md`.
//!
//! ## Why this module exists
//!
//! The crate-level documentation describes the AST's losslessness
//! invariant: a correct producer must be able to represent the Rd
//! semantic content of *any* valid Rd input, including markup this crate
//! doesn't yet know about, without dropping any of that content. That
//! invariant makes [`RdDocument`] and [`RdNode`]
//! deliberately awkward to consume directly -- finding "the" title means
//! scanning top-level nodes for a `Tagged { tag: RdTag::Title, .. }`, and
//! pairing an `\item`'s label with its body means recognizing a specific
//! two-child shape.
//!
//! The original accessors are deliberately lossy: they recognize one common
//! shape and silently skip anything else. The `inspect_*` counterparts are
//! fallible and share the producer-neutral [`crate::RdShapeError`] vocabulary.
//! Semantic views for links, lists, tables, equations, and options extend this
//! same vocabulary. The dynamic-markup layer also resolves document-level
//! `\\RdOpts` state for `\\Sexpr` nodes.
//!
//! ## Never interprets [`RdNode::Raw`]
//!
//! Lossy accessors here only ever match [`RdNode::Tagged`]. A
//! [`RdNode::Raw`] node is, by construction, a shape the lowering couldn't
//! represent canonically -- for example a node whose attributes go beyond
//! what the producer recognizes (see
//! [`RawRdNode`](crate::RawRdNode)'s docs; note that the RDS lowering
//! deliberately discards the ubiquitous `srcref` source-reference
//! attribute rather than treating it as unknown, so ordinary
//! parse-with-source-references Rd content still lowers to `Tagged`).
//! Strict accessors also never interpret `Raw` content, but report a matching
//! raw tag as an unexpected node instead of silently ignoring it. The sole
//! documented exception is the sibling-oriented system-macro view, which
//! recognizes only fully validated corpus-pinned `USERMACRO` sequences.

use crate::{
    RdArity, RdConstruct, RdDocument, RdExpectedNode, RdNode, RdNodeKind, RdPath, RdPathSegment,
    RdShapeError, RdShapeErrorKind, RdTag, RdTagged, is_inter_item_trivia,
};
use crate::{
    RdEffectiveSexprOptions, RdOptionError, RdOptionList, RdSexprOptionOverrides, RdSexprStage,
};

mod conditional;
mod document;
mod dynamic;
mod encoding;
mod equation;
mod example;
mod figure;
mod generation;
mod inline;
mod lifecycle;
mod link;
mod list;
mod method;
mod system_macro;
mod tabular;
mod text;

pub use conditional::{RdConditional, RdConditionalKind};
pub use document::{
    RdAlias, RdArgument, RdConcept, RdKeyword, RdSection, RdSectionKind, RdSectionVisit,
};
pub use dynamic::{
    RdDynamicMarkupEvent, RdDynamicMarkupIter, RdDynamicMarkupState, RdOpts, RdResolvedSexpr,
    RdSexpr,
};
pub use encoding::RdEnc;
pub use equation::{RdEquation, RdEquationDisplay};
pub use example::{RdExampleControl, RdExampleControlKind};
pub use figure::{RdFigure, RdFigureSecondArgument};
pub use generation::{RdGenerationHeader, RdGenerator};
pub use inline::{RdInlineSpan, RdInlineSpanKind, RdTextSymbol, RdTextSymbolKind};
pub use lifecycle::{RdLifecycleBadge, RdLifecycleBadgeShape, RdLifecycleBadges, RdLifecycleStage};
pub use link::{RdHref, RdLink, RdLinkDestination, RdLinkTopic, RdS4ClassLink};
pub use list::{RdDelimitedItem, RdDescribedItem, RdList, RdListItem, RdListKind};
pub use method::{RdMethod, RdMethodKind};
pub use system_macro::{
    RdSystemMacro, RdSystemMacroItem, RdSystemMacroItems, RdSystemMacroItemsStrict,
    RdSystemMacroMatch, RdSystemMacroOrigin,
};
pub use tabular::{RdColumnAlign, RdTableCell, RdTableRow, RdTabular};
pub use text::text_contents;

pub(super) fn top_path(index: usize) -> RdPath {
    RdPath::new(vec![RdPathSegment::TopLevel(index)])
}

pub(super) fn child_path(parent: usize, index: usize) -> RdPath {
    RdPath::new(vec![
        RdPathSegment::TopLevel(parent),
        RdPathSegment::Child(index),
    ])
}

pub(super) fn node_tag(node: &RdNode) -> Option<RdTag> {
    node.as_tagged()
        .map(|n| n.tag().clone())
        .or_else(|| node.as_raw().and_then(|n| n.tag().map(RdTag::from_rd_tag)))
}

pub(super) fn shape(path: RdPath, tag: Option<RdTag>, kind: RdShapeErrorKind) -> RdShapeError {
    RdShapeError::new(path, tag, kind)
}

pub(super) fn concat_exact(
    nodes: &[RdNode],
    required: RdNodeKind,
    group_path: &RdPath,
    tag: &RdTag,
) -> Result<String, RdShapeError> {
    let mut value = String::new();
    for (index, node) in nodes.iter().enumerate() {
        let text = match (required, node) {
            (RdNodeKind::Text, RdNode::Text(text)) => text,
            (RdNodeKind::Verb, RdNode::Verb(text)) => text,
            _ => {
                return Err(shape(
                    group_path.with_child(index),
                    Some(tag.clone()),
                    RdShapeErrorKind::UnexpectedContent {
                        actual: RdNodeKind::of(node),
                    },
                ));
            }
        };
        value.push_str(text);
    }
    Ok(value)
}

#[cfg(test)]
mod tests;
