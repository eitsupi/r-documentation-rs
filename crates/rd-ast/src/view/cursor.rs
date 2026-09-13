//! Cursor-based semantic inspection facades.

use super::*;

impl<'a> RdNodeRef<'a> {
    /// Strictly inspects this node as a link, using this cursor's path.
    pub fn inspect_link(&self) -> Result<Option<crate::RdLink<'a>>, RdShapeError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_error(
                self.node(),
                self.path(),
                RdTag::Link,
                RdExpectedNode::Tagged,
            )
            .map(|_| None);
        };
        if tagged.tag() == &RdTag::Link {
            tagged.inspect_link(self.path()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Strictly inspects this node as an href, using this cursor's path.
    pub fn inspect_href(&self) -> Result<Option<crate::RdHref<'a>>, RdShapeError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_error(
                self.node(),
                self.path(),
                RdTag::Href,
                RdExpectedNode::Tagged,
            )
            .map(|_| None);
        };
        if tagged.tag() == &RdTag::Href {
            tagged.inspect_href(self.path()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Strictly inspects this node as an S4 class link, using this cursor's path.
    pub fn inspect_s4_class_link(&self) -> Result<Option<crate::RdS4ClassLink<'a>>, RdShapeError> {
        self.node().inspect_s4_class_link(self.path())
    }

    /// Lossily views this node as an S4 class link, using this cursor's path.
    pub fn s4_class_link(&self) -> Option<crate::RdS4ClassLink<'a>> {
        self.node().s4_class_link(self.path())
    }

    /// Strictly inspects this node as a list, using this cursor's path.
    pub fn inspect_list(&self) -> Result<Option<crate::RdList<'a>>, RdShapeError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_any_error(
                self.node(),
                self.path(),
                &[RdTag::Itemize, RdTag::Enumerate, RdTag::Describe],
                RdExpectedNode::List,
            )
            .map(|_| None);
        };
        if !matches!(
            tagged.tag(),
            RdTag::Itemize | RdTag::Enumerate | RdTag::Describe
        ) {
            return Ok(None);
        }
        tagged.inspect_list(self.path()).map(Some)
    }

    /// Strictly inspects this node as a tabular container, using this cursor's path.
    pub fn inspect_tabular(&self) -> Result<Option<crate::RdTabular<'a>>, RdShapeError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_error(
                self.node(),
                self.path(),
                RdTag::Tabular,
                RdExpectedNode::Tabular,
            )
            .map(|_| None);
        };
        if tagged.tag() == &RdTag::Tabular {
            tagged.inspect_tabular(self.path()).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Strictly inspects this node as an equation, using this cursor's path.
    pub fn inspect_equation(&self) -> Result<Option<crate::RdEquation<'a>>, RdShapeError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_any_error(
                self.node(),
                self.path(),
                &[RdTag::Eqn, RdTag::Deqn],
                RdExpectedNode::Equation,
            )
            .map(|_| None);
        };
        if !matches!(tagged.tag(), RdTag::Eqn | RdTag::Deqn) {
            return Ok(None);
        }
        tagged.inspect_equation(self.path()).map(Some)
    }

    /// Strictly inspects this node as inline markup, using this cursor's path.
    pub fn inspect_inline_span(&self) -> Result<Option<crate::RdInlineSpan<'a>>, RdShapeError> {
        self.node().inspect_inline_span(self.path())
    }

    /// Lossily views this node as inline markup, using this cursor's path.
    pub fn inline_span(&self) -> Option<crate::RdInlineSpan<'a>> {
        self.node().inline_span(self.path())
    }

    /// Strictly inspects this node as a zero-argument text symbol.
    pub fn inspect_text_symbol(&self) -> Result<Option<crate::RdTextSymbol>, RdShapeError> {
        self.node().inspect_text_symbol(self.path())
    }

    /// Lossily views this node as a zero-argument text symbol.
    pub fn text_symbol(&self) -> Option<crate::RdTextSymbol> {
        self.node().text_symbol(self.path())
    }

    /// Strictly inspects this node as a conditional, using this cursor's path.
    pub fn inspect_conditional(&self) -> Result<Option<crate::RdConditional<'a>>, RdShapeError> {
        self.node().inspect_conditional(self.path())
    }

    /// Lossily views this node as a conditional, using this cursor's path.
    pub fn conditional(&self) -> Option<crate::RdConditional<'a>> {
        self.node().conditional(self.path())
    }

    /// Strictly inspects this node as an encoding wrapper, using this cursor's path.
    pub fn inspect_enc(&self) -> Result<Option<crate::RdEnc<'a>>, RdShapeError> {
        self.node().inspect_enc(self.path())
    }

    /// Lossily views this node as an encoding wrapper, using this cursor's path.
    pub fn enc(&self) -> Option<crate::RdEnc<'a>> {
        self.node().enc(self.path())
    }

    /// Strictly inspects this node as a method, using this cursor's path.
    pub fn inspect_method(&self) -> Result<Option<crate::RdMethod<'a>>, RdShapeError> {
        self.node().inspect_method(self.path())
    }

    /// Lossily views this node as a method, using this cursor's path.
    pub fn method(&self) -> Option<crate::RdMethod<'a>> {
        self.node().method(self.path())
    }

    /// Strictly inspects this node as a figure, using this cursor's path.
    pub fn inspect_figure(&self) -> Result<Option<crate::RdFigure<'a>>, RdShapeError> {
        self.node().inspect_figure(self.path())
    }

    /// Lossily views this node as a figure, using this cursor's path.
    pub fn figure(&self) -> Option<crate::RdFigure<'a>> {
        self.node().figure(self.path())
    }

    /// Strictly inspects this node as an example-control wrapper.
    pub fn inspect_example_control(
        &self,
    ) -> Result<Option<crate::RdExampleControl<'a>>, RdShapeError> {
        self.node().inspect_example_control(self.path())
    }

    /// Lossily views this node as an example-control wrapper.
    pub fn example_control(&self) -> Option<crate::RdExampleControl<'a>> {
        self.node().example_control(self.path())
    }

    /// Strictly inspects this node as an `\Sexpr` expression.
    pub fn inspect_sexpr(&self) -> Result<Option<crate::RdSexpr<'a>>, crate::RdOptionError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_option_error(
                self.node(),
                self.path(),
                RdTag::Sexpr,
                RdExpectedNode::Sexpr,
            )
            .map(|_| None);
        };
        if tagged.tag() != &RdTag::Sexpr {
            return Ok(None);
        }
        tagged.inspect_sexpr(self.path()).map(Some)
    }

    /// Strictly inspects this node as an `\RdOpts` update.
    pub fn inspect_rd_opts(&self) -> Result<Option<crate::RdOpts<'a>>, crate::RdOptionError> {
        let RdNode::Tagged(tagged) = self.node() else {
            return raw_match_option_error(
                self.node(),
                self.path(),
                RdTag::RdOpts,
                RdExpectedNode::RdOpts,
            )
            .map(|_| None);
        };
        if tagged.tag() != &RdTag::RdOpts {
            return Ok(None);
        }
        tagged.inspect_rd_opts(self.path()).map(Some)
    }
}

fn raw_match_error(
    node: &RdNode,
    path: &RdAstPath,
    wanted: RdTag,
    expected: RdExpectedNode,
) -> Result<(), RdShapeError> {
    raw_match_any_error(node, path, std::slice::from_ref(&wanted), expected)
}

fn raw_match_any_error(
    node: &RdNode,
    path: &RdAstPath,
    wanted: &[RdTag],
    expected: RdExpectedNode,
) -> Result<(), RdShapeError> {
    let Some(raw) = node.as_raw() else {
        return Ok(());
    };
    let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
        return Ok(());
    };
    if !wanted.iter().any(|candidate| candidate == &tag) {
        return Ok(());
    }
    Err(RdShapeError::new(
        path.clone(),
        Some(tag),
        RdShapeErrorKind::UnexpectedNode {
            expected,
            actual: RdNodeKind::Raw,
        },
    ))
}

fn raw_match_option_error(
    node: &RdNode,
    path: &RdAstPath,
    wanted: RdTag,
    expected: RdExpectedNode,
) -> Result<(), crate::RdOptionError> {
    raw_match_error(node, path, wanted, expected).map_err(Into::into)
}
