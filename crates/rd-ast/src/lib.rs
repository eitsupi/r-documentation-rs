//! The canonical, producer-agnostic AST for R's Rd documentation format.
//!
//! `rd-ast` is the common target of the help-database lowering and the
//! `rd-source` Rd source parser. Semantic interpretation is provided by borrowed
//! views rather than encoded as producer-specific storage.
#![doc = include_str!("../CONTRACT.md")]

mod document;
mod option;
mod path;
mod raw;
mod raw_classification;
mod shape;
mod tag;
mod view;

pub use document::{RdDocument, RdGroup, RdNode, RdTagged};
pub use option::{
    RdEffectiveSexprOptions, RdOptionError, RdOptionList, RdOptionPair, RdOptionPairErrorKind,
    RdOptionValueKind, RdSexprOptionKey, RdSexprOptionOverrides, RdSexprResults, RdSexprStage,
    RdStripWhite,
};
pub use path::{RdPath, RdPathSegment};
pub use raw::{
    RawRdEnvironment, RawRdNode, RawRdObject, RawRdReal, RawRdValue, RdAttribute, producer,
};
pub use raw_classification::{
    RawNodeClassification, RawNodeReason, UnexpectedRawNode, classify_raw_node,
};
pub use shape::{
    RdArity, RdConstruct, RdExpectedNode, RdNodeKind, RdShapeError, RdShapeErrorKind,
    is_inter_item_trivia,
};
pub use tag::RdTag;
pub use view::{
    RdAlias, RdArgument, RdColumnAlign, RdConcept, RdConditional, RdConditionalKind,
    RdDelimitedItem, RdDescribedItem, RdDynamicMarkupEvent, RdDynamicMarkupIter,
    RdDynamicMarkupState, RdEnc, RdEquation, RdEquationDisplay, RdExampleControl,
    RdExampleControlKind, RdFigure, RdFigureSecondArgument, RdGenerationHeader, RdGenerator,
    RdHref, RdInlineSpan, RdInlineSpanKind, RdKeyword, RdLifecycleBadge, RdLifecycleBadgeShape,
    RdLifecycleBadges, RdLifecycleStage, RdLink, RdLinkDestination, RdLinkTopic, RdList,
    RdListItem, RdListKind, RdMethod, RdMethodKind, RdOpts, RdResolvedSexpr, RdS4ClassLink,
    RdSection, RdSectionKind, RdSectionVisit, RdSexpr, RdSystemMacro, RdSystemMacroItem,
    RdSystemMacroItems, RdSystemMacroItemsStrict, RdSystemMacroMatch, RdSystemMacroOrigin,
    RdTableCell, RdTableRow, RdTabular, RdTextSymbol, RdTextSymbolKind, text_contents,
};

#[cfg(feature = "rds")]
mod rds;

#[cfg(feature = "rds")]
pub use rds::{LowerError, LowerLocation, StringEncoding, lower_r_object};
