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
    source_map::SourceMap,
};
use frame::{Frame, FrameRequest, FrameState, Mode};
use rd_ast::{RdDocument, RdNode};
use rlike::RLikeState;
use spec::Context;

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
    pub(crate) fn parse(mut self) -> Result<Parsed, ParseError> {
        let nodes = self
            .parse_frame(FrameRequest {
                frame: Frame::new(Mode::Latex, false),
                argument: false,
                bracket: false,
                context: Context::Document,
                stop_at_endif: false,
                initial_rlike_state: None,
            })
            .nodes;
        if let Some(error) = self.fatal_error {
            return Err(error);
        }
        Ok(Parsed::new(RdDocument::new(nodes), self.diagnostics))
    }
    fn parse_frame(&mut self, request: FrameRequest) -> frame::FrameResult {
        if self.fatal_error.is_some() {
            return frame::FrameResult {
                nodes: Vec::new(),
                closed: false,
                terminated_by_endif: false,
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
                nodes: Vec::new(),
                closed: false,
                terminated_by_endif: false,
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
        self.flush(&mut state.out, &mut state.buf, frame.leaf);
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
    fn append_content(&self, buf: &mut String, value: &str, mode: Mode, state: &mut RLikeState) {
        state.append_to(buf, value, mode);
    }
    fn flush(&self, out: &mut Vec<RdNode>, buf: &mut String, leaf: frame::Leaf) {
        if !buf.is_empty() {
            let value = std::mem::take(buf);
            out.push(match leaf {
                frame::Leaf::Text => RdNode::Text(value),
                frame::Leaf::RCode => RdNode::RCode(value),
                frame::Leaf::Verb => RdNode::Verb(value),
            });
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
}

#[cfg(test)]
mod tests;
