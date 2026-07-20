//! Lowering from `rd-rds`'s decoded [`RObject`] trees (installed-package
//! help databases, `tools::parse_Rd()` output saved as RDS) into the
//! canonical [`RdDocument`] AST.
//!
//! The normative RDS producer profile is in the crate's included
//! `CONTRACT.md`.
//!
//! ## Attribute policy: what lowers structurally vs. falls back to Raw
//!
//! The losslessness invariant (see the crate-level documentation) is
//! scoped to *Rd semantic content*. Producer metadata that has no v1 AST
//! representation is deliberately discarded when structured lowering
//! succeeds; [`RdNode::Raw`] preserves the complete original object
//! whenever fallback is required. Concretely:
//!
//! - **Validated attribute policy**: a single, valid `Rd_tag` and `Rd_option` are consumed
//!   structurally, while `srcref` is recognized as discardable producer
//!   metadata. `srcref` is recognized by name only. Other validated metadata
//!   values must themselves be attribute-free plain scalars, matching the
//!   observed producer patterns. `dynamicFlag` is recognized as discardable
//!   only when the node value is a list and its value is a non-`NA` integer
//!   vector of exactly one element; its numeric range is not constrained. It
//!   is a derived processing cache, not Rd content, so neither attribute is
//!   represented in the AST. A node lowers
//!   to its normal structured form (a leaf or [`RdNode::Tagged`]) only when
//!   every attribute is consumed structurally or recognized as validated
//!   discardable producer metadata. The `srcref` value (including the nested
//!   `srcfile` attribute, whether an inline environment or a `PERSISTSXP`
//!   reference) is never inspected or validated.
//!   `class = "Rd"` is likewise discardable only for a `LIST` node whose
//!   value is a list; the class value must be exactly the non-`NA` character
//!   scalar `"Rd"`. This is a displaced prepared-fragment marker, not Rd
//!   content. `class` on any other tag still forces the lossless `Raw` path.
//!   For expanded `\ifelse` and `\href` nodes, `class = "Rd"` and `Rdfile`
//!   form a validated pair: both are discarded only when the class scalar is
//!   exactly `"Rd"` and `Rdfile` is a non-`NA`, valid UTF-8 character scalar.
//!   They are displaced prepared-fragment provenance markers, not Rd
//!   content. A lone occurrence, an unexpected value, a nested attribute on
//!   either metadata scalar, or the pair on another tag still forces `Raw`.
//! - **Raw stays fully lossless**: a node carrying any unexpected attribute
//!   falls back to [`RdNode::Raw`], which preserves all of its attributes --
//!   *including* `srcref`. `srcref` is only discarded on the structured path;
//!   it is never stripped from a `Raw` node.
//! - **Root attributes** (on the document object itself, e.g. `class =
//!   "Rd"`, `srcref`, `parse_Rd()`'s `macros` macro-definition table, or
//!   a help database's `Rdfile`/`meta`/`prepared` provenance) are all
//!   discarded: [`RdDocument`] has no attribute storage in v1, and these
//!   are structural markers or parser-state/provenance rather than Rd
//!   content. This applies at the root ONLY -- a non-root node carrying
//!   e.g. `class` or `macros` still falls back to `Raw` per the per-node
//!   rule above.
//!
//! `Rd_tag` is rejected when malformed or duplicated; this preserves the
//! original attribute rather than silently treating the node as untagged. A
//! duplicated `Rd_tag` contributes nothing to the node's tag or child
//! interpretation. A malformed but decodable value takes the Raw path, while
//! a value whose bytes cannot be decoded remains a hard lowering error.
//!
//! Untagged list-valued nodes with no rejected attributes and no `Rd_option`
//! are positional groups and lower to [`RdNode::Group`]. Untagged non-list
//! values, nodes carrying `Rd_option`, and nodes with unexpected attributes
//! remain opaque [`RdNode::Raw`] values.

use std::fmt;

use rd_rds::{Attribute, Attributes, EnvHandle, RObject, RStr, RValue};

use crate::{
    RawRdEnvironment, RawRdObject, RawRdReal, RawRdValue, RdAttribute, RdDocument, RdNode, RdTag,
    producer,
};
use crate::{RdPath, RdPathSegment};

/// Lowers a decoded help-database Rd object into the canonical AST.
///
/// The root object's own attributes are all discarded (see the module
/// documentation): `class = "Rd"` is the structural marker identifying a
/// parsed Rd object, `srcref` and `macros` are parser metadata, and help
/// databases add provenance such as `Rdfile`/`meta`/`prepared` -- none of
/// which [`RdDocument`] represents in v1. Only the root *value* is
/// validated: it must be a list, or [`LowerError::RootMustBeList`] is
/// returned.
pub fn lower_r_object(root: &RObject) -> Result<RdDocument, LowerError> {
    let mut context = LowerContext::new();
    let RValue::List(items) = root.value() else {
        return Err(LowerError::RootMustBeList {
            location: context.location(None, None),
        });
    };

    let mut nodes = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        nodes.push(context.scoped(RdPathSegment::TopLevel(index), |context| {
            lower_node(context, item, None)
        })?);
    }

    Ok(RdDocument::new(nodes))
}

fn lower_node(
    context: &mut LowerContext,
    object: &RObject,
    inherited_attribute: Option<&str>,
) -> Result<RdNode, LowerError> {
    let tag = rd_tag_string(context, object)?;
    let option = lower_option(context, object, tag.as_deref())?;
    let node_context = NodeContext {
        tag: tag.as_deref(),
        value: object.value(),
        attributes: object.attributes(),
    };
    // Keep the original object intact until the structured-vs-Raw decision:
    // Raw must preserve every attribute, including otherwise-discardable
    // producer metadata.
    let has_rejected_attributes = object.attributes().iter().any(|attribute| {
        matches!(
            classify_attribute(&node_context, attribute),
            AttributeDisposition::Reject
        )
    });
    let is_leaf_tag = matches!(tag.as_deref(), Some("TEXT" | "RCODE" | "VERB" | "COMMENT"));

    if let Some(tag_text) = tag.as_deref() {
        if !has_rejected_attributes
            && option.is_none()
            && let Some(leaf) = lower_leaf(
                context,
                Some(tag_text),
                inherited_attribute,
                tag_text,
                object.value(),
            )?
        {
            return Ok(leaf);
        }

        if !is_leaf_tag
            && !has_rejected_attributes
            && let RValue::List(children) = object.value()
        {
            return Ok(RdNode::tagged(
                RdTag::from_rd_tag(tag_text),
                option,
                lower_children(context, inherited_attribute, children)?,
            ));
        }
    } else if !has_rejected_attributes
        && option.is_none()
        && let RValue::List(children) = object.value()
    {
        return Ok(RdNode::group(lower_children(
            context,
            inherited_attribute,
            children,
        )?));
    }

    let (raw_children, payload) =
        lower_raw_children(context, tag.as_deref(), inherited_attribute, object.value())?;
    let attributes = lower_attributes(context, object, tag.as_deref())?;
    Ok(RdNode::Raw(producer::raw_node(
        tag,
        option,
        raw_children,
        payload,
        attributes,
    )))
}

fn lower_children(
    context: &mut LowerContext,
    inherited_attribute: Option<&str>,
    children: &[RObject],
) -> Result<Vec<RdNode>, LowerError> {
    let mut lowered = Vec::with_capacity(children.len());
    for (index, child) in children.iter().enumerate() {
        lowered.push(context.scoped(RdPathSegment::Child(index), |context| {
            lower_node(context, child, inherited_attribute)
        })?);
    }
    Ok(lowered)
}

fn lower_option(
    context: &mut LowerContext,
    object: &RObject,
    tag: Option<&str>,
) -> Result<Option<Vec<RdNode>>, LowerError> {
    let Some(option) = object.attributes().get("Rd_option") else {
        return Ok(None);
    };

    Ok(Some(context.scoped(RdPathSegment::Option, |context| {
        match option.value() {
            RValue::List(children) => {
                let option_tag = rd_tag_string(context, option)?;
                let option_context = NodeContext {
                    tag: option_tag.as_deref(),
                    value: option.value(),
                    attributes: option.attributes(),
                };
                let has_rejected_attributes = option.attributes().iter().any(|attribute| {
                    matches!(
                        classify_attribute(&option_context, attribute),
                        AttributeDisposition::Reject
                    )
                });
                if option_tag.is_some()
                    || option.attributes().get("Rd_option").is_some()
                    || has_rejected_attributes
                {
                    lower_node(context, option, Some("Rd_option")).map(|node| vec![node])
                } else {
                    lower_children(context, Some("Rd_option"), children)
                }
            }
            RValue::Character(_) => {
                lower_raw_children(context, tag, Some("Rd_option"), option.value())
                    .map(|(children, _)| children)
            }
            _ => lower_node(context, option, Some("Rd_option")).map(|node| vec![node]),
        }
    })?))
}

mod attributes;
mod context;
mod error;
mod raw;
mod structured;

use attributes::AttributeDisposition;
use attributes::{classify_attribute, lower_attribute, lower_attributes, valid_rd_tag_value};
use context::{LowerContext, NodeContext};
use raw::lower_raw_children;
use structured::{lower_leaf, rd_tag_string};

pub use error::{LowerError, LowerLocation, StringEncoding};

#[cfg(test)]
#[allow(dead_code, unused_imports)]
mod tests;
