use super::spec::Context;
use crate::source_map::{SourceExtentNode, SourceExtentSequence};
use std::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Latex,
    RLike,
    Verbatim,
    Equation,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemPolicy {
    Unknown,
    Zero,
    Two,
}
#[derive(Clone, Copy)]
pub(super) enum Leaf {
    Text,
    RCode,
    Verb,
}
pub(super) struct Frame {
    pub(super) mode: Mode,
    pub(super) leaf: Leaf,
    pub(super) comments_enabled: bool,
    pub(super) opener: Option<std::ops::Range<usize>>,
    /// True for option frames whose owner sits at document level: section
    /// macros are synchronization points there (CONTRACT §6 rules 7 and 11).
    pub(super) section_sync: bool,
    pub(super) item_policy: ItemPolicy,
}
pub(super) struct FrameResult {
    pub(super) nodes: Vec<LocatedNode>,
    pub(super) closed: bool,
    pub(super) terminated_by_endif: bool,
    /// End of the frame's actual content, before a closing delimiter or a
    /// synchronization directive that was left for the enclosing frame.
    pub(super) content_end: usize,
    /// End of the source consumed by this frame, including a closing
    /// delimiter or conditional terminator when one was consumed.
    pub(super) consumed_end: usize,
    pub(super) rlike_state: Option<super::RLikeState>,
    pub(super) rlike_brace_depth: Option<usize>,
}
pub(super) struct LocatedNode {
    pub(super) node: rd_ast::RdNode,
    pub(super) source: SourceExtentNode,
}

impl LocatedNode {
    pub(super) fn leaf(node: rd_ast::RdNode, extent: Range<usize>) -> Self {
        Self {
            source: SourceExtentNode::leaf(extent.clone()),
            node,
        }
    }

    pub(super) fn tagged(
        tag: rd_ast::RdTag,
        option: Option<(Vec<Self>, Range<usize>)>,
        children: Vec<Self>,
        extent: Range<usize>,
    ) -> Self {
        let (option_nodes, option_source) = option.map_or((None, None), |(nodes, bytes)| {
            let (values, sources) = split_nodes(Some(nodes));
            (
                values,
                Some(SourceExtentSequence {
                    bytes,
                    nodes: sources.unwrap_or_default(),
                }),
            )
        });
        let (child_nodes, child_sources) = split_nodes(Some(children));
        Self {
            node: rd_ast::RdNode::tagged(tag, option_nodes, child_nodes.unwrap_or_default()),
            source: SourceExtentNode::with_children(extent, option_source, child_sources),
        }
    }

    pub(super) fn group(children: Vec<Self>, extent: Range<usize>) -> Self {
        let (child_nodes, child_sources) = split_nodes(Some(children));
        Self {
            node: rd_ast::RdNode::group(child_nodes.unwrap_or_default()),
            source: SourceExtentNode::with_children(extent, None, child_sources),
        }
    }

    pub(super) fn into_parts(self) -> (rd_ast::RdNode, SourceExtentNode) {
        (self.node, self.source)
    }
}

fn split_nodes(
    nodes: Option<Vec<LocatedNode>>,
) -> (Option<Vec<rd_ast::RdNode>>, Option<Vec<SourceExtentNode>>) {
    let Some(nodes) = nodes else {
        return (None, None);
    };
    let mut values = Vec::with_capacity(nodes.len());
    let mut sources = Vec::with_capacity(nodes.len());
    for node in nodes {
        values.push(node.node);
        sources.push(node.source);
    }
    (Some(values), Some(sources))
}
pub(super) struct FrameRequest {
    pub(super) frame: Frame,
    pub(super) argument: bool,
    pub(super) bracket: bool,
    pub(super) context: Context,
    pub(super) stop_at_endif: bool,
    pub(super) initial_rlike_state: Option<(super::RLikeState, usize)>,
}
pub(super) struct FrameState {
    pub(super) out: Vec<LocatedNode>,
    pub(super) buf: String,
    pub(super) buf_range: Option<Range<usize>>,
    pub(super) brace_depth: usize,
    pub(super) rlike_state: super::RLikeState,
    pub(super) closed: bool,
    pub(super) terminated_by_endif: bool,
    pub(super) surplus_group_at: Option<usize>,
    pub(super) content_end: Option<usize>,
    pub(super) consumed_end: Option<usize>,
}
impl FrameState {
    pub(super) fn new(request: &FrameRequest) -> Self {
        let brace_depth = request
            .initial_rlike_state
            .as_ref()
            .map(|(_, depth)| *depth)
            .unwrap_or(0);
        let rlike_state = request
            .initial_rlike_state
            .as_ref()
            .map(|(state, _)| state.clone())
            .unwrap_or_default();
        Self {
            out: Vec::new(),
            buf: String::new(),
            buf_range: None,
            brace_depth,
            rlike_state,
            closed: false,
            terminated_by_endif: false,
            surplus_group_at: None,
            content_end: None,
            consumed_end: None,
        }
    }
}
impl Frame {
    pub(super) fn new(mode: Mode, _argument: bool) -> Self {
        Self {
            leaf: match mode {
                Mode::Verbatim | Mode::Equation => Leaf::Verb,
                Mode::RLike => Leaf::RCode,
                _ => Leaf::Text,
            },
            mode,
            comments_enabled: !matches!(mode, Mode::Equation),
            opener: None,
            section_sync: false,
            item_policy: ItemPolicy::Unknown,
        }
    }
    pub(super) fn with_item_policy(mut self, item_policy: ItemPolicy) -> Self {
        self.item_policy = item_policy;
        self
    }
    pub(super) fn with_opener(mut self, opener: std::ops::Range<usize>) -> Self {
        self.opener = Some(opener);
        self
    }
    pub(super) fn with_section_sync(mut self, section_sync: bool) -> Self {
        self.section_sync = section_sync;
        self
    }
}
