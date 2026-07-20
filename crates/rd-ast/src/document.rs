//! [`RdDocument`] and [`RdNode`]: the core Rd document tree.
//!
//! The normative node, ordering, group, and option rules are in the crate's
//! included `CONTRACT.md`.

use crate::{RawRdNode, RdTag};

/// A complete parsed Rd document.
///
/// `nodes` are the top-level children of the parsed Rd object: a mix of
/// section-tagged nodes (`\name`, `\title`, `\description`, ...) and the
/// whitespace `TEXT` leaves R's parser leaves between them. Nothing is
/// filtered out at this layer -- see the crate-level documentation for the
/// losslessness rationale.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdDocument {
    /// The document's top-level nodes, in source order.
    nodes: Vec<RdNode>,
}

impl RdDocument {
    pub fn new(nodes: Vec<RdNode>) -> Self {
        Self { nodes }
    }

    pub fn nodes(&self) -> &[RdNode] {
        &self.nodes
    }

    pub fn into_nodes(self) -> Vec<RdNode> {
        self.nodes
    }
}

impl From<Vec<RdNode>> for RdDocument {
    fn from(nodes: Vec<RdNode>) -> Self {
        Self::new(nodes)
    }
}

impl IntoIterator for RdDocument {
    type Item = RdNode;
    type IntoIter = std::vec::IntoIter<RdNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.into_iter()
    }
}

impl<'a> IntoIterator for &'a RdDocument {
    type Item = &'a RdNode;
    type IntoIter = std::slice::Iter<'a, RdNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.nodes.iter()
    }
}

/// A tagged node with optional bracket content and positional children.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdTagged {
    tag: RdTag,
    option: Option<Vec<RdNode>>,
    children: Vec<RdNode>,
}

impl RdTagged {
    pub fn new(tag: RdTag, option: Option<Vec<RdNode>>, children: Vec<RdNode>) -> Self {
        Self {
            tag,
            option,
            children,
        }
    }

    pub fn tag(&self) -> &RdTag {
        &self.tag
    }

    pub fn option(&self) -> Option<&[RdNode]> {
        self.option.as_deref()
    }

    pub fn children(&self) -> &[RdNode] {
        &self.children
    }

    pub fn into_parts(self) -> (RdTag, Option<Vec<RdNode>>, Vec<RdNode>) {
        (self.tag, self.option, self.children)
    }
}

/// An untagged positional-argument group, distinct from the `LIST` pseudo-tag.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RdGroup {
    children: Vec<RdNode>,
}

impl RdGroup {
    pub fn new(children: Vec<RdNode>) -> Self {
        Self { children }
    }

    pub fn children(&self) -> &[RdNode] {
        &self.children
    }

    pub fn into_children(self) -> Vec<RdNode> {
        self.children
    }
}

impl From<Vec<RdNode>> for RdGroup {
    fn from(children: Vec<RdNode>) -> Self {
        Self::new(children)
    }
}

/// A single node of an Rd document tree.
///
/// The design is mostly generic: four leaf kinds carry raw text, one
/// variant ([`Tagged`](RdNode::Tagged)) represents *every* known markup
/// macro uniformly via [`RdTag`], and [`Raw`](RdNode::Raw) is the lossless
/// fallback for anything that doesn't fit. See the crate-level
/// documentation for why semantics (link targets, argument pairing, table
/// rows) are intentionally left out of this enum.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RdNode {
    /// A `TEXT`-tagged leaf: plain documentation text.
    Text(String),
    /// An `RCODE`-tagged leaf: R source code, as found inside `\usage`,
    /// `\examples`, and similar code-valued sections/macros.
    RCode(String),
    /// A `VERB`-tagged leaf: verbatim text, as found inside `\verb{...}`
    /// and `\preformatted{...}`.
    Verb(String),
    /// A `COMMENT`-tagged leaf: a `%`-introduced Rd comment, preserved
    /// (including the leading `%`) rather than dropped, since
    /// `parse_Rd(keep.source = TRUE)` keeps them and a lossless AST must
    /// be able to represent whatever the producer handed it.
    Comment(String),
    /// Any node carrying a known or unknown macro-style `Rd_tag`.
    Tagged(RdTagged),
    /// An untagged positional-argument group.
    Group(RdGroup),
    /// A lossless fallback for a node that doesn't fit the shapes above:
    /// genuinely non-canonical structures such as nodes with unexpected
    /// attributes. Positional-argument groups use [`RdNode::Group`].
    Raw(RawRdNode),
}

impl RdNode {
    pub fn tagged(tag: RdTag, option: Option<Vec<RdNode>>, children: Vec<RdNode>) -> Self {
        Self::Tagged(RdTagged::new(tag, option, children))
    }

    pub fn group(children: Vec<RdNode>) -> Self {
        Self::Group(RdGroup::new(children))
    }

    pub fn as_tagged(&self) -> Option<&RdTagged> {
        match self {
            Self::Tagged(node) => Some(node),
            _ => None,
        }
    }
    pub fn as_group(&self) -> Option<&RdGroup> {
        match self {
            Self::Group(node) => Some(node),
            _ => None,
        }
    }
    pub fn as_raw(&self) -> Option<&RawRdNode> {
        match self {
            Self::Raw(node) => Some(node),
            _ => None,
        }
    }

    pub fn into_tagged(self) -> Result<RdTagged, Self> {
        match self {
            Self::Tagged(node) => Ok(node),
            other => Err(other),
        }
    }
    pub fn into_group(self) -> Result<RdGroup, Self> {
        match self {
            Self::Group(node) => Ok(node),
            other => Err(other),
        }
    }
    pub fn into_raw(self) -> Result<RawRdNode, Self> {
        match self {
            Self::Raw(node) => Ok(node),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RawRdValue, producer};

    /// A minimal but representative document: `\name`, `\title`, and a
    /// `\description` containing inline markup, separated by the
    /// whitespace `TEXT` leaves a real parse produces.
    fn sample_document() -> RdDocument {
        RdDocument::new(vec![
            RdNode::tagged(RdTag::Name, None, vec![RdNode::Text("example".to_string())]),
            RdNode::Text("\n".to_string()),
            RdNode::tagged(
                RdTag::Title,
                None,
                vec![RdNode::Text("An example topic".to_string())],
            ),
            RdNode::Text("\n".to_string()),
            RdNode::tagged(
                RdTag::Description,
                None,
                vec![
                    RdNode::Text("See ".to_string()),
                    RdNode::tagged(
                        RdTag::Link,
                        Some(vec![RdNode::Text("base".to_string())]),
                        vec![RdNode::Text("print".to_string())],
                    ),
                    RdNode::Text(" for details.".to_string()),
                ],
            ),
        ])
    }

    #[test]
    fn builds_and_matches_expected_shape() {
        let doc = sample_document();
        assert_eq!(doc.nodes().len(), 5);
        let Some(tagged) = doc.nodes()[0].as_tagged() else {
            panic!("expected a Tagged node");
        };
        assert_eq!(tagged.tag(), &RdTag::Name);
        assert_eq!(tagged.children(), &[RdNode::Text("example".to_string())]);
    }

    #[test]
    fn tagged_option_carries_markup() {
        let doc = sample_document();
        let Some(description) = doc.nodes()[4].as_tagged() else {
            panic!("expected the description's Tagged node");
        };
        let Some(link) = description.children()[1].as_tagged() else {
            panic!(r"expected the \link node");
        };
        assert_eq!(link.tag(), &RdTag::Link);
        assert_eq!(link.option(), Some(&[RdNode::Text("base".to_string())][..]));
    }

    /// [`RdNode::Raw`] must preserve an untagged list with an unexpected
    /// attribute, without losing any of it.
    #[test]
    fn raw_node_preserves_untagged_group_and_attributes() {
        let raw = producer::raw_node(
            None,
            None,
            vec![RdNode::Text("print".to_string())],
            None,
            vec![producer::raw_attribute(
                "class".to_string(),
                producer::raw_object(
                    RawRdValue::Character(vec![Some("weird".to_string())]),
                    Vec::new(),
                ),
            )],
        );
        let node = RdNode::Raw(raw.clone());
        let RdNode::Raw(inner) = &node else {
            panic!("expected Raw");
        };
        assert_eq!(inner, &raw);
        assert_eq!(inner.tag(), None);
        assert_eq!(inner.attributes().len(), 1);
    }

    #[test]
    fn equal_documents_compare_equal() {
        assert_eq!(sample_document(), sample_document());
    }
}
