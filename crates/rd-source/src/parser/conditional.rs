use crate::diagnostic::DiagnosticCode;
use rd_ast::{RdNode, RdTag};

use super::{
    Parser,
    frame::{Frame, FrameRequest, LocatedNode, Mode},
    rlike::RLikeState,
    spec::Context,
};

impl<'a> Parser<'a> {
    pub(super) fn parse_conditional(
        &mut self,
        tag: RdTag,
        frame: &Frame,
        context: Context,
        enclosing_rlike_state: &mut RLikeState,
        enclosing_rlike_brace_depth: &mut usize,
    ) -> LocatedNode {
        let directive = self.tokens[self.index].range.clone();
        self.index += 1;
        let (target, target_range) = self.consume_directive_tail(directive.end);
        let body = self.parse_frame(FrameRequest {
            frame: Frame::new(frame.mode, true)
                .with_item_policy(frame.item_policy)
                .with_section_sync(frame.section_sync),
            argument: true,
            bracket: false,
            context,
            stop_at_endif: true,
            initial_rlike_state: (frame.mode == Mode::RLike)
                .then(|| (enclosing_rlike_state.clone(), *enclosing_rlike_brace_depth)),
        });
        if let Some(state) = body.rlike_state {
            *enclosing_rlike_state = state;
        }
        if let Some(depth) = body.rlike_brace_depth {
            *enclosing_rlike_brace_depth = depth;
        }
        if !body.terminated_by_endif {
            self.warn(
                DiagnosticCode::MissingEndIf,
                "unexpected '}' while parsing conditional",
                frame.opener.clone().unwrap_or(directive.clone()),
            );
        }
        let target_group = LocatedNode::group(
            vec![LocatedNode::leaf(
                RdNode::Text(target),
                target_range.clone(),
            )],
            target_range.clone(),
        );
        let body_group = LocatedNode::group(body.nodes, target_range.end..body.content_end);
        LocatedNode::tagged(
            tag,
            None,
            vec![target_group, body_group],
            directive.start..body.consumed_end,
        )
    }

    fn consume_directive_tail(&mut self, start: usize) -> (String, std::ops::Range<usize>) {
        let mut value = String::new();
        let mut end = start;
        while let Some(token) = self.tokens.get(self.index) {
            value.push_str(self.canonical(token));
            end = token.range.end;
            let newline = token.kind == crate::lexer::TokenKind::Newline;
            self.index += 1;
            if newline {
                break;
            }
        }
        if value.is_empty() && start < self.input.len() {
            // This branch is only reachable for a malformed final line; retain
            // the same source-based behavior as ordinary token consumption.
            (
                String::from_utf8_lossy(&self.input[start..]).into_owned(),
                start..self.input.len(),
            )
        } else {
            (value, start..end)
        }
    }

    pub(super) fn discard_directive_line(&mut self) -> usize {
        let mut end = self
            .tokens
            .get(self.index.saturating_sub(1))
            .map_or(0, |token| token.range.end);
        while let Some(token) = self.tokens.get(self.index) {
            let newline = token.kind == crate::lexer::TokenKind::Newline;
            end = token.range.end;
            self.index += 1;
            if newline {
                break;
            }
        }
        end
    }
}
