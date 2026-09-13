use super::*;
use crate::{RdNodeRef, RdNodesRef};

/// A located singleton top-level field such as `\title{...}` or
/// `\description{...}`.
#[derive(Debug, Clone, PartialEq)]
pub struct RdField<'a> {
    node: &'a RdNode,
    path: RdAstPath,
    tag: RdTag,
    body: &'a [RdNode],
    body_ref: RdNodesRef<'a>,
}

impl<'a> RdField<'a> {
    pub(super) fn new(
        node: &'a RdNode,
        path: RdAstPath,
        tag: RdTag,
        body_ref: RdNodesRef<'a>,
    ) -> Self {
        Self {
            node,
            path,
            tag,
            body: body_ref.nodes(),
            body_ref,
        }
    }

    /// Returns the original tagged node represented by this field.
    pub fn node(&self) -> &'a RdNode {
        self.node
    }

    /// Returns the original tagged node as a cursor at [`Self::path`].
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the tagged node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the recognized tag of the original node.
    pub fn tag(&self) -> &RdTag {
        &self.tag
    }

    /// Returns the legacy unpositioned body slice.
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }

    /// Returns the body as a positioned sibling sequence below [`Self::path`].
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        self.body_ref.clone()
    }
}

/// A custom `\section{title}{body}` node (see [`RdDocument::sections`]).
///
/// Not the standard, fixed-vocabulary sections (`\description`, `\value`,
/// ...) -- those are read individually via
/// [`RdDocument::title`]/[`RdDocument::description`]/etc.
#[derive(Debug, Clone, PartialEq)]
pub struct RdSection<'a> {
    pub(super) path: RdAstPath,
    pub(super) node: &'a RdNode,
    pub(super) title_group: &'a RdNode,
    pub(super) body_group: &'a RdNode,
    pub(super) title: &'a [RdNode],
    pub(super) body: &'a [RdNode],
}

impl<'a> RdSection<'a> {
    /// Returns the custom section node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the original custom section node.
    pub fn node(&self) -> &'a RdNode {
        self.node
    }

    /// Returns the custom section node as a cursor at [`Self::path`].
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the legacy unpositioned title slice.
    pub fn title(&self) -> &'a [RdNode] {
        self.title
    }

    /// Returns the legacy unpositioned body slice.
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }

    /// Returns the title as a positioned sequence at child index `0`.
    pub fn title_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.title_group, self.path.with_child(0)).children()
    }

    /// Returns the body as a positioned sequence at child index `1`.
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.body_group, self.path.with_child(1)).children()
    }
}

/// A single `\item{name}{description}` entry within `\arguments` (see
/// [`RdDocument::arguments`]).
#[derive(Debug, Clone, PartialEq)]
pub struct RdArgument<'a> {
    pub(super) path: RdAstPath,
    pub(super) node: &'a RdNode,
    pub(super) name_group: &'a RdNode,
    pub(super) description_group: &'a RdNode,
    pub(super) name: &'a [RdNode],
    pub(super) description: &'a [RdNode],
}

impl<'a> RdArgument<'a> {
    /// Returns this `\item` node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the original `\item` node.
    pub fn node(&self) -> &'a RdNode {
        self.node
    }

    /// Returns the `\item` node as a cursor at [`Self::path`].
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the legacy unpositioned argument-name slice.
    pub fn name(&self) -> &'a [RdNode] {
        self.name
    }

    /// Returns the legacy unpositioned description slice.
    pub fn description(&self) -> &'a [RdNode] {
        self.description
    }

    /// Returns the argument name as a positioned sequence at child index `0`.
    pub fn name_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.name_group, self.path.with_child(0)).children()
    }

    /// Returns the description as a positioned sequence at child index `1`.
    pub fn description_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.description_group, self.path.with_child(1)).children()
    }
}

/// A borrowed, structurally valid `\alias{...}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdAlias<'a> {
    pub(super) node: &'a RdNode,
    pub(super) path: RdAstPath,
    pub(super) nodes: &'a [RdNode],
}

/// A borrowed, structurally valid `\keyword{...}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdKeyword<'a> {
    pub(super) node: &'a RdNode,
    pub(super) path: RdAstPath,
    pub(super) nodes: &'a [RdNode],
}

impl<'a> RdKeyword<'a> {
    /// Returns the keyword body as an unpositioned slice.
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }

    /// Returns the keyword node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the original keyword node as a positioned cursor.
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the keyword body as a positioned sibling sequence.
    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.node_ref().children()
    }

    /// Returns the lossy flattened keyword text.
    pub fn text_contents(&self) -> String {
        text_contents(self.nodes)
    }
}

/// A borrowed, structurally valid `\concept{...}` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdConcept<'a> {
    pub(super) node: &'a RdNode,
    pub(super) path: RdAstPath,
    pub(super) nodes: &'a [RdNode],
}

impl<'a> RdConcept<'a> {
    /// Returns the concept body as an unpositioned slice.
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }

    /// Returns the concept node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the original concept node as a positioned cursor.
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the concept body as a positioned sibling sequence.
    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.node_ref().children()
    }

    /// Returns the lossy flattened concept text.
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
    pub(super) path: RdAstPath,
    pub(super) node: &'a RdNode,
    pub(super) kind: RdSectionKind,
    pub(super) nesting: usize,
    pub(super) title: &'a [RdNode],
    pub(super) body: &'a [RdNode],
    pub(super) title_group: &'a RdNode,
    pub(super) body_group: &'a RdNode,
}

impl<'a> RdSectionVisit<'a> {
    /// Returns the visited section node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns whether this visit is a section or subsection.
    pub fn kind(&self) -> RdSectionKind {
        self.kind
    }
    /// Returns syntactic nesting only; heading-level and rendering policy
    /// belong to consumers.
    pub fn nesting(&self) -> usize {
        self.nesting
    }

    /// Returns the legacy unpositioned title slice.
    pub fn title(&self) -> &'a [RdNode] {
        self.title
    }

    /// Returns the legacy unpositioned body slice.
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }

    /// Returns the visited section node as a positioned cursor.
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the title as a positioned sequence at child index `0`.
    pub fn title_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.title_group, self.path.with_child(0)).children()
    }

    /// Returns the body as a positioned sequence at child index `1`.
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.body_group, self.path.with_child(1)).children()
    }
}

impl<'a> RdAlias<'a> {
    /// Returns the alias body as an unpositioned slice.
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes
    }

    /// Returns the alias node's canonical AST path.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns the original alias node as a positioned cursor.
    pub fn node_ref(&self) -> RdNodeRef<'a> {
        RdNodeRef::new(self.node, self.path.clone())
    }

    /// Returns the alias body as a positioned sibling sequence.
    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.node_ref().children()
    }

    /// Returns the lossy flattened alias text.
    pub fn text_contents(&self) -> String {
        text_contents(self.nodes)
    }
}
