use super::spec::Context;

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
    pub(super) nodes: Vec<rd_ast::RdNode>,
    pub(super) closed: bool,
    pub(super) terminated_by_endif: bool,
    pub(super) rlike_state: Option<super::RLikeState>,
    pub(super) rlike_brace_depth: Option<usize>,
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
    pub(super) out: Vec<rd_ast::RdNode>,
    pub(super) buf: String,
    pub(super) brace_depth: usize,
    pub(super) rlike_state: super::RLikeState,
    pub(super) closed: bool,
    pub(super) terminated_by_endif: bool,
    pub(super) surplus_group_at: Option<usize>,
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
            brace_depth,
            rlike_state,
            closed: false,
            terminated_by_endif: false,
            surplus_group_at: None,
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
