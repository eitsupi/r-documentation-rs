mod conditional;
mod dispatch;
mod frame;
mod relex;
mod rlike;
mod spec;
mod tag;

use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, ParseError, Parsed, Severity},
    lexer::{self, Token, TokenKind},
    source_map::{SourceExtents, SourceMap},
};
use frame::{Frame, FrameRequest, FrameState, Mode};
use rd_ast::{RdDocument, RdNode};
use rlike::RLikeState;
use spec::Context;
use std::ops::Range;

pub(crate) struct Parser<'a> {
    input: &'a [u8],
    tokens: Vec<Token>,
    index: usize,
    map: SourceMap,
    diagnostics: Vec<Diagnostic>,
    depth: usize,
    fatal_error: Option<ParseError>,
    relex_work: usize,
}
/// v1 implementation limit chosen to keep recursive parsing safe on ordinary
/// thread stacks. Customization is deferred beyond v1 (CONTRACT §14).
pub(crate) const MAX_FRAME_DEPTH: usize = 128;
impl<'a> Parser<'a> {
    pub(crate) fn new(input: &'a [u8], source: &'a str) -> Self {
        Self {
            input,
            tokens: lexer::lex(input),
            index: 0,
            map: SourceMap::new(source),
            diagnostics: Vec::new(),
            depth: 0,
            fatal_error: None,
            relex_work: 0,
        }
    }
    pub(crate) fn parse(self) -> Result<Parsed, ParseError> {
        self.parse_internal(false).map(|(parsed, _)| parsed)
    }

    #[allow(dead_code)]
    pub(crate) fn parse_with_extents(self) -> Result<(Parsed, SourceExtents), ParseError> {
        let (parsed, extents) = self.parse_internal(true)?;
        let extents = extents.expect("extent tracking enabled");
        Ok((parsed, extents))
    }

    fn parse_internal(
        mut self,
        track_extents: bool,
    ) -> Result<(Parsed, Option<SourceExtents>), ParseError> {
        let nodes = self
            .parse_frame(FrameRequest {
                frame: Frame::new(Mode::Latex, false),
                argument: false,
                bracket: false,
                context: Context::Document,
                stop_at_endif: false,
                initial_rlike_state: None,
                track_extents,
            })
            .nodes;
        if let Some(error) = self.fatal_error {
            return Err(error);
        }
        let top_level = nodes.extents;
        let nodes = nodes.nodes;
        Ok((
            Parsed::new(RdDocument::new(nodes), self.diagnostics),
            top_level.map(|top_level| SourceExtents {
                root: 0..self.input.len(),
                top_level,
            }),
        ))
    }
    fn parse_frame(&mut self, request: FrameRequest) -> frame::FrameResult {
        if self.fatal_error.is_some() {
            return frame::FrameResult {
                nodes: frame::NodeBatch::new(request.track_extents),
                closed: false,
                terminated_by_endif: false,
                content_end: self.index_start(),
                consumed_end: self.index_start(),
                rlike_state: None,
                rlike_brace_depth: None,
            };
        }
        if self.depth >= MAX_FRAME_DEPTH {
            let span = request
                .frame
                .opener
                .map(|range| self.map.span(range))
                .unwrap_or_else(|| self.map.span(0..0));
            self.fatal_error = Some(ParseError::NestingLimitExceeded { span });
            return frame::FrameResult {
                nodes: frame::NodeBatch::new(request.track_extents),
                closed: false,
                terminated_by_endif: false,
                content_end: self.index_start(),
                consumed_end: self.index_start(),
                rlike_state: None,
                rlike_brace_depth: None,
            };
        }
        self.depth += 1;
        let mut state = FrameState::new(&request);
        self.dispatch_loop(&request, &mut state);
        let frame = request.frame;
        let argument = request.argument;
        let bracket = request.bracket;
        self.flush(
            &mut state.out,
            &mut state.buf,
            &mut state.buf_range,
            frame.leaf,
        );
        self.depth -= 1;
        if argument
            && !bracket
            && !state.closed
            && let Some(opener) = frame.opener
        {
            self.diagnostics.push(Diagnostic::new(
                Severity::Error,
                DiagnosticCode::UnclosedGroup,
                "unclosed group",
                self.map.span(opener),
            ));
        }
        frame::FrameResult {
            nodes: state.out,
            closed: state.closed,
            terminated_by_endif: state.terminated_by_endif,
            content_end: state.content_end.unwrap_or_else(|| self.index_start()),
            consumed_end: state.consumed_end.unwrap_or_else(|| self.index_start()),
            rlike_state: (frame.mode == Mode::RLike).then_some(state.rlike_state),
            rlike_brace_depth: (frame.mode == Mode::RLike).then_some(state.brace_depth),
        }
    }

    fn warn(&mut self, code: DiagnosticCode, message: &str, range: std::ops::Range<usize>) {
        self.diagnostics.push(Diagnostic::new(
            Severity::Warning,
            code,
            message,
            self.map.span(range),
        ));
    }
    fn append_content(
        &self,
        buf: &mut String,
        buf_range: &mut Option<Range<usize>>,
        value: &str,
        range: Range<usize>,
        mode: Mode,
        state: &mut RLikeState,
    ) {
        state.append_to(buf, value, mode);
        if !value.is_empty() {
            *buf_range = Some(match buf_range.take() {
                Some(current) => current.start.min(range.start)..current.end.max(range.end),
                None => range,
            });
        }
    }
    fn flush(
        &self,
        out: &mut frame::NodeBatch,
        buf: &mut String,
        buf_range: &mut Option<Range<usize>>,
        leaf: frame::Leaf,
    ) {
        if !buf.is_empty() {
            let value = std::mem::take(buf);
            let range = buf_range.take().unwrap_or(0..0);
            let node = match leaf {
                frame::Leaf::Text => RdNode::Text(value),
                frame::Leaf::RCode => RdNode::RCode(value),
                frame::Leaf::Verb => RdNode::Verb(value),
            };
            out.push(frame::LocatedNode::leaf(node, range, out.extents.is_some()));
        } else {
            *buf_range = None;
        }
    }
    fn text(&self, token: &Token) -> &str {
        std::str::from_utf8(&self.input[token.range.clone()]).unwrap()
    }
    fn canonical(&self, token: &Token) -> &str {
        if token.kind == TokenKind::Newline {
            "\n"
        } else {
            self.text(token)
        }
    }

    pub(super) fn index_start(&self) -> usize {
        self.tokens
            .get(self.index)
            .map_or(self.input.len(), |token| token.range.start)
    }
}

#[cfg(test)]
mod tests;
