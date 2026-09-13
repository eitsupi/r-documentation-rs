//! Borrowed, position-aware navigation through an [`RdDocument`].
//!
//! Paths and ranges in this module are coordinates in one document snapshot.
//! They do not carry an owner identity, so applying a path or range to a
//! different document (or to a later edited snapshot) is not prevented by the
//! type system.

use std::ops::Range;

use crate::{RdAstPath, RdAstPathSegment, RdDocument, RdNode, RdOptionError, RdOptionList};

/// A borrowed AST node together with its canonical location.
#[derive(Debug, Clone, PartialEq)]
pub struct RdNodeRef<'a> {
    node: &'a RdNode,
    path: RdAstPath,
}

impl<'a> RdNodeRef<'a> {
    pub(crate) fn new(node: &'a RdNode, path: RdAstPath) -> Self {
        Self { node, path }
    }

    /// Returns the underlying node without copying it.
    pub fn node(&self) -> &'a RdNode {
        self.node
    }

    /// Returns this node's canonical path in its document snapshot.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    /// Returns this node's positional children, preserving their locations.
    /// Leaf nodes consequently return an empty sequence.
    pub fn children(&self) -> RdNodesRef<'a> {
        RdNodesRef::children(self.node, &self.path)
    }

    /// Returns the present bracket option, including when it is empty.
    pub fn option(&self) -> Option<RdOptionRef<'a>> {
        let nodes = match self.node {
            RdNode::Tagged(tagged) => tagged.option(),
            RdNode::Raw(raw) => raw.option(),
            RdNode::Text(_)
            | RdNode::RCode(_)
            | RdNode::Verb(_)
            | RdNode::Comment(_)
            | RdNode::Group(_) => None,
        }?;
        Some(RdOptionRef::new(nodes, self.path.with_option()))
    }
}

/// A borrowed sequence of sibling nodes in one AST container.
///
/// The range returned by [`Self::range`] uses absolute sibling indices in the
/// original container. Slicing never renumbers nodes, including after several
/// successive slices.
#[derive(Debug, Clone, PartialEq)]
pub struct RdNodesRef<'a> {
    nodes: &'a [RdNode],
    container_path: RdAstPath,
    kind: ContainerKind,
    start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    Root,
    Child,
    Option,
}

impl<'a> RdNodesRef<'a> {
    pub(crate) fn from_slice(nodes: &'a [RdNode], container_path: RdAstPath) -> Self {
        Self::from_slice_at(nodes, container_path, 0)
    }

    pub(crate) fn from_slice_at(
        nodes: &'a [RdNode],
        container_path: RdAstPath,
        start: usize,
    ) -> Self {
        Self {
            nodes,
            container_path,
            kind: ContainerKind::Child,
            start,
        }
    }

    pub(crate) fn root(nodes: &'a [RdNode]) -> Self {
        Self {
            nodes,
            container_path: RdAstPath::new(Vec::new()),
            kind: ContainerKind::Root,
            start: 0,
        }
    }

    fn children(node: &'a RdNode, path: &RdAstPath) -> Self {
        let nodes = match node {
            RdNode::Tagged(tagged) => tagged.children(),
            RdNode::Group(group) => group.children(),
            RdNode::Raw(raw) => raw.children(),
            RdNode::Text(_) | RdNode::RCode(_) | RdNode::Verb(_) | RdNode::Comment(_) => &[],
        };
        Self {
            nodes,
            container_path: path.clone(),
            kind: ContainerKind::Child,
            start: 0,
        }
    }

    fn option(nodes: &'a [RdNode], path: RdAstPath) -> Self {
        Self {
            nodes,
            container_path: path,
            kind: ContainerKind::Option,
            start: 0,
        }
    }

    fn path_for(&self, absolute_index: usize) -> RdAstPath {
        match self.kind {
            ContainerKind::Root => RdAstPath::new(vec![RdAstPathSegment::TopLevel(absolute_index)]),
            ContainerKind::Child | ContainerKind::Option => {
                self.container_path.with_child(absolute_index)
            }
        }
    }

    /// Returns the path of the container holding this sequence.
    ///
    /// The empty path identifies the document root. For child and option
    /// sequences this is respectively the owning node path and the path ending
    /// in [`RdAstPathSegment::Option`].
    pub fn container_path(&self) -> &RdAstPath {
        &self.container_path
    }

    /// Alias for [`Self::container_path`].
    pub fn path(&self) -> &RdAstPath {
        self.container_path()
    }

    /// Returns the absolute sibling range represented by this sequence.
    pub fn range(&self) -> RdSiblingRange {
        RdSiblingRange::new(
            self.container_path.clone(),
            self.start..self.start + self.nodes.len(),
        )
    }

    /// Alias for [`Self::range`].
    pub fn sibling_range(&self) -> RdSiblingRange {
        self.range()
    }

    pub fn as_slice(&self) -> &'a [RdNode] {
        self.nodes
    }

    /// Returns the underlying nodes without positional wrappers.
    pub fn nodes(&self) -> &'a [RdNode] {
        self.as_slice()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a positioned node by relative index.
    pub fn get(&self, index: usize) -> Option<RdNodeRef<'a>> {
        let node = self.nodes.get(index)?;
        Some(RdNodeRef::new(node, self.path_for(self.start + index)))
    }

    /// Returns a subslice, retaining absolute sibling indices.
    pub fn slice(&self, range: Range<usize>) -> Option<Self> {
        if range.start > range.end || range.end > self.nodes.len() {
            return None;
        }
        Some(Self {
            nodes: &self.nodes[range.clone()],
            container_path: self.container_path.clone(),
            kind: self.kind,
            start: self.start + range.start,
        })
    }

    pub fn iter(&self) -> RdNodesIter<'a> {
        RdNodesIter {
            nodes: self.nodes,
            container_path: self.container_path.clone(),
            kind: self.kind,
            start: self.start,
            next: 0,
        }
    }
}

/// An iterator over positioned nodes in one sibling sequence.
#[derive(Debug, Clone)]
pub struct RdNodesIter<'a> {
    nodes: &'a [RdNode],
    container_path: RdAstPath,
    kind: ContainerKind,
    start: usize,
    next: usize,
}

impl<'a> Iterator for RdNodesIter<'a> {
    type Item = RdNodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.nodes.get(self.next)?;
        let absolute_index = self.start + self.next;
        self.next += 1;
        let path = match self.kind {
            ContainerKind::Root => RdAstPath::new(vec![RdAstPathSegment::TopLevel(absolute_index)]),
            ContainerKind::Child | ContainerKind::Option => {
                self.container_path.with_child(absolute_index)
            }
        };
        Some(RdNodeRef::new(node, path))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.nodes.len() - self.next;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RdNodesIter<'_> {}

impl<'a> IntoIterator for &RdNodesRef<'a> {
    type Item = RdNodeRef<'a>;
    type IntoIter = RdNodesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for RdNodesRef<'a> {
    type Item = RdNodeRef<'a>;
    type IntoIter = RdNodesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A present bracket-option container and its positioned child sequence.
///
/// An empty `RdOptionRef` is distinct from `None` returned by
/// [`RdNodeRef::option`].
#[derive(Debug, Clone, PartialEq)]
pub struct RdOptionRef<'a> {
    nodes: RdNodesRef<'a>,
    path: RdAstPath,
}

impl<'a> RdOptionRef<'a> {
    pub(crate) fn new(nodes: &'a [RdNode], path: RdAstPath) -> Self {
        Self {
            nodes: RdNodesRef::option(nodes, path.clone()),
            path,
        }
    }

    /// Returns the canonical path of this option container.
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }

    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.nodes.clone()
    }

    pub fn children(&self) -> RdNodesRef<'a> {
        self.nodes_ref()
    }

    pub fn range(&self) -> RdSiblingRange {
        self.nodes.range()
    }

    pub fn as_slice(&self) -> &'a [RdNode] {
        self.nodes.as_slice()
    }

    /// Returns the underlying option nodes without positional wrappers.
    pub fn nodes(&self) -> &'a [RdNode] {
        self.as_slice()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Parses this option's plain text children as a comma-separated scalar
    /// option list.
    ///
    /// The grammar accepts `key=value` pairs separated by commas, with no
    /// quoting, escaping, or nesting. Pair order and soft diagnostics are
    /// preserved; malformed syntax and non-text children return an
    /// [`RdOptionError`]. The returned errors use this option's canonical
    /// `Option` path rather than a caller-supplied coordinate.
    pub fn parse(&self) -> Result<RdOptionList<'a>, RdOptionError> {
        RdOptionList::parse(self.as_slice(), self.path.clone())
    }

    pub fn get(&self, index: usize) -> Option<RdNodeRef<'a>> {
        self.nodes.get(index)
    }

    pub fn iter(&self) -> RdNodesIter<'a> {
        self.nodes.iter()
    }
}

impl<'a> IntoIterator for &RdOptionRef<'a> {
    type Item = RdNodeRef<'a>;
    type IntoIter = RdNodesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for RdOptionRef<'a> {
    type Item = RdNodeRef<'a>;
    type IntoIter = RdNodesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An absolute half-open range of siblings in one AST container.
///
/// This is a snapshot-local AST coordinate, not a source byte range. The
/// type cannot prevent applying it to another document or an edited snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RdSiblingRange {
    container_path: RdAstPath,
    range: Range<usize>,
}

impl RdSiblingRange {
    pub(crate) fn new(container_path: RdAstPath, range: Range<usize>) -> Self {
        Self {
            container_path,
            range,
        }
    }

    pub fn container_path(&self) -> &RdAstPath {
        &self.container_path
    }

    pub fn path(&self) -> &RdAstPath {
        self.container_path()
    }

    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn start(&self) -> usize {
        self.range.start
    }

    pub fn end(&self) -> usize {
        self.range.end
    }

    pub fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }
}

/// The structural preorder iterator returned by [`RdDocument::walk`].
///
/// The iterator keeps only one sibling iterator per active container, so it
/// does not materialize the complete document traversal before yielding the
/// first node.
#[derive(Debug, Clone)]
pub struct RdWalk<'a> {
    stack: Vec<RdNodesIter<'a>>,
}

impl<'a> Iterator for RdWalk<'a> {
    type Item = RdNodeRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let cursor = match self.stack.last_mut()?.next() {
                Some(cursor) => cursor,
                None => {
                    self.stack.pop();
                    continue;
                }
            };

            let children = cursor.children();
            let option = cursor.option();
            // The stack is LIFO: push children first so that option nodes are
            // visited before child nodes for the current parent.
            self.stack.push(children.iter());
            if let Some(option) = option {
                self.stack.push(option.children().iter());
            }
            return Some(cursor);
        }
    }
}

impl RdDocument {
    /// Returns all top-level nodes as positioned cursors.
    pub fn top_level(&self) -> RdNodesRef<'_> {
        RdNodesRef::root(self.nodes())
    }

    /// Resolves a node path in this document snapshot.
    ///
    /// The empty path and paths ending in `Option` identify containers rather
    /// than nodes and therefore return `None`.
    pub fn node_at(&self, path: &RdAstPath) -> Option<RdNodeRef<'_>> {
        let [RdAstPathSegment::TopLevel(index), rest @ ..] = path.segments() else {
            return None;
        };
        let mut node = self.nodes().get(*index)?;
        let mut position = 0;
        while position < rest.len() {
            match &rest[position] {
                RdAstPathSegment::Child(index) => {
                    let children = node_children(node)?;
                    node = children.get(*index)?;
                    position += 1;
                }
                RdAstPathSegment::Option => {
                    let option = node_option(node)?;
                    let Some(RdAstPathSegment::Child(index)) = rest.get(position + 1) else {
                        return None;
                    };
                    node = option.get(*index)?;
                    position += 2;
                }
                RdAstPathSegment::TopLevel(_) => return None,
            }
        }
        Some(RdNodeRef::new(node, path.clone()))
    }

    /// Resolves a present option container in this document snapshot.
    ///
    /// The path must end in `Option`; a path ending in `Option` followed by a
    /// child identifies a node and belongs to [`Self::node_at`] instead.
    pub fn option_at(&self, path: &RdAstPath) -> Option<RdOptionRef<'_>> {
        let segments = path.segments();
        if !matches!(segments.last(), Some(RdAstPathSegment::Option)) {
            return None;
        }
        let owner_path = RdAstPath::new(segments[..segments.len() - 1].to_vec());
        self.node_at(&owner_path)?.option()
    }

    /// Walks every stored [`RdNode`] exactly once in structural preorder.
    ///
    /// A node is followed by descendants in its present option and then by
    /// descendants in its children. Raw payloads and attributes are not AST
    /// nodes, but Raw's stored option and child nodes are walked.
    pub fn walk(&self) -> RdWalk<'_> {
        RdWalk {
            stack: vec![self.top_level().iter()],
        }
    }
}

fn node_children(node: &RdNode) -> Option<&[RdNode]> {
    match node {
        RdNode::Tagged(tagged) => Some(tagged.children()),
        RdNode::Group(group) => Some(group.children()),
        RdNode::Raw(raw) => Some(raw.children()),
        RdNode::Text(_) | RdNode::RCode(_) | RdNode::Verb(_) | RdNode::Comment(_) => None,
    }
}

fn node_option(node: &RdNode) -> Option<&[RdNode]> {
    match node {
        RdNode::Tagged(tagged) => tagged.option(),
        RdNode::Raw(raw) => raw.option(),
        RdNode::Text(_)
        | RdNode::RCode(_)
        | RdNode::Verb(_)
        | RdNode::Comment(_)
        | RdNode::Group(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RawRdValue, RdTag, producer};

    fn fixture() -> RdDocument {
        RdDocument::new(vec![
            RdNode::Text("root".into()),
            RdNode::tagged(
                RdTag::Unknown(r"\future".into()),
                Some(vec![RdNode::group(vec![RdNode::Text("option".into())])]),
                vec![RdNode::group(vec![RdNode::Text("child".into())])],
            ),
            RdNode::Raw(producer::raw_node(
                Some("RAW".into()),
                Some(vec![RdNode::Text("raw option".into())]),
                vec![RdNode::group(vec![RdNode::Text("raw child".into())])],
                Some(RawRdValue::Character(vec![Some("payload".into())])),
                vec![producer::raw_attribute(
                    "hidden".into(),
                    producer::raw_object(RawRdValue::Null, vec![]),
                )],
            )),
        ])
    }

    #[test]
    fn walk_order_and_lookup_cover_all_structural_nodes() {
        let document = fixture();
        let walked: Vec<_> = document.walk().collect();
        let paths: Vec<_> = walked.iter().map(|node| node.path().clone()).collect();
        assert_eq!(
            paths,
            vec![
                vec![RdAstPathSegment::TopLevel(0)].into(),
                vec![RdAstPathSegment::TopLevel(1)].into(),
                vec![
                    RdAstPathSegment::TopLevel(1),
                    RdAstPathSegment::Option,
                    RdAstPathSegment::Child(0)
                ]
                .into(),
                vec![
                    RdAstPathSegment::TopLevel(1),
                    RdAstPathSegment::Option,
                    RdAstPathSegment::Child(0),
                    RdAstPathSegment::Child(0)
                ]
                .into(),
                vec![RdAstPathSegment::TopLevel(1), RdAstPathSegment::Child(0)].into(),
                vec![
                    RdAstPathSegment::TopLevel(1),
                    RdAstPathSegment::Child(0),
                    RdAstPathSegment::Child(0)
                ]
                .into(),
                vec![RdAstPathSegment::TopLevel(2)].into(),
                vec![
                    RdAstPathSegment::TopLevel(2),
                    RdAstPathSegment::Option,
                    RdAstPathSegment::Child(0)
                ]
                .into(),
                vec![RdAstPathSegment::TopLevel(2), RdAstPathSegment::Child(0)].into(),
                vec![
                    RdAstPathSegment::TopLevel(2),
                    RdAstPathSegment::Child(0),
                    RdAstPathSegment::Child(0)
                ]
                .into(),
            ]
        );
        for cursor in walked {
            let resolved = document.node_at(cursor.path()).expect("walk path resolves");
            assert!(std::ptr::eq(cursor.node(), resolved.node()));
        }
        assert_eq!(document.walk().count(), 10);
    }

    #[test]
    fn options_distinguish_absent_and_present_empty() {
        let empty_document = RdDocument::new(vec![]);
        let empty_root = empty_document.top_level();
        assert!(empty_root.is_empty());
        assert_eq!(empty_root.range().range(), 0..0);
        assert_eq!(empty_root.container_path(), &RdAstPath::new(vec![]));

        let document = RdDocument::new(vec![
            RdNode::group(vec![]),
            RdNode::tagged(RdTag::Title, Some(vec![]), vec![]),
            RdNode::tagged(RdTag::Title, None, vec![]),
        ]);
        let empty_group = document.top_level().get(0).unwrap().children();
        assert!(empty_group.is_empty());
        assert_eq!(
            empty_group.container_path(),
            &RdAstPath::new(vec![RdAstPathSegment::TopLevel(0)])
        );
        assert_eq!(empty_group.range().range(), 0..0);

        let empty_tagged = document.top_level().get(1).unwrap().children();
        assert!(empty_tagged.is_empty());
        assert_eq!(
            empty_tagged.container_path(),
            &RdAstPath::new(vec![RdAstPathSegment::TopLevel(1)])
        );
        assert_eq!(empty_tagged.range().range(), 0..0);

        let empty = document.top_level().get(1).unwrap().option().unwrap();
        assert!(empty.is_empty());
        assert_eq!(
            empty.path().segments().last(),
            Some(&RdAstPathSegment::Option)
        );
        assert!(document.top_level().get(2).unwrap().option().is_none());
        assert!(document.node_at(empty.path()).is_none());
        assert!(document.option_at(empty.path()).unwrap().is_empty());
        assert!(
            document
                .option_at(&RdAstPath::new(vec![
                    RdAstPathSegment::TopLevel(0),
                    RdAstPathSegment::Option,
                    RdAstPathSegment::Child(0),
                ]))
                .is_none()
        );
    }

    #[test]
    fn slices_retain_absolute_indices() {
        let document = RdDocument::new(vec![RdNode::group(
            (0..7)
                .map(|index| RdNode::Text(index.to_string()))
                .collect(),
        )]);
        let children = document.top_level().get(0).unwrap().children();
        let slice = children.slice(3..6).unwrap().slice(1..2).unwrap();
        assert_eq!(
            slice.get(0).unwrap().path().segments().last(),
            Some(&RdAstPathSegment::Child(4))
        );
        assert_eq!(slice.range().range(), 4..5);
    }

    #[test]
    fn invalid_paths_and_identical_siblings_are_handled_structurally() {
        let document = RdDocument::new(vec![RdNode::group(vec![
            RdNode::Text("same".into()),
            RdNode::Text("same".into()),
        ])]);
        let children = document.top_level().get(0).unwrap().children();
        assert_ne!(
            children.get(0).unwrap().path(),
            children.get(1).unwrap().path()
        );
        assert!(document.node_at(&RdAstPath::new(vec![])).is_none());
        assert!(
            document
                .node_at(&RdAstPath::new(vec![
                    RdAstPathSegment::TopLevel(0),
                    RdAstPathSegment::Option
                ]))
                .is_none()
        );
        assert!(
            document
                .node_at(&RdAstPath::new(vec![
                    RdAstPathSegment::TopLevel(0),
                    RdAstPathSegment::Child(2)
                ]))
                .is_none()
        );
        assert!(
            document
                .node_at(&RdAstPath::new(vec![
                    RdAstPathSegment::TopLevel(0),
                    RdAstPathSegment::Child(0),
                    RdAstPathSegment::Child(0)
                ]))
                .is_none()
        );
    }
}
