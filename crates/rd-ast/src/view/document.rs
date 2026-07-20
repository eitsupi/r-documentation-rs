use super::list::inspect_two_group_item;
use super::*;

/// A custom `\section{title}{body}` node (see [`RdDocument::sections`]).
///
/// Not the standard, fixed-vocabulary sections (`\description`, `\value`,
/// ...) -- those are read individually via
/// [`RdDocument::title`]/[`RdDocument::description`]/etc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdSection<'a> {
    /// The section's title, i.e. `\section{title}{...}`'s first argument
    /// group.
    pub title: &'a [RdNode],
    /// The section's body, i.e. `\section{...}{body}`'s second argument
    /// group.
    pub body: &'a [RdNode],
}

/// A single `\item{name}{description}` entry within `\arguments` (see
/// [`RdDocument::arguments`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdArgument<'a> {
    /// The argument's name, i.e. `\item{name}{...}`'s first argument
    /// group.
    pub name: &'a [RdNode],
    /// The argument's description, i.e. `\item{...}{description}`'s
    /// second argument group.
    pub description: &'a [RdNode],
}

/// A borrowed, structurally valid `\alias{...}` view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdAlias<'a> {
    nodes: &'a [RdNode],
}

/// A borrowed, structurally valid `\keyword{...}` view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdKeyword<'a> {
    nodes: &'a [RdNode],
}

impl<'a> RdKeyword<'a> {
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }
    pub fn text_contents(&self) -> String {
        text_contents(self.nodes)
    }
}

/// A borrowed, structurally valid `\concept{...}` view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdConcept<'a> {
    nodes: &'a [RdNode],
}

impl<'a> RdConcept<'a> {
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }
    pub fn text_contents(&self) -> String {
        text_contents(self.nodes)
    }
}

/// The kind of custom section-family node visited by [`RdDocument::section_tree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdSectionKind {
    Section,
    Subsection,
}

/// A structurally valid custom section-family node in a document.
#[derive(Debug, Clone, PartialEq)]
pub struct RdSectionVisit<'a> {
    path: RdPath,
    kind: RdSectionKind,
    nesting: usize,
    title: &'a [RdNode],
    body: &'a [RdNode],
}

impl<'a> RdSectionVisit<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn kind(&self) -> RdSectionKind {
        self.kind
    }
    /// Returns syntactic nesting only; heading-level and rendering policy
    /// belong to consumers.
    pub fn nesting(&self) -> usize {
        self.nesting
    }
    pub fn title(&self) -> &'a [RdNode] {
        self.title
    }
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }
}

impl<'a> RdAlias<'a> {
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }
    pub fn text_contents(&self) -> String {
        text_contents(self.nodes)
    }
}
impl RdDocument {
    /// Lossy: returns the first top-level `\title{...}`'s children, if any.
    /// See [`Self::inspect_title`] for diagnostics.
    ///
    /// "First-wins": if `\title` appears more than once at the top level
    /// (malformed input), only the first occurrence is returned. Only
    /// [`RdNode::Tagged`] top-level nodes are considered -- see the
    /// module-level documentation for why a top-level [`RdNode::Raw`] node
    /// whose tag string happens to be `"\\title"` is not matched.
    pub fn title(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Title)
    }

    /// Lossy: returns the first top-level `\description{...}`'s children, if any.
    /// See [`Self::inspect_description`] for diagnostics.
    ///
    /// Same first-wins-on-duplicates and `Tagged`-only semantics as
    /// [`RdDocument::title`].
    pub fn description(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Description)
    }

    /// Lossy: returns the first top-level `\usage{...}`'s children, if any.
    /// See [`Self::inspect_usage`] for diagnostics.
    ///
    /// Same first-wins-on-duplicates and `Tagged`-only semantics as
    /// [`RdDocument::title`].
    pub fn usage(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Usage)
    }

    /// Lossy: returns the first top-level `\value{...}`'s children, if any.
    /// See [`Self::inspect_value`] for diagnostics.
    ///
    /// Same first-wins-on-duplicates and `Tagged`-only semantics as
    /// [`RdDocument::title`].
    pub fn value(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Value)
    }

    /// Lossy: returns the first top-level `\name{...}`'s children, if any.
    /// See [`Self::inspect_name`] for diagnostics. First-wins on duplicates.
    pub fn name(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Name)
    }
    /// Lossy: returns the first top-level `\details{...}`'s children, if any.
    /// See [`Self::inspect_details`] for diagnostics. First-wins on duplicates.
    pub fn details(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Details)
    }
    /// Lossy: returns the first top-level `\note{...}`'s children, if any.
    /// See [`Self::inspect_note`] for diagnostics. First-wins on duplicates.
    pub fn note(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Note)
    }
    /// Lossy: returns the first top-level `\author{...}`'s children, if any.
    /// See [`Self::inspect_author`] for diagnostics. First-wins on duplicates.
    pub fn author(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Author)
    }
    /// Lossy: returns the first top-level `\references{...}`'s children, if any.
    /// See [`Self::inspect_references`] for diagnostics. First-wins on duplicates.
    pub fn references(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::References)
    }
    /// Lossy: returns the first top-level `\seealso{...}`'s children, if any.
    /// See [`Self::inspect_see_also`] for diagnostics. First-wins on duplicates.
    pub fn see_also(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::SeeAlso)
    }
    /// Lossy: returns the first top-level `\examples{...}`'s children, if any.
    /// See [`Self::inspect_examples`] for diagnostics. First-wins on duplicates.
    pub fn examples(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Examples)
    }
    /// Lossy: returns the first top-level `\format{...}`'s children, if any.
    /// See [`Self::inspect_format`] for diagnostics. First-wins on duplicates.
    pub fn format(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Format)
    }
    /// Lossy: returns the first top-level `\source{...}`'s children, if any.
    /// See [`Self::inspect_source`] for diagnostics. First-wins on duplicates.
    pub fn source(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Source)
    }
    /// Lossy: returns the first top-level `\encoding{...}`'s children, if any.
    /// See [`Self::inspect_encoding`] for diagnostics. First-wins on duplicates.
    pub fn encoding(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Encoding)
    }
    /// Lossy: returns the first top-level `\docType{...}`'s children, if any.
    /// See [`Self::inspect_doc_type`] for diagnostics. First-wins on duplicates.
    pub fn doc_type(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::DocType)
    }
    /// Lossy: returns the first top-level `\Rdversion{...}`'s children, if any.
    /// See [`Self::inspect_rd_version`] for diagnostics. First-wins on duplicates.
    pub fn rd_version(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Rdversion)
    }
    /// Lossy: returns the first top-level `\synopsis{...}`'s children, if any.
    /// See [`Self::inspect_synopsis`] for diagnostics. First-wins on duplicates.
    pub fn synopsis(&self) -> Option<&[RdNode]> {
        self.first_tagged_children(RdTag::Synopsis)
    }

    /// The children of the first top-level `Tagged` node with the given
    /// `tag`, if any.
    fn first_tagged_children(&self, tag: RdTag) -> Option<&[RdNode]> {
        self.nodes().iter().find_map(|node| {
            let tagged = node.as_tagged()?;
            (tagged.tag() == &tag).then(|| tagged.children())
        })
    }

    /// Lossy: iterates the current topic's `\alias{...}` declarations, in source
    /// order, yielding each alias's [`text_contents`] (no splitting, no
    /// trimming -- an alias with empty children yields an empty string).
    ///
    /// This reads the topic's *own* `\alias` markup, i.e. the alias names
    /// this specific Rd document declares for itself. It is unrelated to a
    /// package-wide `help/aliases.rds` index (as read by e.g.
    /// `rd-helpdb`'s `PackageHelpDb::aliases`/`resolve_alias`), which maps
    /// every alias in a package to the topic file that documents it.
    ///
    /// Only top-level [`RdNode::Tagged`] nodes are considered -- see the
    /// module-level documentation for why a top-level [`RdNode::Raw`] node
    /// whose tag string happens to be `"\\alias"` is not matched.
    pub fn aliases(&self) -> impl Iterator<Item = String> + '_ {
        self.nodes().iter().filter_map(|node| {
            let tagged = node.as_tagged()?;
            (tagged.tag() == &RdTag::Alias).then(|| text_contents(tagged.children()))
        })
    }

    /// Lossy: iterates top-level `\keyword{...}` declarations in source order.
    pub fn keywords(&self) -> impl Iterator<Item = String> + '_ {
        self.nodes().iter().filter_map(|node| {
            let tagged = node.as_tagged()?;
            (tagged.tag() == &RdTag::Keyword).then(|| text_contents(tagged.children()))
        })
    }

    /// Lossy: iterates top-level `\concept{...}` declarations in source order.
    pub fn concepts(&self) -> impl Iterator<Item = String> + '_ {
        self.nodes().iter().filter_map(|node| {
            let tagged = node.as_tagged()?;
            (tagged.tag() == &RdTag::Concept).then(|| text_contents(tagged.children()))
        })
    }

    /// Lossy: iterates the document's custom `\section{title}{body}` nodes, in
    /// source order.
    ///
    /// This is distinct from the standard, fixed-vocabulary sections
    /// (`\description`, `\value`, `\details`, ...), which are read
    /// individually (see [`RdDocument::title`], [`RdDocument::description`],
    /// etc.). It only recognizes custom `\section{...}{...}` markup
    /// (`RdTag::Section`).
    ///
    /// A top-level node is recognized as a section only when it is a
    /// `Tagged` node with `RdTag::Section`, no option, and
    /// exactly two `RdNode::Group` children (the title/body positional-
    /// argument groups the `\section` macro lowers to). Anything else is
    /// silently skipped rather than erroring.
    /// For nested `\subsection` traversal, see [`Self::section_tree`].
    pub fn sections(&self) -> impl Iterator<Item = RdSection<'_>> {
        self.nodes().iter().filter_map(|node| {
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Section || tagged.option().is_some() {
                return None;
            }
            let [RdNode::Group(title), RdNode::Group(body)] = tagged.children() else {
                return None;
            };
            Some(RdSection {
                title: title.children(),
                body: body.children(),
            })
        })
    }

    /// Lossy: iterates the `\item{name}{description}` entries of the first
    /// top-level `\arguments{...}` node, in source order.
    ///
    /// Only the FIRST top-level `Tagged` node with `RdTag::Arguments`
    /// node is considered (first-wins on duplicates, like
    /// [`RdDocument::title`]); its DIRECT children are then scanned --
    /// nested `\arguments` are not (there is no such thing in valid Rd,
    /// but this function makes no attempt to look for one either way).
    ///
    /// A direct child yields an [`RdArgument`] only when it matches
    /// a `Tagged` node with `RdTag::Item`, no option, and the given children where
    /// `children` has exactly two `RdNode::Group` children (the
    /// name/description positional-argument groups the two-argument form of
    /// `\item` lowers to). Everything else is
    /// silently skipped: the whitespace `TEXT` leaves between `\item`s,
    /// an `\item[label]` with an option (the zero/one-argument
    /// `\itemize`/`\enumerate` form, not the two-argument
    /// `\arguments`/`\describe` form), an `\item` with the wrong number of
    /// children, and any other non-`\item` markup. This is a total
    /// function -- it never errors, it just yields fewer items than the
    /// source `\arguments` block might structurally contain.
    ///
    /// If `\arguments` itself isn't found as a top-level `Tagged` node
    /// (see the module-level documentation on why `Raw` is never
    /// interpreted), this yields no items.
    pub fn arguments(&self) -> impl Iterator<Item = RdArgument<'_>> {
        let children = self.first_tagged_children(RdTag::Arguments).unwrap_or(&[]);
        children.iter().filter_map(|node| {
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Item || tagged.option().is_some() {
                return None;
            }
            let [RdNode::Group(name), RdNode::Group(description)] = tagged.children() else {
                return None;
            };
            Some(RdArgument {
                name: name.children(),
                description: description.children(),
            })
        })
    }

    /// Strict counterpart to the lossy [`Self::title`] accessor.
    pub fn inspect_title(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Title)
    }
    /// Strict counterpart to the lossy [`Self::description`] accessor.
    pub fn inspect_description(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Description)
    }
    /// Strict counterpart to the lossy [`Self::usage`] accessor.
    pub fn inspect_usage(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Usage)
    }
    /// Strict counterpart to the lossy [`Self::value`] accessor.
    pub fn inspect_value(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Value)
    }

    /// Strict counterpart to the lossy [`Self::name`] accessor.
    pub fn inspect_name(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Name)
    }
    /// Strict counterpart to the lossy [`Self::details`] accessor.
    pub fn inspect_details(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Details)
    }
    /// Strict counterpart to the lossy [`Self::note`] accessor.
    pub fn inspect_note(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Note)
    }
    /// Strict counterpart to the lossy [`Self::author`] accessor.
    pub fn inspect_author(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Author)
    }
    /// Strict counterpart to the lossy [`Self::references`] accessor.
    pub fn inspect_references(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::References)
    }
    /// Strict counterpart to the lossy [`Self::see_also`] accessor.
    pub fn inspect_see_also(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::SeeAlso)
    }
    /// Strict counterpart to the lossy [`Self::examples`] accessor.
    pub fn inspect_examples(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Examples)
    }
    /// Strict counterpart to the lossy [`Self::format`] accessor.
    pub fn inspect_format(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Format)
    }
    /// Strict counterpart to the lossy [`Self::source`] accessor.
    pub fn inspect_source(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Source)
    }
    /// Strict counterpart to the lossy [`Self::encoding`] accessor.
    pub fn inspect_encoding(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Encoding)
    }
    /// Strict counterpart to the lossy [`Self::doc_type`] accessor.
    pub fn inspect_doc_type(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::DocType)
    }
    /// Strict counterpart to the lossy [`Self::rd_version`] accessor.
    pub fn inspect_rd_version(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Rdversion)
    }
    /// Strict counterpart to the lossy [`Self::synopsis`] accessor.
    pub fn inspect_synopsis(&self) -> Result<Option<&[RdNode]>, RdShapeError> {
        self.inspect_fixed(RdTag::Synopsis)
    }

    fn inspect_fixed(&self, wanted: RdTag) -> Result<Option<&[RdNode]>, RdShapeError> {
        let mut found: Option<(usize, &[RdNode])> = None;
        for (index, node) in self.nodes().iter().enumerate() {
            let path = top_path(index);
            if let Some(raw) = node.as_raw() {
                if raw.tag() == Some(wanted.as_rd_tag()) {
                    return Err(shape(
                        path,
                        Some(wanted.clone()),
                        RdShapeErrorKind::UnexpectedNode {
                            expected: RdExpectedNode::Tagged,
                            actual: RdNodeKind::Raw,
                        },
                    ));
                }
                continue;
            }
            let Some(tagged) = node.as_tagged() else {
                continue;
            };
            if tagged.tag() != &wanted {
                continue;
            }
            if let Some((first_index, _)) = found {
                let first_path = top_path(first_index);
                return Err(shape(
                    path,
                    Some(wanted.clone()),
                    RdShapeErrorKind::Duplicate {
                        construct: RdConstruct::Tag(wanted.clone()),
                        first_path,
                    },
                ));
            }
            if tagged.option().is_some() {
                return Err(shape(
                    path,
                    Some(wanted.clone()),
                    RdShapeErrorKind::UnexpectedOption,
                ));
            }
            found = Some((index, tagged.children()));
        }
        Ok(found.map(|(_, children)| children))
    }

    /// Strict counterpart to the lossy [`Self::aliases`] accessor.
    pub fn inspect_aliases(&self) -> impl Iterator<Item = Result<RdAlias<'_>, RdShapeError>> + '_ {
        self.nodes().iter().enumerate().filter_map(|(index, node)| {
            let path = top_path(index);
            if node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(RdTag::Alias.as_rd_tag()))
            {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Alias),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                )));
            }
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Alias {
                return None;
            }
            if tagged.option().is_some() {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Alias),
                    RdShapeErrorKind::UnexpectedOption,
                )));
            }
            Some(Ok(RdAlias {
                nodes: tagged.children(),
            }))
        })
    }

    /// Strict counterpart to the lossy [`Self::keywords`] accessor.
    pub fn inspect_keywords(
        &self,
    ) -> impl Iterator<Item = Result<RdKeyword<'_>, RdShapeError>> + '_ {
        self.nodes().iter().enumerate().filter_map(|(index, node)| {
            let path = top_path(index);
            if node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(RdTag::Keyword.as_rd_tag()))
            {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Keyword),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                )));
            }
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Keyword {
                return None;
            }
            if tagged.option().is_some() {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Keyword),
                    RdShapeErrorKind::UnexpectedOption,
                )));
            }
            Some(Ok(RdKeyword {
                nodes: tagged.children(),
            }))
        })
    }

    /// Strict counterpart to the lossy [`Self::concepts`] accessor.
    pub fn inspect_concepts(
        &self,
    ) -> impl Iterator<Item = Result<RdConcept<'_>, RdShapeError>> + '_ {
        self.nodes().iter().enumerate().filter_map(|(index, node)| {
            let path = top_path(index);
            if node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(RdTag::Concept.as_rd_tag()))
            {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Concept),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                )));
            }
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Concept {
                return None;
            }
            if tagged.option().is_some() {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Concept),
                    RdShapeErrorKind::UnexpectedOption,
                )));
            }
            Some(Ok(RdConcept {
                nodes: tagged.children(),
            }))
        })
    }

    /// Lossy depth-first preorder traversal of recognized top-level
    /// `\section` nodes and recognized nested `\subsection` nodes.
    ///
    /// Malformed candidates and their descendants are skipped. An orphan
    /// top-level `\subsection` is outside this view; source-parser diagnostics
    /// own that condition. See [`Self::sections`] for the top-level-only view.
    pub fn section_tree(&self) -> impl Iterator<Item = RdSectionVisit<'_>> {
        let mut visits = Vec::new();
        for (index, node) in self.nodes().iter().enumerate() {
            collect_section_visits(
                node,
                top_path(index),
                RdSectionKind::Section,
                0,
                false,
                &mut visits,
            );
        }
        visits.into_iter().filter_map(Result::ok)
    }

    /// Strict depth-first preorder traversal of recognized top-level
    /// `\section` nodes and recognized nested `\subsection` nodes.
    ///
    /// Each malformed candidate yields one error and its descendants are not
    /// traversed. An orphan top-level `\subsection` is outside this view;
    /// source-parser diagnostics own that condition. See [`Self::sections`]
    /// for the top-level-only view.
    pub fn inspect_section_tree(
        &self,
    ) -> impl Iterator<Item = Result<RdSectionVisit<'_>, RdShapeError>> {
        let mut visits = Vec::new();
        for (index, node) in self.nodes().iter().enumerate() {
            collect_section_visits(
                node,
                top_path(index),
                RdSectionKind::Section,
                0,
                true,
                &mut visits,
            );
        }
        visits.into_iter()
    }

    /// Strict counterpart to the lossy [`Self::sections`] accessor.
    pub fn inspect_sections(
        &self,
    ) -> impl Iterator<Item = Result<RdSection<'_>, RdShapeError>> + '_ {
        self.nodes().iter().enumerate().filter_map(|(index, node)| {
            let path = top_path(index);
            if node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(RdTag::Section.as_rd_tag()))
            {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Section),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                )));
            }
            let tagged = node.as_tagged()?;
            if tagged.tag() != &RdTag::Section {
                return None;
            }
            if tagged.option().is_some() {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Section),
                    RdShapeErrorKind::UnexpectedOption,
                )));
            }
            if tagged.children().len() != 2 {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Section),
                    RdShapeErrorKind::WrongArity {
                        expected: RdArity::Exactly(2),
                        actual: tagged.children().len(),
                    },
                )));
            }
            let [title, body] = tagged.children() else {
                unreachable!()
            };
            for (child_index, child) in tagged.children().iter().enumerate() {
                if child.as_group().is_none() {
                    return Some(Err(shape(
                        child_path(index, child_index),
                        Some(RdTag::Section),
                        RdShapeErrorKind::UnexpectedNode {
                            expected: RdExpectedNode::Group,
                            actual: RdNodeKind::of(child),
                        },
                    )));
                }
            }
            Some(Ok(RdSection {
                title: title.as_group().unwrap().children(),
                body: body.as_group().unwrap().children(),
            }))
        })
    }

    /// Strict counterpart to the lossy [`Self::arguments`] accessor.
    pub fn inspect_arguments(
        &self,
    ) -> Result<impl Iterator<Item = Result<RdArgument<'_>, RdShapeError>> + '_, RdShapeError> {
        let mut container = None;
        for (index, node) in self.nodes().iter().enumerate() {
            let path = top_path(index);
            if node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(RdTag::Arguments.as_rd_tag()))
            {
                return Err(shape(
                    path,
                    Some(RdTag::Arguments),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                ));
            }
            let Some(tagged) = node.as_tagged() else {
                continue;
            };
            if tagged.tag() != &RdTag::Arguments {
                continue;
            }
            if container.is_some() {
                let first_index = container.map(|(index, _)| index).unwrap_or(index);
                return Err(shape(
                    path,
                    Some(RdTag::Arguments),
                    RdShapeErrorKind::Duplicate {
                        construct: RdConstruct::Tag(RdTag::Arguments),
                        first_path: top_path(first_index),
                    },
                ));
            }
            if tagged.option().is_some() {
                return Err(shape(
                    path,
                    Some(RdTag::Arguments),
                    RdShapeErrorKind::UnexpectedOption,
                ));
            }
            container = Some((index, tagged.children()));
        }
        let parent_index = container.map_or(0, |(index, _)| index);
        let children = container.map_or(&[][..], |(_, children)| children);
        Ok(children
            .iter()
            .enumerate()
            .filter_map(move |(index, node)| {
                if is_inter_item_trivia(node) {
                    return None;
                }
                let path = child_path(parent_index, index);
                let Some(tagged) = node.as_tagged() else {
                    return Some(Err(shape(
                        path,
                        node_tag(node),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::of(node),
                        },
                    )));
                };
                if tagged.tag() != &RdTag::Item {
                    return Some(Err(shape(
                        path,
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::Tagged,
                        },
                    )));
                }
                let (name, description) = match inspect_two_group_item(tagged, &path) {
                    Ok(groups) => groups,
                    Err(error) => return Some(Err(error)),
                };
                Some(Ok(RdArgument { name, description }))
            }))
    }
}

fn collect_section_visits<'a>(
    node: &'a RdNode,
    path: RdPath,
    kind: RdSectionKind,
    nesting: usize,
    strict: bool,
    output: &mut Vec<Result<RdSectionVisit<'a>, RdShapeError>>,
) {
    let wanted = match kind {
        RdSectionKind::Section => RdTag::Section,
        RdSectionKind::Subsection => RdTag::Subsection,
    };
    let Some(tagged) = node.as_tagged() else {
        if strict
            && node
                .as_raw()
                .is_some_and(|raw| raw.tag() == Some(wanted.as_rd_tag()))
        {
            output.push(Err(shape(
                path,
                Some(wanted),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Tagged,
                    actual: RdNodeKind::Raw,
                },
            )));
        }
        return;
    };
    if tagged.tag() != &wanted {
        return;
    }
    if tagged.option().is_some() {
        if strict {
            output.push(Err(shape(
                path,
                Some(wanted),
                RdShapeErrorKind::UnexpectedOption,
            )));
        }
        return;
    }
    if tagged.children().len() != 2 {
        if strict {
            output.push(Err(shape(
                path,
                Some(wanted),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(2),
                    actual: tagged.children().len(),
                },
            )));
        }
        return;
    }
    let [title, body] = tagged.children() else {
        unreachable!()
    };
    if let Some((child_index, child)) = tagged
        .children()
        .iter()
        .enumerate()
        .find(|(_, child)| child.as_group().is_none())
    {
        if strict {
            output.push(Err(shape(
                path.with_child(child_index),
                Some(wanted),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(child),
                },
            )));
        }
        return;
    }
    let title = title.as_group().unwrap().children();
    let body = body.as_group().unwrap().children();
    output.push(Ok(RdSectionVisit {
        path: path.clone(),
        kind,
        nesting,
        title,
        body,
    }));

    let body_path = path.with_child(1);
    for (index, child) in body.iter().enumerate() {
        collect_section_visits(
            child,
            body_path.with_child(index),
            RdSectionKind::Subsection,
            nesting + 1,
            strict,
            output,
        );
    }
}
