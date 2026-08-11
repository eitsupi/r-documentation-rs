use rd_ast::{RdDocument, RdNode, RdPath, RdPathSegment};

use crate::{
    error::{UnserializableKind, WriteError},
    escape,
    options::{LineEnding, WriterOptions},
    spec::{self, Context as ParserContext, Mode},
};

pub struct Writer {
    options: WriterOptions,
}

impl Writer {
    pub fn new(options: WriterOptions) -> Self {
        Self { options }
    }

    pub fn write_document(&self, document: &RdDocument) -> Result<String, WriteError> {
        let mut output = String::new();
        let mut context = Context::new(self.options);
        context.sequence(
            document.nodes(),
            Mode::Latex,
            &mut output,
            RdPath::new(Vec::new()),
            true,
            None,
            ParserContext::Document,
            &mut escape::RLikeState::default(),
        )?;
        let parsed =
            rd_source::parse(output.as_bytes()).map_err(|error| WriteError::Verification {
                reason: error.to_string(),
            })?;
        if !parsed.diagnostics().is_empty() || parsed.document() != document {
            return Err(WriteError::Verification {
                reason: if parsed.diagnostics().is_empty() {
                    "parsed document differs from input".into()
                } else {
                    format!(
                        "parser produced {} diagnostic(s)",
                        parsed.diagnostics().len()
                    )
                },
            });
        }
        Ok(output)
    }

    pub fn write_document_to<W: std::io::Write>(
        &self,
        document: &RdDocument,
        sink: &mut W,
    ) -> Result<(), WriteError> {
        let output = self.write_document(document)?;
        sink.write_all(output.as_bytes())
            .map_err(|source| WriteError::Io { source })
    }
}

struct Context {
    options: WriterOptions,
}

impl Context {
    fn new(options: WriterOptions) -> Self {
        Self { options }
    }

    fn fail<T>(&self, path: RdPath, kind: UnserializableKind) -> Result<T, WriteError> {
        Err(WriteError::Unserializable { path, kind })
    }

    #[allow(clippy::too_many_arguments)]
    fn sequence(
        &mut self,
        nodes: &[RdNode],
        mode: Mode,
        out: &mut String,
        path: RdPath,
        document: bool,
        item_context: Option<&str>,
        parser_context: ParserContext,
        rlike: &mut escape::RLikeState,
    ) -> Result<(), WriteError> {
        let mut previous_leaf: Option<(&'static str, bool)> = None;
        for (index, node) in nodes.iter().enumerate() {
            let node_path = if document {
                RdPath::new(vec![RdPathSegment::TopLevel(index)])
            } else {
                path.with_child(index)
            };
            if let RdNode::Comment(comment) = node {
                if mode == Mode::Equation {
                    return self.fail(node_path, UnserializableKind::InvalidComment);
                }
                if !comment.starts_with('%') || comment.contains(['\r', '\n']) {
                    return self.fail(node_path, UnserializableKind::InvalidComment);
                }
                if index + 1 < nodes.len() {
                    if !is_newline_leaf(&nodes[index + 1], mode) {
                        return self.fail(node_path, UnserializableKind::InvalidComment);
                    }
                } else if !document {
                    return self.fail(node_path, UnserializableKind::InvalidComment);
                }
                out.push_str(comment);
                previous_leaf = None;
                continue;
            }
            let leaf_info = match node {
                RdNode::Text(_) => Some("text"),
                RdNode::RCode(_) => Some("rcode"),
                RdNode::Verb(_) => Some("verb"),
                _ => None,
            };
            if let Some(kind) = leaf_info {
                if previous_leaf.is_some_and(|(old, ended)| old == kind && !ended) {
                    return self.fail(node_path, UnserializableKind::UnrepresentableLeafBoundary);
                }
                let ended = self.leaf(node, mode, out, node_path.clone(), rlike)?;
                previous_leaf = Some((kind, ended));
            } else {
                self.node(
                    node,
                    mode,
                    parser_context,
                    out,
                    node_path,
                    item_context,
                    rlike,
                )?;
                previous_leaf = None;
            }
        }
        Ok(())
    }

    fn leaf(
        &mut self,
        node: &RdNode,
        mode: Mode,
        out: &mut String,
        path: RdPath,
        rlike: &mut escape::RLikeState,
    ) -> Result<bool, WriteError> {
        let (value, actual) = match node {
            RdNode::Text(s) => (s, Mode::Latex),
            RdNode::RCode(s) => (s, Mode::RLike),
            RdNode::Verb(s) => (s, Mode::Verbatim),
            _ => unreachable!(),
        };
        if actual != leaf_mode(mode) {
            return self.fail(path, UnserializableKind::UnexpectedNodeKind);
        }
        if value.is_empty() || value.contains('\r') {
            return self.fail(path, UnserializableKind::UnrepresentableLeaf);
        }
        if mode == Mode::Equation && !balanced_equation(value) {
            return self.fail(path, UnserializableKind::UnrepresentableEquation);
        }
        let (escaped, raw_newline, nonraw_interior_newline) = escape::escape(value, mode, rlike);
        if nonraw_interior_newline
            || (value.contains('\n') && !value.ends_with('\n') && !raw_newline)
        {
            return self.fail(path, UnserializableKind::UnrepresentableLeaf);
        }
        self.append(out, &escaped);
        Ok(value.ends_with('\n'))
    }

    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        node: &RdNode,
        parent_mode: Mode,
        parser_context: ParserContext,
        out: &mut String,
        path: RdPath,
        item_context: Option<&str>,
        parent_rlike: &mut escape::RLikeState,
    ) -> Result<(), WriteError> {
        let tagged = match node {
            RdNode::Tagged(t) => t,
            RdNode::Group(_) => return self.fail(path, UnserializableKind::BareGroup),
            RdNode::Raw(_) => return self.fail(path, UnserializableKind::RawNode),
            _ => return self.fail(path, UnserializableKind::UnsupportedNode),
        };
        if matches!(tagged.tag(), rd_ast::RdTag::Unknown(_)) {
            return self.fail(path, UnserializableKind::UnknownTag);
        }
        if matches!(parent_mode, Mode::Verbatim | Mode::Equation)
            && !matches!(tagged.tag(), rd_ast::RdTag::IfDef | rd_ast::RdTag::IfNDef)
        {
            return self.fail(path, UnserializableKind::TagNotAllowedInContext);
        }
        let spelling = tagged.tag().as_rd_tag();
        let conditional = spelling == "#ifdef" || spelling == "#ifndef";
        let quoted = parent_mode == Mode::RLike && parent_rlike.is_ordinary_quote();
        if parent_mode == Mode::RLike && escape::is_raw_string_or_comment(parent_rlike) {
            // The parser treats markup in raw strings and comments as literal R code.
            return self.fail(path, UnserializableKind::TagNotAllowedInContext);
        }
        if quoted && !conditional && !spec::recognized_in_ordinary_quote(spelling) {
            // Non-recognized markup is literal R code in an ordinary quoted string.
            return self.fail(path, UnserializableKind::TagNotAllowedInContext);
        }
        if parent_mode == Mode::RLike && !conditional {
            // The parser clones the whole R-like state, transients included,
            // into a conditional body; only ordinary macros reset transients.
            parent_rlike.clear_transient_opener();
        }
        if spelling == "\\item" {
            return self.write_item(tagged, item_context, out, path);
        }
        if spelling == "LIST" {
            if parser_context != ParserContext::Latex {
                return self.fail(
                    path,
                    if parser_context == ParserContext::Document {
                        UnserializableKind::ListInDocument
                    } else {
                        UnserializableKind::ListOutsideLatex
                    },
                );
            }
            out.push('{');
            self.sequence(
                tagged.children(),
                Mode::Latex,
                out,
                path,
                false,
                Some(spelling),
                ParserContext::Latex,
                &mut escape::RLikeState::default(),
            )?;
            out.push('}');
            return Ok(());
        }
        if spelling == "#ifdef" || spelling == "#ifndef" {
            if !out.is_empty() && !out.ends_with('\n') {
                return self.fail(path, UnserializableKind::ConditionalNotAtLineStart);
            }
            if tagged.children().len() != 2 {
                return self.fail(
                    path,
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            if let Some(index) = tagged
                .children()
                .iter()
                .position(|node| !matches!(node, RdNode::Group(_)))
            {
                return self.fail(
                    path.with_child(index),
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            if tagged.option().is_some() {
                return self.fail(
                    path.with_option(),
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            let target = tagged.children()[0].as_group().expect("checked");
            let body = tagged.children()[1].as_group().expect("checked");
            let target_path = path.with_child(0);
            if target.children().len() != 1 {
                return self.fail(
                    target_path,
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            if !matches!(target.children()[0], RdNode::Text(_))
                || !matches!(target.children()[0], RdNode::Text(ref s) if !s.contains('\r') && s.matches('\n').count() == 1 && s.ends_with('\n'))
            {
                return self.fail(
                    target_path.with_child(0),
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            out.push_str(spelling);
            if let RdNode::Text(value) = &target.children()[0] {
                self.append(out, value);
            }
            self.sequence(
                body.children(),
                parent_mode,
                out,
                path.with_child(1),
                false,
                item_context,
                parser_context,
                parent_rlike,
            )?;
            if !out.ends_with('\n') {
                return self.fail(
                    path.with_child(1),
                    UnserializableKind::ConditionalNotAtLineStart,
                );
            }
            out.push_str("#endif");
            self.newline(out);
            return Ok(());
        }
        let known = spec::tag_spec(spelling);
        let spec = match known {
            Some(s) => s,
            None => {
                return self.fail(
                    path,
                    UnserializableKind::InvalidTagSpelling {
                        spelling: spelling.into(),
                    },
                );
            }
        };
        if !quoted && !spec.contexts.contains(&parser_context) {
            return self.fail(path, UnserializableKind::TagNotAllowedInContext);
        }
        const RLIKE_ONE: &[Mode] = &[Mode::RLike];
        let args = if spelling == "\\I" && parent_mode == Mode::RLike {
            RLIKE_ONE
        } else if spelling == "\\figure" && parent_mode == Mode::RLike {
            &[Mode::Verbatim][..]
        } else {
            spec.args
        };
        let required = if spelling == "\\figure" && parent_mode == Mode::RLike {
            1
        } else {
            spec.required
        };
        // The parser gives list items distinct context-dependent shapes.
        if spelling == "\\item" {
            return self.write_item(tagged, item_context, out, path);
        }
        if tagged.option().is_some() && !spec.optional {
            return self.fail(
                path.with_option(),
                UnserializableKind::InvalidTagShape {
                    tag: spelling.into(),
                },
            );
        }
        if let Some(option) = tagged.option()
            && let Some(option_path) = invalid_option_path(option, &path.with_option())
        {
            return self.fail(option_path, UnserializableKind::InvalidOptionContent);
        }
        if tagged
            .children()
            .iter()
            .filter(|n| matches!(n, RdNode::Group(_)))
            .count()
            > 0
        {
            if tagged.children().len() < required || tagged.children().len() > args.len() {
                return self.fail(
                    path,
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
            if let Some(index) = tagged
                .children()
                .iter()
                .position(|node| !matches!(node, RdNode::Group(_)))
            {
                return self.fail(
                    path.with_child(index),
                    UnserializableKind::InvalidTagShape {
                        tag: spelling.into(),
                    },
                );
            }
        } else if tagged.children().len() < required
            || (tagged.children().is_empty() && required > 0)
            || (args.len() >= 2 && tagged.children().len() > args.len())
        {
            return self.fail(
                path,
                UnserializableKind::InvalidTagShape {
                    tag: spelling.into(),
                },
            );
        }
        if args.len() >= 2
            && tagged.children().len() >= 2
            && tagged.children().len() <= args.len()
            && let Some(index) = tagged
                .children()
                .iter()
                .position(|node| !matches!(node, RdNode::Group(_)))
        {
            return self.fail(
                path.with_child(index),
                UnserializableKind::InvalidTagShape {
                    tag: spelling.into(),
                },
            );
        }
        if args.is_empty() && !tagged.children().is_empty() {
            return self.fail(
                path,
                UnserializableKind::InvalidTagShape {
                    tag: spelling.into(),
                },
            );
        }
        out.push_str(spelling);
        if let Some(option) = tagged.option() {
            out.push('[');
            self.sequence(
                option,
                Mode::Latex,
                out,
                path.with_option(),
                false,
                None,
                ParserContext::Latex,
                &mut escape::RLikeState::default(),
            )?;
            out.push(']');
        }
        if !args.is_empty() {
            if args.len() >= 2 && tagged.children().len() >= 2 {
                for (i, child) in tagged.children().iter().enumerate() {
                    let group = child.as_group().expect("checked");
                    out.push('{');
                    let mut child_rlike = escape::RLikeState::default();
                    self.sequence(
                        group.children(),
                        args[i],
                        out,
                        path.with_child(i),
                        false,
                        Some(spelling),
                        if args[i] == Mode::RLike {
                            ParserContext::RLike
                        } else {
                            ParserContext::Latex
                        },
                        &mut child_rlike,
                    )?;
                    if args[i] == Mode::RLike && !child_rlike.closure_compatible() {
                        return self.fail(
                            path.with_child(i),
                            UnserializableKind::UnterminatedRLikeState,
                        );
                    }
                    out.push('}');
                }
            } else {
                if tagged.children().len() == 1
                    && (args.len() >= 2 || spelling == "\\figure")
                    && required == 1
                    && matches!(tagged.children()[0], RdNode::Group(_))
                {
                    let group = tagged.children()[0].as_group().expect("checked");
                    out.push('{');
                    let mut child_rlike = escape::RLikeState::default();
                    self.sequence(
                        group.children(),
                        args[0],
                        out,
                        path.with_child(0),
                        false,
                        Some(spelling),
                        if args[0] == Mode::RLike {
                            ParserContext::RLike
                        } else {
                            ParserContext::Latex
                        },
                        &mut child_rlike,
                    )?;
                    if args[0] == Mode::RLike && !child_rlike.closure_compatible() {
                        return self.fail(
                            path.with_child(0),
                            UnserializableKind::UnterminatedRLikeState,
                        );
                    }
                    out.push('}');
                    return Ok(());
                }
                if tagged
                    .children()
                    .iter()
                    .any(|n| matches!(n, RdNode::Group(_)))
                {
                    let index = tagged
                        .children()
                        .iter()
                        .position(|node| matches!(node, RdNode::Group(_)))
                        .expect("checked");
                    return self.fail(
                        path.with_child(index),
                        UnserializableKind::InvalidTagShape {
                            tag: spelling.into(),
                        },
                    );
                }
                out.push('{');
                let mut child_rlike = escape::RLikeState::default();
                self.sequence(
                    tagged.children(),
                    args[0],
                    out,
                    path.clone(),
                    false,
                    Some(spelling),
                    if args[0] == Mode::RLike {
                        ParserContext::RLike
                    } else {
                        ParserContext::Latex
                    },
                    &mut child_rlike,
                )?;
                if args[0] == Mode::RLike && !child_rlike.closure_compatible() {
                    return self.fail(path, UnserializableKind::UnterminatedRLikeState);
                }
                out.push('}');
            }
        }
        Ok(())
    }

    fn write_item(
        &mut self,
        tagged: &rd_ast::RdTagged,
        context: Option<&str>,
        out: &mut String,
        path: RdPath,
    ) -> Result<(), WriteError> {
        let argument_context = matches!(
            context,
            Some("\\arguments") | Some("\\value") | Some("\\describe")
        );
        if argument_context
            && tagged.children().len() == 2
            && let Some(index) = tagged
                .children()
                .iter()
                .position(|node| !matches!(node, RdNode::Group(_)))
        {
            return self.fail(
                path.with_child(index),
                UnserializableKind::InvalidTagShape {
                    tag: "\\item".into(),
                },
            );
        }
        match context {
            Some("\\itemize") | Some("\\enumerate") if tagged.children().is_empty() => {
                out.push_str("\\item");
                Ok(())
            }
            Some("\\arguments") | Some("\\value") | Some("\\describe")
                if tagged.children().len() == 2
                    && tagged
                        .children()
                        .iter()
                        .all(|n| matches!(n, RdNode::Group(_))) =>
            {
                out.push_str("\\item");
                for (i, child) in tagged.children().iter().enumerate() {
                    let group = child.as_group().expect("checked");
                    out.push('{');
                    self.sequence(
                        group.children(),
                        Mode::Latex,
                        out,
                        path.with_child(i),
                        false,
                        None,
                        ParserContext::Latex,
                        &mut escape::RLikeState::default(),
                    )?;
                    out.push('}');
                }
                Ok(())
            }
            _ => self.fail(
                path,
                UnserializableKind::InvalidTagShape {
                    tag: "\\item".into(),
                },
            ),
        }
    }

    fn append(&self, out: &mut String, text: &str) {
        match self.options.line_ending {
            LineEnding::Lf => out.push_str(text),
            LineEnding::CrLf => {
                for ch in text.chars() {
                    if ch == '\n' {
                        out.push_str("\r\n")
                    } else {
                        out.push(ch)
                    }
                }
            }
        }
    }
    fn newline(&self, out: &mut String) {
        self.append(out, "\n")
    }
}

fn leaf_mode(mode: Mode) -> Mode {
    match mode {
        Mode::Latex => Mode::Latex,
        Mode::RLike => Mode::RLike,
        Mode::Verbatim | Mode::Equation => Mode::Verbatim,
    }
}

fn is_newline_leaf(node: &RdNode, mode: Mode) -> bool {
    match (mode, node) {
        (Mode::Latex, RdNode::Text(s))
        | (Mode::RLike, RdNode::RCode(s))
        | (Mode::Verbatim, RdNode::Verb(s)) => s == "\n",
        (Mode::Equation, _) => false,
        _ => false,
    }
}
fn balanced_equation(s: &str) -> bool {
    let mut depth = 0usize;
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => return false,
            '}' => depth -= 1,
            _ => {}
        }
    }
    depth == 0 && !escaped
}
fn invalid_option_path(nodes: &[RdNode], path: &RdPath) -> Option<RdPath> {
    for (index, node) in nodes.iter().enumerate() {
        match node {
            RdNode::Text(s) | RdNode::RCode(s) | RdNode::Verb(s) | RdNode::Comment(s) => {
                if s.contains(']') {
                    return Some(path.with_child(index));
                }
            }
            RdNode::Tagged(tagged) => {
                let node_path = path.with_child(index);
                if let Some(option) = tagged.option()
                    && let Some(path) = invalid_option_path(option, &node_path.with_option())
                {
                    return Some(path);
                }
                if let Some(path) = invalid_option_path(tagged.children(), &node_path) {
                    return Some(path);
                }
            }
            RdNode::Group(group) => {
                let node_path = path.with_child(index);
                if let Some(path) = invalid_option_path(group.children(), &node_path) {
                    return Some(path);
                }
            }
            RdNode::Raw(_) => return Some(path.with_child(index)),
            _ => return Some(path.with_child(index)),
        }
    }
    None
}
