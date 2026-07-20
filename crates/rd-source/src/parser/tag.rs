use crate::{
    diagnostic::{Diagnostic, DiagnosticCode, ParseError, Severity},
    lexer::TokenKind,
};
use rd_ast::{RdNode, RdTag};

use super::{
    Parser,
    frame::{Frame, FrameRequest, ItemPolicy, Mode},
    spec::{self, Context},
};

impl<'a> Parser<'a> {
    pub(super) fn parse_tag(
        &mut self,
        name: String,
        spec: Option<spec::TagSpec>,
        context: Context,
        quoted: bool,
        item_policy: ItemPolicy,
    ) -> RdNode {
        let unknown = spec.is_none();
        let spec = spec.unwrap_or(spec::TagSpec {
            option_policy: spec::OptionPolicy::Optional { mode: Mode::Latex },
            arguments: spec::unknown_arguments(),
            allowed_contexts: &[],
            section: false,
        });
        let tag = RdTag::from_rd_tag(&name);
        self.index += 1;
        if !unknown && !quoted && !spec.allowed_contexts.contains(&context) {
            self.diagnostics.push(Diagnostic::new(
                Severity::Error,
                DiagnosticCode::TagNotAllowedHere,
                format!("tag {name} is not allowed here"),
                self.map.span(self.tokens[self.index - 1].range.clone()),
            ));
        }
        let arguments = if name == r"\item" && item_policy == ItemPolicy::Two {
            spec::item_arguments()
        } else {
            spec.arguments
        };
        let preserve_single_argument_group = name == r"\figure" && context == Context::RLike;
        let option = match spec.option_policy {
            spec::OptionPolicy::Optional { mode }
                if self
                    .tokens
                    .get(self.index)
                    .is_some_and(|token| token.kind == TokenKind::LBracket) =>
            {
                self.index += 1;
                let opener = self.tokens[self.index - 1].range.clone();
                let result = self.parse_frame(FrameRequest {
                    frame: Frame::new(mode, true)
                        .with_opener(opener.clone())
                        .with_section_sync(context == Context::Document),
                    argument: true,
                    bracket: true,
                    context: Context::Latex,
                    stop_at_endif: false,
                    initial_rlike_state: None,
                });
                if !result.closed {
                    self.diagnostics.push(Diagnostic::new(
                        Severity::Error,
                        DiagnosticCode::UnclosedOption,
                        "unclosed option",
                        self.map.span(opener),
                    ));
                }
                Some(result.nodes)
            }
            _ => None,
        };
        if arguments.is_empty() {
            return RdNode::tagged(tag, option, Vec::new());
        }
        if self
            .tokens
            .get(self.index)
            .is_none_or(|t| t.kind != TokenKind::LBrace)
            && arguments[0].required
        {
            self.diagnostics.push(Diagnostic::new(
                Severity::Error,
                DiagnosticCode::MissingArgument,
                format!("missing argument for {name}"),
                self.map.span(self.tokens[self.index - 1].range.clone()),
            ));
            return RdNode::tagged(tag, option, Vec::new());
        }
        let mut children = Vec::new();
        for argument in arguments {
            if self
                .tokens
                .get(self.index)
                .is_none_or(|t| t.kind != TokenKind::LBrace)
            {
                if !argument.required {
                    break;
                }
                self.diagnostics.push(Diagnostic::new(
                    Severity::Error,
                    DiagnosticCode::MissingArgument,
                    format!("missing argument for {name}"),
                    self.map
                        .span(self.tokens[self.index.saturating_sub(1)].range.clone()),
                ));
                if spec.arguments.len() == 1 {
                    return RdNode::tagged(tag, None, Vec::new());
                }
                continue;
            }
            let open = self.index;
            self.index += 1;
            let child_item_policy = if argument.mode == Mode::Latex {
                match name.as_str() {
                    r"\arguments" | r"\value" | r"\describe" => ItemPolicy::Two,
                    r"\itemize" | r"\enumerate" => ItemPolicy::Zero,
                    _ => ItemPolicy::Unknown,
                }
            } else {
                ItemPolicy::Unknown
            };
            let argument_children = self.parse_frame(FrameRequest {
                frame: Frame::new(argument.mode, true)
                    .with_opener(self.tokens[open].range.clone())
                    .with_item_policy(child_item_policy),
                argument: true,
                bracket: false,
                context: if argument.mode == Mode::RLike {
                    Context::RLike
                } else {
                    Context::Latex
                },
                stop_at_endif: false,
                initial_rlike_state: None,
            });
            if name == r"\encoding" && self.fatal_error.is_none() {
                let start = self.tokens[open].range.end;
                let finish = (if argument_children.closed {
                    self.tokens[self.index - 1].range.start
                } else {
                    self.input.len()
                })
                .max(start);
                let all_text = argument_children
                    .nodes
                    .iter()
                    .all(|node| matches!(node, RdNode::Text(_)));
                let value = argument_children
                    .nodes
                    .iter()
                    .filter_map(|node| match node {
                        RdNode::Text(value) => Some(value.as_str()),
                        _ => None,
                    })
                    .collect::<String>();
                let declared = value.trim();
                if !all_text
                    || (!declared.eq_ignore_ascii_case("utf-8")
                        && !declared.eq_ignore_ascii_case("utf8"))
                {
                    let declared = std::str::from_utf8(&self.input[start..finish])
                        .unwrap()
                        .trim()
                        .to_owned();
                    self.fatal_error = Some(ParseError::UnsupportedEncoding {
                        name: declared,
                        span: Some(self.map.span(start..finish)),
                    });
                }
            }
            if arguments.len() == 1 && !preserve_single_argument_group {
                children.extend(argument_children.nodes);
            } else {
                children.push(RdNode::group(argument_children.nodes));
            }
        }
        RdNode::tagged(tag, option, children)
    }
}
