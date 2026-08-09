use super::{
    Parser,
    frame::{Frame, FrameRequest, FrameState, ItemPolicy, Mode},
    spec::{self, Context, tag_spec},
};
use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, Severity},
    lexer::{EscapeKind, TokenKind},
};
use rd_ast::{RdNode, RdTag};

impl<'a> Parser<'a> {
    pub(super) fn dispatch_loop(&mut self, request: &FrameRequest, state: &mut FrameState) {
        while self.index < self.tokens.len() {
            // A fatal error raised anywhere (including by a nested parse) must
            // stop every running frame so the first fatal error is the one
            // reported and no further work is spent on a doomed parse.
            if self.fatal_error.is_some() {
                break;
            }
            let token = &self.tokens[self.index];
            match &token.kind {
                TokenKind::IfDef | TokenKind::IfNDef
                    if request.frame.mode != Mode::Equation
                        && !request.bracket
                        && !state.rlike_state.is_raw_string() =>
                {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    let tag = if token.kind == TokenKind::IfDef {
                        RdTag::IfDef
                    } else {
                        RdTag::IfNDef
                    };
                    let conditional = self.parse_conditional(
                        tag,
                        &request.frame,
                        request.context,
                        &mut state.rlike_state,
                        &mut state.brace_depth,
                    );
                    state.out.push(conditional);
                }
                TokenKind::EndIf
                    if request.bracket
                        && request.frame.mode != Mode::Equation
                        && !state.rlike_state.is_raw_string() =>
                {
                    let value = self.canonical(token).to_string();
                    self.append_content(
                        &mut state.buf,
                        &value,
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                }
                TokenKind::EndIf
                    if request.frame.mode != Mode::Equation
                        && !state.rlike_state.is_raw_string() =>
                {
                    if request.stop_at_endif {
                        self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                        self.discard_directive_line();
                        state.closed = true;
                        state.terminated_by_endif = true;
                        break;
                    }
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    self.warn(
                        DiagnosticCode::UnexpectedEndIf,
                        "unexpected ENDIF '#endif'",
                        token.range.clone(),
                    );
                    self.discard_directive_line();
                }
                TokenKind::IfDef | TokenKind::IfNDef if request.bracket => {
                    let range = token.range.clone();
                    let value = self.canonical(token).to_string();
                    self.warn(
                        DiagnosticCode::UnexpectedConditional,
                        "unexpected conditional, expecting ']'",
                        range,
                    );
                    self.append_content(
                        &mut state.buf,
                        &value,
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                }
                TokenKind::RBracket if request.argument && request.bracket => {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    self.index += 1;
                    state.closed = true;
                    break;
                }
                TokenKind::RBrace if request.argument && request.bracket => {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    break;
                }
                TokenKind::RBrace if !request.argument => {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    self.diagnostics.push(Diagnostic::new(
                        Severity::Error,
                        DiagnosticCode::UnexpectedClosingDelimiter,
                        "unexpected closing delimiter",
                        self.map.span(token.range.clone()),
                    ));
                    self.index += 1;
                }
                TokenKind::RBrace
                    if request.stop_at_endif
                        && request.argument
                        && !request.bracket
                        && state.brace_depth == 0
                        && (request.frame.mode != Mode::RLike
                            || state.rlike_state.is_normal()
                            || state.rlike_state.is_transient_opener()
                            || state.rlike_state.is_comment()) =>
                {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    self.warn(
                        DiagnosticCode::UnexpectedClosingDelimiter,
                        "unexpected closing delimiter",
                        token.range.clone(),
                    );
                    self.index += 1;
                }
                TokenKind::RBrace if request.argument && !request.bracket => {
                    match request.frame.mode {
                        Mode::Latex if state.brace_depth == 0 => {
                            self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                            self.index += 1;
                            state.closed = true;
                            break;
                        }
                        Mode::RLike
                            if (state.rlike_state.is_normal()
                                || state.rlike_state.is_transient_opener()
                                || state.rlike_state.is_comment())
                                && state.brace_depth == 0 =>
                        {
                            state.rlike_state.clear_transient_opener();
                            self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                            self.index += 1;
                            state.closed = true;
                            break;
                        }
                        Mode::Verbatim | Mode::Equation if state.brace_depth == 0 => {
                            self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                            self.index += 1;
                            state.closed = true;
                            break;
                        }
                        _ => {
                            if request.frame.mode == Mode::RLike
                                && state.rlike_state.is_active()
                                && !state.rlike_state.is_transient_opener()
                                && !state.rlike_state.is_comment()
                            {
                                self.append_content(
                                    &mut state.buf,
                                    "}",
                                    request.frame.mode,
                                    &mut state.rlike_state,
                                );
                                self.index += 1;
                                continue;
                            }
                            state.brace_depth -= 1;
                            self.append_content(
                                &mut state.buf,
                                "}",
                                request.frame.mode,
                                &mut state.rlike_state,
                            );
                            self.index += 1;
                        }
                    }
                }
                TokenKind::Newline => {
                    let raw_body = state.rlike_state.is_raw_string();
                    self.append_content(
                        &mut state.buf,
                        "\n",
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                    if !raw_body {
                        self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    }
                }
                TokenKind::Escape(kind) => {
                    let preserve = request.frame.mode == Mode::Equation
                        || (request.frame.mode == Mode::RLike
                            && (state.rlike_state.is_raw_string()
                                || (state.rlike_state.is_ordinary_quote()
                                    && matches!(kind, EscapeKind::LBrace | EscapeKind::RBrace))));
                    let value = if preserve {
                        self.text(token).to_owned()
                    } else {
                        let value = match kind {
                            EscapeKind::Percent => '%',
                            EscapeKind::LBrace => '{',
                            EscapeKind::RBrace => '}',
                            EscapeKind::Backslash => '\\',
                        };
                        value.to_string()
                    };
                    self.append_content(
                        &mut state.buf,
                        &value,
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                }
                TokenKind::ControlSequence => {
                    let name = self.text(token).to_string();
                    if matches!(request.frame.mode, Mode::Verbatim | Mode::Equation)
                        || (request.frame.mode == Mode::RLike
                            && (state.rlike_state.is_comment()
                                || (state.rlike_state.is_active()
                                    && !state.rlike_state.is_ordinary_quote()
                                    && !state.rlike_state.is_transient_opener())))
                    {
                        let spelling = self.text(token).to_string();
                        self.append_content(
                            &mut state.buf,
                            &spelling,
                            request.frame.mode,
                            &mut state.rlike_state,
                        );
                        self.index += 1;
                        continue;
                    }
                    let quoted =
                        request.frame.mode == Mode::RLike && state.rlike_state.is_ordinary_quote();
                    if quoted && !spec::recognized_in_ordinary_quote(&name) {
                        self.append_content(
                            &mut state.buf,
                            &name,
                            request.frame.mode,
                            &mut state.rlike_state,
                        );
                        self.index += 1;
                        continue;
                    }
                    state.rlike_state.clear_transient_opener();
                    if let Some(spec) = tag_spec(&name, request.context) {
                        // Section synchronization (CONTRACT §6 rules 7 and
                        // 11): a section-level macro terminates an unclosed
                        // option owned at document level instead of being
                        // swallowed into it. Options nested in inline content
                        // close at their enclosing close instead.
                        if request.bracket && request.frame.section_sync && spec.section {
                            self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                            break;
                        }
                        self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                        if name == r"\item" && request.frame.item_policy == ItemPolicy::Unknown {
                            self.diagnostics.push(Diagnostic::new(
                                Severity::Error,
                                DiagnosticCode::UnknownTag,
                                r"unknown macro '\item'",
                                self.map.span(token.range.clone()),
                            ));
                            self.index += 1;
                            state.out.push(RdNode::Text(name));
                        } else {
                            state.out.push(self.parse_tag(
                                name,
                                Some(spec),
                                request.context,
                                quoted,
                                request.frame.item_policy,
                            ));
                        }
                        state.surplus_group_at = Some(self.index);
                    } else {
                        self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                        self.diagnostics.push(Diagnostic::new(
                            Severity::Error,
                            DiagnosticCode::UnknownTag,
                            format!("unknown tag {name}"),
                            self.map.span(token.range.clone()),
                        ));
                        // The one-argument fallback and exact spelling are provisional
                        // pending oracle probes (CONTRACT §8).
                        state.out.push(self.parse_tag(
                            name,
                            None,
                            request.context,
                            false,
                            request.frame.item_policy,
                        ));
                        state.surplus_group_at = Some(self.index);
                    }
                }
                // A bare raw prefix is still normal for comment recognition; only a scan/body is active.
                TokenKind::Comment
                    if request.frame.mode == Mode::RLike && state.rlike_state.is_comment() =>
                {
                    // Once R has recognized a hash comment, an Rd percent is
                    // ordinary comment text. Re-lex the token tail so braces
                    // still update depth and may terminate the argument; the
                    // R hash-comment state remains active, so Rd markup stays
                    // opaque.
                    let range = token.range.clone();
                    self.append_content(
                        &mut state.buf,
                        "%",
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    if !self.relex_comment_tail(range) {
                        break;
                    }
                    continue;
                }
                TokenKind::Comment
                    if request.frame.comments_enabled
                        && (state.rlike_state.is_normal()
                            || (state.rlike_state.is_transient_opener()
                                && !state.rlike_state.is_raw_delimiter())
                            || state.rlike_state.is_comment()) =>
                {
                    state.rlike_state.clear_raw_prefix();
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    state
                        .out
                        .push(RdNode::Comment(self.text(token).to_string()));
                    self.index += 1;
                }
                TokenKind::Comment => {
                    if matches!(request.frame.mode, Mode::Equation) || state.rlike_state.is_active()
                    {
                        // The percent is literal content here, so the rest of
                        // the comment token must go back through ordinary
                        // tokenization: it may hold the argument's closing
                        // brace and content belonging to enclosing frames.
                        let range = token.range.clone();
                        self.append_content(
                            &mut state.buf,
                            "%",
                            request.frame.mode,
                            &mut state.rlike_state,
                        );
                        if !self.relex_comment_tail(range) {
                            break;
                        }
                        continue;
                    }
                    state.buf.push_str(self.text(token));
                    self.index += 1;
                }
                TokenKind::LBrace
                    if request.bracket && matches!(request.frame.mode, Mode::Latex) =>
                {
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    break;
                }
                TokenKind::LBrace
                    if matches!(request.frame.mode, Mode::Latex) && request.argument =>
                {
                    self.index += 1;
                    let result = self.parse_frame(FrameRequest {
                        frame: Frame::new(Mode::Latex, true)
                            .with_opener(self.tokens[self.index - 1].range.clone()),
                        argument: true,
                        bracket: false,
                        context: Context::Latex,
                        stop_at_endif: false,
                        initial_rlike_state: None,
                    });
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    state
                        .out
                        .push(RdNode::tagged(RdTag::List, None, result.nodes));
                }
                TokenKind::LBrace if matches!(request.frame.mode, Mode::Latex) => {
                    // A bare group directly after a macro that stopped at its
                    // maximum arity is a silent sibling LIST (CONTRACT §6 rule
                    // 10); any other top-level bare group is forbidden and is
                    // stripped with a diagnostic (rule 5).
                    if state.surplus_group_at == Some(self.index) {
                        self.index += 1;
                        let result = self.parse_frame(FrameRequest {
                            frame: Frame::new(Mode::Latex, true)
                                .with_opener(self.tokens[self.index - 1].range.clone()),
                            argument: true,
                            bracket: false,
                            context: Context::Latex,
                            stop_at_endif: false,
                            initial_rlike_state: None,
                        });
                        self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                        state
                            .out
                            .push(RdNode::tagged(RdTag::List, None, result.nodes));
                        state.surplus_group_at = Some(self.index);
                        continue;
                    }
                    let opener = token.range.clone();
                    self.index += 1;
                    let result = self.parse_frame(FrameRequest {
                        frame: Frame::new(Mode::Latex, true).with_opener(opener.clone()),
                        argument: true,
                        bracket: false,
                        context: request.context,
                        stop_at_endif: false,
                        initial_rlike_state: None,
                    });
                    self.flush(&mut state.out, &mut state.buf, request.frame.leaf);
                    self.diagnostics.push(Diagnostic::new(
                        Severity::Error,
                        DiagnosticCode::UnexpectedOpeningDelimiter,
                        "unexpected opening delimiter",
                        self.map.span(opener),
                    ));
                    state.out.extend(result.nodes);
                }
                TokenKind::LBrace => {
                    if request.frame.mode == Mode::RLike && state.rlike_state.is_comment() {
                        state.brace_depth += 1;
                        self.append_content(
                            &mut state.buf,
                            "{",
                            request.frame.mode,
                            &mut state.rlike_state,
                        );
                        self.index += 1;
                        continue;
                    }
                    if request.frame.mode == Mode::RLike
                        && state.rlike_state.is_active()
                        && (!state.rlike_state.is_transient_opener()
                            || state.rlike_state.is_raw_delimiter())
                    {
                        self.append_content(
                            &mut state.buf,
                            "{",
                            request.frame.mode,
                            &mut state.rlike_state,
                        );
                        self.index += 1;
                        continue;
                    }
                    state.rlike_state.clear_transient_opener();
                    state.brace_depth += 1;
                    self.append_content(
                        &mut state.buf,
                        "{",
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                }
                TokenKind::RBrace
                | TokenKind::LBracket
                | TokenKind::RBracket
                | TokenKind::IfDef
                | TokenKind::IfNDef
                | TokenKind::EndIf
                | TokenKind::Text
                | TokenKind::Whitespace
                | TokenKind::Backslash => {
                    let value = self.canonical(token).to_string();
                    self.append_content(
                        &mut state.buf,
                        &value,
                        request.frame.mode,
                        &mut state.rlike_state,
                    );
                    self.index += 1;
                }
            }
        }
    }
}
