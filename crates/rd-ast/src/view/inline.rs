//! Borrowed views for source-level one-argument inline markup and text symbols.
//!
//! [`RdInlineSpan`] classifies the source-level spelling of uniform
//! one-argument inline markup. It assigns no typography, quoting,
//! code-formatting, URL/email, or rendering behavior. Preformatted is excluded
//! because it is block-level and belongs to rd2qmd's fenced-code matcher;
//! Href and Link have existing link views; LinkS4Class, Figure, and Enc have
//! dedicated views in later passes; Eqn and Deqn have the equation view; Out
//! is format-specific raw output; I is parent-mode-dependent and help-DB
//! producers expand it as a system macro; CranPkg and Doi have
//! producer-dependent system-macro representations; Sspace is zero-argument
//! and output-dependent; section, list, table, example-control, and dynamic
//! tags have their own structural views; and R, Dots, and LDots belong to the
//! text-symbol view below.
//!
//! One-argument macros do not preserve the argument group, so contents sit
//! directly in [`RdTagged::children()`]. Empty and missing arguments are
//! therefore indistinguishable to this view. The option and tag are validated;
//! argument presence and arity are not.

use super::*;

/// The source-level spelling of a uniform one-argument inline markup macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdInlineSpanKind {
    Emph,
    Strong,
    Bold,
    Code,
    Special,
    Verb,
    Url,
    Email,
    File,
    Pkg,
    Samp,
    SQuote,
    DQuote,
    Kbd,
    Var,
    Env,
    Command,
    Option,
    Acronym,
    Abbr,
    Cite,
    Dfn,
}

/// A borrowed source-level inline span.
#[derive(Debug, Clone, PartialEq)]
pub struct RdInlineSpan<'a> {
    path: RdPath,
    kind: RdInlineSpanKind,
    body: &'a [RdNode],
}

impl<'a> RdInlineSpan<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn kind(&self) -> RdInlineSpanKind {
        self.kind
    }
    /// Returns the direct children exactly as stored.
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }
}

fn inline_span_kind(tag: &RdTag) -> Option<RdInlineSpanKind> {
    Some(match tag {
        RdTag::Emph => RdInlineSpanKind::Emph,
        RdTag::Strong => RdInlineSpanKind::Strong,
        RdTag::Bold => RdInlineSpanKind::Bold,
        RdTag::Code => RdInlineSpanKind::Code,
        RdTag::Special => RdInlineSpanKind::Special,
        RdTag::Verb => RdInlineSpanKind::Verb,
        RdTag::Url => RdInlineSpanKind::Url,
        RdTag::Email => RdInlineSpanKind::Email,
        RdTag::File => RdInlineSpanKind::File,
        RdTag::Pkg => RdInlineSpanKind::Pkg,
        RdTag::Samp => RdInlineSpanKind::Samp,
        RdTag::SQuote => RdInlineSpanKind::SQuote,
        RdTag::DQuote => RdInlineSpanKind::DQuote,
        RdTag::Kbd => RdInlineSpanKind::Kbd,
        RdTag::Var => RdInlineSpanKind::Var,
        RdTag::Env => RdInlineSpanKind::Env,
        RdTag::Command => RdInlineSpanKind::Command,
        RdTag::Option => RdInlineSpanKind::Option,
        RdTag::Acronym => RdInlineSpanKind::Acronym,
        RdTag::Abbr => RdInlineSpanKind::Abbr,
        RdTag::Cite => RdInlineSpanKind::Cite,
        RdTag::Dfn => RdInlineSpanKind::Dfn,
        _ => return None,
    })
}

impl RdNode {
    pub fn inline_span(&self, base_path: &RdPath) -> Option<RdInlineSpan<'_>> {
        let tagged = self.as_tagged()?;
        let kind = inline_span_kind(tagged.tag())?;
        tagged.option().is_none().then(|| RdInlineSpan {
            path: base_path.clone(),
            kind,
            body: tagged.children(),
        })
    }

    pub fn inspect_inline_span(
        &self,
        base_path: &RdPath,
    ) -> Result<Option<RdInlineSpan<'_>>, RdShapeError> {
        match self {
            RdNode::Tagged(tagged) => {
                let Some(kind) = inline_span_kind(tagged.tag()) else {
                    return Ok(None);
                };
                if tagged.option().is_some() {
                    return Err(shape(
                        base_path.clone(),
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::UnexpectedOption,
                    ));
                }
                Ok(Some(RdInlineSpan {
                    path: base_path.clone(),
                    kind,
                    body: tagged.children(),
                }))
            }
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if inline_span_kind(&tag).is_none() {
                    return Ok(None);
                }
                Err(shape(
                    base_path.clone(),
                    Some(tag),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                ))
            }
            _ => Ok(None),
        }
    }
}

/// The source-level spelling of a zero-argument text symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdTextSymbolKind {
    R,
    Dots,
    LDots,
}

impl RdTextSymbolKind {
    /// A plain-text projection only; typographic substitution is consumer
    /// policy. `Dots` and `LDots` stay distinct for spelling preservation.
    pub const fn fallback_text(self) -> &'static str {
        match self {
            Self::R => "R",
            Self::Dots | Self::LDots => "...",
        }
    }
}

/// A source-level zero-argument text symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct RdTextSymbol {
    path: RdPath,
    kind: RdTextSymbolKind,
}

impl RdTextSymbol {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn kind(&self) -> RdTextSymbolKind {
        self.kind
    }
    pub const fn fallback_text(&self) -> &'static str {
        self.kind.fallback_text()
    }
}

fn text_symbol_kind(tag: &RdTag) -> Option<RdTextSymbolKind> {
    match tag {
        RdTag::R => Some(RdTextSymbolKind::R),
        RdTag::Dots => Some(RdTextSymbolKind::Dots),
        RdTag::LDots => Some(RdTextSymbolKind::LDots),
        _ => None,
    }
}

impl RdNode {
    pub fn text_symbol(&self, base_path: &RdPath) -> Option<RdTextSymbol> {
        let tagged = self.as_tagged()?;
        let kind = text_symbol_kind(tagged.tag())?;
        (tagged.option().is_none() && tagged.children().is_empty()).then(|| RdTextSymbol {
            path: base_path.clone(),
            kind,
        })
    }

    pub fn inspect_text_symbol(
        &self,
        base_path: &RdPath,
    ) -> Result<Option<RdTextSymbol>, RdShapeError> {
        match self {
            RdNode::Tagged(tagged) => {
                let Some(kind) = text_symbol_kind(tagged.tag()) else {
                    return Ok(None);
                };
                if tagged.option().is_some() {
                    return Err(shape(
                        base_path.clone(),
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::UnexpectedOption,
                    ));
                }
                if !tagged.children().is_empty() {
                    return Err(shape(
                        base_path.clone(),
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::WrongArity {
                            expected: RdArity::Exactly(0),
                            actual: tagged.children().len(),
                        },
                    ));
                }
                Ok(Some(RdTextSymbol {
                    path: base_path.clone(),
                    kind,
                }))
            }
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if text_symbol_kind(&tag).is_none() {
                    return Ok(None);
                }
                Err(shape(
                    base_path.clone(),
                    Some(tag),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                ))
            }
            _ => Ok(None),
        }
    }
}
