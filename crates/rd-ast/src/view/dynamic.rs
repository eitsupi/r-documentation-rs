use super::*;

/// A borrowed, structurally valid `\\Sexpr{code}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdSexpr<'a> {
    path: RdPath,
    code: &'a str,
    options: Option<RdOptionList<'a>>,
}

impl<'a> RdSexpr<'a> {
    /// Returns the `\\Sexpr` node path.
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    /// Returns the R code body.
    pub fn code(&self) -> &'a str {
        self.code
    }
    /// Returns the parsed local option list, if present.
    pub fn options(&self) -> Option<&RdOptionList<'a>> {
        self.options.as_ref()
    }
    /// Returns the valid typed local overrides.
    pub fn option_overrides(&self) -> RdSexprOptionOverrides {
        self.options
            .as_ref()
            .map_or_else(RdSexprOptionOverrides::empty, RdOptionList::typed)
    }
}

/// A borrowed, structurally valid `\\RdOpts{options}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdOpts<'a> {
    path: RdPath,
    options: RdOptionList<'a>,
}

impl<'a> RdOpts<'a> {
    /// Returns the `\\RdOpts` node path.
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    /// Returns the parsed option list.
    pub fn options(&self) -> &RdOptionList<'a> {
        &self.options
    }
    /// Returns the valid typed overrides.
    pub fn option_overrides(&self) -> RdSexprOptionOverrides {
        self.options.typed()
    }
}

/// The result of resolving one `\\Sexpr` against document-level options.
#[derive(Debug, Clone, PartialEq)]
pub struct RdResolvedSexpr<'a> {
    view: RdSexpr<'a>,
    effective: RdEffectiveSexprOptions,
}

impl<'a> RdResolvedSexpr<'a> {
    /// Returns the inspected expression view.
    pub fn view(&self) -> &RdSexpr<'a> {
        &self.view
    }
    /// Returns the effective options after local overrides.
    pub fn effective_options(&self) -> RdEffectiveSexprOptions {
        self.effective
    }
    /// Returns the resolution state.
    pub fn state(&self) -> RdDynamicMarkupState {
        RdDynamicMarkupState::Unresolved {
            stage: self.effective.stage,
        }
    }
}

/// Resolution state for a surviving dynamic markup expression.
///
/// There is no `Expanded` variant because lowering validates the producer's
/// `dynamicFlag` attribute but discards it, so the AST cannot prove that
/// surrounding content is an evaluated replacement. Every surviving
/// `\\Sexpr` is unresolved for its effective stage. Renderer fallback policy
/// (show-as-code, warn-and-omit, or require-pre-expanded) is a downstream
/// decision made by matching `Unresolved`; `rd-ast` encodes no rendering
/// policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdDynamicMarkupState {
    /// Not expanded for this evaluation stage.
    Unresolved { stage: RdSexprStage },
}

/// An event emitted while resolving dynamic markup in document order.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdDynamicMarkupEvent<'a> {
    /// A document-level option update and its resulting state.
    OptionsChanged {
        view: RdOpts<'a>,
        effective: RdEffectiveSexprOptions,
    },
    /// A `\\Sexpr` with locally resolved options.
    Sexpr(RdResolvedSexpr<'a>),
}

/// A depth-first pre-order dynamic-markup traversal.
/// The resolver deliberately mirrors R's observable `processRdSexprs`
/// semantics: one global options state is folded in document order. Conditional
/// markup (`\\if`/`\\ifelse`) is not branch-selected by this inspection pass,
/// so an `\\RdOpts` inside an arm that a renderer would not select still
/// affects subsequent `\\Sexpr` resolution.
pub struct RdDynamicMarkupIter<'a> {
    frames: Vec<RdTraversalFrame<'a>>,
    effective: RdEffectiveSexprOptions,
}

struct RdTraversalFrame<'a> {
    nodes: &'a [RdNode],
    next: usize,
    path: RdPath,
    top_level: bool,
}

impl<'a> Iterator for RdDynamicMarkupIter<'a> {
    type Item = Result<RdDynamicMarkupEvent<'a>, RdOptionError>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let frame = self.frames.last_mut()?;
            if frame.next == frame.nodes.len() {
                self.frames.pop();
                continue;
            }
            let index = frame.next;
            frame.next += 1;
            let path = if frame.top_level {
                RdPath::new(vec![RdPathSegment::TopLevel(index)])
            } else {
                frame.path.with_child(index)
            };
            let node = &frame.nodes[index];
            if let Some(tagged) = node.as_tagged() {
                let result = match tagged.tag() {
                    RdTag::RdOpts => match tagged.inspect_rd_opts(&path) {
                        Ok(view) => {
                            let mut effective = self.effective;
                            view.option_overrides().apply_to(&mut effective);
                            self.effective = effective;
                            Ok(RdDynamicMarkupEvent::OptionsChanged { view, effective })
                        }
                        Err(error) => Err(error),
                    },
                    RdTag::Sexpr => match tagged.inspect_sexpr(&path) {
                        Ok(view) => {
                            let mut effective = self.effective;
                            view.option_overrides().apply_to(&mut effective);
                            Ok(RdDynamicMarkupEvent::Sexpr(RdResolvedSexpr {
                                view,
                                effective,
                            }))
                        }
                        Err(error) => Err(error),
                    },
                    _ => {
                        self.push_children(tagged.children(), path);
                        continue;
                    }
                };
                self.push_children(tagged.children(), path);
                return Some(result);
            }
            if let RdNode::Group(group) = node {
                self.push_children(group.children(), path);
            }
        }
    }
}

impl<'a> RdDynamicMarkupIter<'a> {
    fn push_children(&mut self, nodes: &'a [RdNode], path: RdPath) {
        self.frames.push(RdTraversalFrame {
            nodes,
            next: 0,
            path,
            top_level: false,
        });
    }
}

impl RdDocument {
    /// Returns a depth-first pre-order resolver for `\\RdOpts` and `\\Sexpr`.
    ///
    /// The resolver deliberately mirrors R's observable `processRdSexprs`
    /// semantics: a single options state is folded in document order.
    /// Conditional markup (`\\if`/`\\ifelse`) is not branch-selected here;
    /// therefore an `\\RdOpts` in an arm that a renderer would not select
    /// still affects later `\\Sexpr` resolution, matching R's behavior.
    pub fn inspect_dynamic_markup(&self) -> RdDynamicMarkupIter<'_> {
        RdDynamicMarkupIter {
            frames: vec![RdTraversalFrame {
                nodes: self.nodes(),
                next: 0,
                path: RdPath::new(vec![]),
                top_level: true,
            }],
            effective: RdEffectiveSexprOptions::default(),
        }
    }
}

impl RdTagged {
    /// Strictly inspects a `\\Sexpr{code}` node.
    pub fn inspect_sexpr<'a>(&'a self, base_path: &RdPath) -> Result<RdSexpr<'a>, RdOptionError> {
        if self.tag() != &RdTag::Sexpr {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Sexpr,
                    actual: RdNodeKind::Tagged,
                },
            )
            .into());
        }
        let children = self.children();
        if children.len() != 1 {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Sexpr),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(1),
                    actual: children.len(),
                },
            )
            .into());
        }
        let code = match &children[0] {
            RdNode::RCode(code) => code.as_str(),
            node => {
                return Err(shape(
                    base_path.with_child(0),
                    Some(RdTag::Sexpr),
                    RdShapeErrorKind::UnexpectedContent {
                        actual: RdNodeKind::of(node),
                    },
                )
                .into());
            }
        };
        let options = self
            .option()
            .map(|nodes| RdOptionList::parse(nodes, base_path.with_option()))
            .transpose()?;
        Ok(RdSexpr {
            path: base_path.clone(),
            code,
            options,
        })
    }

    /// Strictly inspects a `\\RdOpts{options}` node.
    pub fn inspect_rd_opts<'a>(&'a self, base_path: &RdPath) -> Result<RdOpts<'a>, RdOptionError> {
        if self.tag() != &RdTag::RdOpts {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::RdOpts,
                    actual: RdNodeKind::Tagged,
                },
            )
            .into());
        }
        if self.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::RdOpts),
                RdShapeErrorKind::UnexpectedOption,
            )
            .into());
        }
        let options = RdOptionList::parse(self.children(), base_path.clone())?;
        Ok(RdOpts {
            path: base_path.clone(),
            options,
        })
    }
}
