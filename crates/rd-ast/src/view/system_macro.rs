//! Producer-neutral views of the four documented Rd system-macro profiles.

use crate::{
    RawRdValue, RdArity, RdConstruct, RdDocument, RdNode, RdNodeKind, RdPath, RdShapeError,
    RdShapeErrorKind, RdTag, classify_raw_node,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdSystemMacroOrigin {
    CuratedTag,
    UserMacroExpansion,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum RdSystemMacro<'a> {
    Doi { id: &'a str },
    CranPkg { package: &'a str },
    Sspace,
    I { body: &'a [RdNode] },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdSystemMacroMatch<'a> {
    path: RdPath,
    semantic: RdSystemMacro<'a>,
    origin: RdSystemMacroOrigin,
    consumed: usize,
}

impl<'a> RdSystemMacroMatch<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    /// Returns the producer-neutral meaning of the matched sibling sequence.
    pub fn semantic(&self) -> RdSystemMacro<'a> {
        self.semantic
    }
    pub fn origin(&self) -> RdSystemMacroOrigin {
        self.origin
    }
    pub fn consumed(&self) -> usize {
        self.consumed
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdSystemMacroItem<'a> {
    Macro(RdSystemMacroMatch<'a>),
    Node { path: RdPath, node: &'a RdNode },
}

#[derive(Debug, Clone)]
pub struct RdSystemMacroItems<'a> {
    nodes: &'a [RdNode],
    parent_path: Option<RdPath>,
    index: usize,
}

#[derive(Debug, Clone)]
pub struct RdSystemMacroItemsStrict<'a> {
    nodes: &'a [RdNode],
    parent_path: Option<RdPath>,
    index: usize,
}

impl RdDocument {
    pub fn system_macro_items(&self) -> RdSystemMacroItems<'_> {
        RdSystemMacroItems::top_level(self.nodes())
    }
    pub fn inspect_system_macro_items(&self) -> RdSystemMacroItemsStrict<'_> {
        RdSystemMacroItemsStrict::top_level(self.nodes())
    }
}

impl<'a> RdSystemMacroItems<'a> {
    pub fn top_level(nodes: &'a [RdNode]) -> Self {
        Self {
            nodes,
            parent_path: None,
            index: 0,
        }
    }
    pub fn children(nodes: &'a [RdNode], parent_path: &RdPath) -> Self {
        Self {
            nodes,
            parent_path: Some(parent_path.clone()),
            index: 0,
        }
    }
    fn path(&self, index: usize) -> RdPath {
        self.parent_path.as_ref().map_or_else(
            || RdPath::new(vec![crate::RdPathSegment::TopLevel(index)]),
            |path| path.with_child(index),
        )
    }
}

impl<'a> RdSystemMacroItemsStrict<'a> {
    pub fn top_level(nodes: &'a [RdNode]) -> Self {
        Self {
            nodes,
            parent_path: None,
            index: 0,
        }
    }
    pub fn children(nodes: &'a [RdNode], parent_path: &RdPath) -> Self {
        Self {
            nodes,
            parent_path: Some(parent_path.clone()),
            index: 0,
        }
    }
    fn path(&self, index: usize) -> RdPath {
        self.parent_path.as_ref().map_or_else(
            || RdPath::new(vec![crate::RdPathSegment::TopLevel(index)]),
            |path| path.with_child(index),
        )
    }
}

impl<'a> Iterator for RdSystemMacroItems<'a> {
    type Item = RdSystemMacroItem<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let node = self.nodes.get(index)?;
        let path = self.path(index);
        self.index += 1;
        if let Some((semantic, consumed)) = recognize(
            node,
            self.nodes.get(index + 1),
            &path,
            &self.path(index + 1),
        ) {
            self.index += consumed - 1;
            return Some(RdSystemMacroItem::Macro(RdSystemMacroMatch {
                path,
                semantic,
                origin: origin(node, consumed),
                consumed,
            }));
        }
        Some(RdSystemMacroItem::Node { path, node })
    }
}

impl<'a> Iterator for RdSystemMacroItemsStrict<'a> {
    type Item = Result<RdSystemMacroItem<'a>, RdShapeError>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        let node = self.nodes.get(index)?;
        let path = self.path(index);
        self.index += 1;
        match inspect(
            node,
            self.nodes.get(index + 1),
            &path,
            &self.path(index + 1),
        ) {
            Ok(Some((semantic, consumed, origin))) => {
                self.index += consumed - 1;
                Some(Ok(RdSystemMacroItem::Macro(RdSystemMacroMatch {
                    path,
                    semantic,
                    origin,
                    consumed,
                })))
            }
            Ok(None) => Some(Ok(RdSystemMacroItem::Node { path, node })),
            Err(error) => Some(Err(error)),
        }
    }
}

fn origin(node: &RdNode, consumed: usize) -> RdSystemMacroOrigin {
    if consumed == 1 && node.as_tagged().is_some() {
        RdSystemMacroOrigin::CuratedTag
    } else {
        RdSystemMacroOrigin::UserMacroExpansion
    }
}

fn recognize<'a>(
    node: &'a RdNode,
    following: Option<&RdNode>,
    path: &RdPath,
    following_path: &RdPath,
) -> Option<(RdSystemMacro<'a>, usize)> {
    if let Some(tagged) = node.as_tagged() {
        return curated(tagged.tag(), tagged.option(), tagged.children(), path)
            .ok()
            .flatten()
            .map(|s| (s, 1));
    }
    let raw = node.as_raw()?;
    if !matches!(
        classify_raw_node(raw),
        crate::RawNodeClassification::ExpectedUserMacroDefinition
    ) {
        return None;
    }
    let name = macro_name(raw)?;
    let text = match raw.children() {
        [RdNode::Text(text)] => text.as_str(),
        _ => return None,
    };
    let (semantic, arg) = definition(name, text)?;
    let expansion = following?;
    valid_expansion(name, arg, expansion, following_path)
        .ok()
        .map(|()| (semantic, 2))
}

fn inspect<'a>(
    node: &'a RdNode,
    following: Option<&RdNode>,
    path: &RdPath,
    following_path: &RdPath,
) -> Result<Option<(RdSystemMacro<'a>, usize, RdSystemMacroOrigin)>, RdShapeError> {
    if let Some(tagged) = node.as_tagged() {
        return curated(tagged.tag(), tagged.option(), tagged.children(), path)
            .map(|s| s.map(|s| (s, 1, RdSystemMacroOrigin::CuratedTag)));
    }
    let Some(raw) = node.as_raw() else {
        return Ok(None);
    };
    if let Some(tag) = raw.tag().map(RdTag::from_rd_tag)
        && matches!(tag, RdTag::Doi | RdTag::CranPkg | RdTag::Sspace | RdTag::I)
    {
        return Err(error(
            path.clone(),
            Some(tag),
            RdShapeErrorKind::UnexpectedNode {
                expected: crate::RdExpectedNode::Tagged,
                actual: RdNodeKind::Raw,
            },
        ));
    }
    if !matches!(
        classify_raw_node(raw),
        crate::RawNodeClassification::ExpectedUserMacroDefinition
    ) {
        return Ok(None);
    }
    let Some(name) = macro_name(raw) else {
        return Ok(None);
    };
    if !matches!(name, r"\doi" | r"\CRANpkg" | r"\sspace" | r"\I") {
        return Ok(None);
    }
    let construct = RdConstruct::SystemMacro(name.to_string());
    if name == r"\I" {
        return Err(error(
            path.clone(),
            None,
            RdShapeErrorKind::UnsupportedRepresentation { construct },
        ));
    }
    let text = match raw.children() {
        [RdNode::Text(text)] => text.as_str(),
        _ => unreachable!(),
    };
    let Some((semantic, arg)) = definition(name, text) else {
        return Err(error(
            path.clone(),
            None,
            RdShapeErrorKind::DefinitionMismatch { construct },
        ));
    };
    let Some(expansion) = following else {
        return Err(error(
            path.clone(),
            None,
            RdShapeErrorKind::MissingFollowing {
                construct: RdConstruct::SystemMacroExpansion(name.to_string()),
            },
        ));
    };
    valid_expansion(name, arg, expansion, following_path)
        .map(|()| Some((semantic, 2, RdSystemMacroOrigin::UserMacroExpansion)))
}

fn macro_name(raw: &crate::RawRdNode) -> Option<&str> {
    raw.attributes()
        .iter()
        .find(|a| a.name() == "macro")
        .and_then(|a| match a.value().value() {
            RawRdValue::Character(values) if values.len() == 1 => values[0].as_deref(),
            _ => None,
        })
}

fn definition<'a>(name: &str, text: &'a str) -> Option<(RdSystemMacro<'a>, &'a str)> {
    let mut scanner = DefinitionScanner::new(text);
    let arg = match name {
        r"\doi" => {
            scanner.expect_control_word(DOI_TAG)?;
            scanner.expect_option(DOI_OPTION)?;
            scanner.expect_group_start()?;
            scanner.expect_atom(DOI_FUNCTION)?;
            scanner.expect_atom("(")?;
            scanner.expect_atom(r#"""#)?;
            scanner.expect_atom(DOI_PLACEHOLDER)?;
            scanner.expect_atom(r#"""#)?;
            scanner.expect_atom(")")?;
            scanner.expect_group_end()?;
            scanner.finish_and_remainder()
        }
        r"\CRANpkg" => {
            scanner.expect_control_word(CRAN_TAG)?;
            scanner.expect_group_start()?;
            scanner.expect_atom(CRAN_URL_PREFIX)?;
            scanner.expect_atom(CRAN_PLACEHOLDER)?;
            scanner.expect_group_end()?;
            scanner.expect_group_start()?;
            scanner.expect_control_word(CRAN_DISPLAY_TAG)?;
            scanner.expect_group_start()?;
            scanner.expect_atom(CRAN_PLACEHOLDER)?;
            scanner.expect_group_end()?;
            scanner.expect_group_end()?;
            scanner.finish_and_remainder()
        }
        r"\sspace" => {
            scanner.expect_control_word(SSPACE_TAG)?;
            scanner.expect_group_start()?;
            scanner.expect_atom(SSPACE_LATEX)?;
            scanner.expect_group_end()?;
            scanner.expect_group_start()?;
            scanner.expect_control_word(SSPACE_OUT_TAG)?;
            scanner.expect_group_start()?;
            scanner.expect_atom(SSPACE_TILDE)?;
            scanner.expect_group_end()?;
            scanner.expect_group_end()?;
            scanner.expect_group_start()?;
            scanner.expect_atom(SSPACE_PLAIN)?;
            scanner.expect_group_end()?;
            scanner.finish_and_remainder()
        }
        _ => return None,
    };
    match name {
        r"\doi" => Some((RdSystemMacro::Doi { id: arg }, arg)),
        r"\CRANpkg" => Some((RdSystemMacro::CranPkg { package: arg }, arg)),
        r"\sspace" if arg.is_empty() => Some((RdSystemMacro::Sspace, arg)),
        _ => None,
    }
}

const DOI_TAG: &str = r"\Sexpr";
const DOI_OPTION: &str = "results=rd";
const DOI_FUNCTION: &str = "tools:::Rd_expr_doi";
const DOI_PLACEHOLDER: &str = "#1";
const CRAN_TAG: &str = r"\href";
const CRAN_URL_PREFIX: &str = "https://CRAN.R-project.org/package=";
const CRAN_PLACEHOLDER: &str = "#1";
const CRAN_DISPLAY_TAG: &str = r"\pkg";
const SSPACE_TAG: &str = r"\ifelse";
const SSPACE_LATEX: &str = "latex";
const SSPACE_OUT_TAG: &str = r"\out";
const SSPACE_TILDE: &str = "~";
const SSPACE_PLAIN: &str = " ";

struct DefinitionScanner<'a> {
    text: &'a str,
    position: usize,
}

impl<'a> DefinitionScanner<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, position: 0 }
    }
    fn expect_atom(&mut self, atom: &str) -> Option<()> {
        self.text[self.position..]
            .starts_with(atom)
            .then(|| self.position += atom.len())
    }
    fn expect_control_word(&mut self, word: &str) -> Option<()> {
        self.expect_atom(word)
    }
    fn expect_option(&mut self, option: &str) -> Option<()> {
        self.expect_atom("[")?;
        self.expect_atom(option)?;
        self.expect_atom("]")
    }
    fn expect_group_start(&mut self) -> Option<()> {
        self.expect_atom("{")
    }
    fn expect_group_end(&mut self) -> Option<()> {
        self.expect_atom("}")
    }
    fn finish_and_remainder(self) -> &'a str {
        &self.text[self.position..]
    }
}

fn valid_expansion(
    name: &str,
    arg: &str,
    node: &RdNode,
    path: &RdPath,
) -> Result<(), RdShapeError> {
    let mismatch = || {
        error(
            path.clone(),
            None,
            RdShapeErrorKind::UnexpectedNode {
                expected: match name {
                    r"\doi" => crate::RdExpectedNode::Sexpr,
                    r"\CRANpkg" => crate::RdExpectedNode::Href,
                    _ => crate::RdExpectedNode::Tagged,
                },
                actual: RdNodeKind::of(node),
            },
        )
    };
    match name {
        r"\doi" => match node.as_tagged() {
            Some(t)
                if t.tag() == &RdTag::Sexpr
                    && t.option() == Some(&[RdNode::Text("results=rd".into())][..])
                    && t.children()
                        == [RdNode::RCode(format!(r#"tools:::Rd_expr_doi("{}")"#, arg))] =>
            {
                Ok(())
            }
            _ => Err(mismatch()),
        },
        r"\CRANpkg" => {
            let Some(t) = node.as_tagged() else {
                return Err(mismatch());
            };
            if t.tag() != &RdTag::Href || t.option().is_some() || t.children().len() != 2 {
                return Err(mismatch());
            }
            let [url, display] = t.children() else {
                unreachable!()
            };
            if url.as_group().is_none() || display.as_group().is_none() {
                return Err(mismatch());
            }
            if url.as_group().unwrap().children()
                != [RdNode::Verb(format!(
                    "https://CRAN.R-project.org/package={arg}"
                ))]
            {
                return Err(mismatch());
            }
            if display.as_group().unwrap().children()
                != [RdNode::tagged(
                    RdTag::Pkg,
                    None,
                    vec![RdNode::Text(arg.into())],
                )]
            {
                return Err(mismatch());
            }
            Ok(())
        }
        r"\sspace" => {
            let Some(t) = node.as_tagged() else {
                return Err(mismatch());
            };
            if t.tag() != &RdTag::IfElse || t.option().is_some() || t.children().len() != 3 {
                return Err(mismatch());
            }
            let [a, b, c] = t.children() else {
                unreachable!()
            };
            let (Some(a), Some(b), Some(c)) = (a.as_group(), b.as_group(), c.as_group()) else {
                return Err(mismatch());
            };
            if a.children() != [RdNode::Text("latex".into())]
                || c.children() != [RdNode::Text(" ".into())]
            {
                return Err(mismatch());
            }
            let [out] = b.children() else {
                return Err(mismatch());
            };
            let Some(out) = out.as_tagged() else {
                return Err(mismatch());
            };
            if out.tag() != &RdTag::Out
                || out.option().is_some()
                || out.children() != [RdNode::Verb("~".into())]
            {
                return Err(mismatch());
            }
            Ok(())
        }
        _ => unreachable!(),
    }
}

fn curated<'a>(
    tag: &RdTag,
    option: Option<&[RdNode]>,
    children: &'a [RdNode],
    path: &RdPath,
) -> Result<Option<RdSystemMacro<'a>>, RdShapeError> {
    if !matches!(tag, RdTag::Doi | RdTag::CranPkg | RdTag::Sspace | RdTag::I) {
        return Ok(None);
    }
    if option.is_some() {
        return Err(error(
            path.clone(),
            Some(tag.clone()),
            RdShapeErrorKind::UnexpectedOption,
        ));
    }
    let value = match tag {
        RdTag::Doi => Some(RdSystemMacro::Doi {
            id: one_text(children, tag, path)?,
        }),
        RdTag::CranPkg => Some(RdSystemMacro::CranPkg {
            package: one_text(children, tag, path)?,
        }),
        RdTag::Sspace => {
            if !children.is_empty() {
                return Err(error(
                    path.clone(),
                    Some(tag.clone()),
                    RdShapeErrorKind::WrongArity {
                        expected: RdArity::Exactly(0),
                        actual: children.len(),
                    },
                ));
            }
            Some(RdSystemMacro::Sspace)
        }
        RdTag::I => Some(RdSystemMacro::I { body: children }),
        _ => None,
    };
    Ok(value)
}

fn one_text<'a>(
    children: &'a [RdNode],
    tag: &RdTag,
    path: &RdPath,
) -> Result<&'a str, RdShapeError> {
    if children.len() != 1 {
        return Err(error(
            path.clone(),
            Some(tag.clone()),
            RdShapeErrorKind::WrongArity {
                expected: RdArity::Exactly(1),
                actual: children.len(),
            },
        ));
    }
    match &children[0] {
        RdNode::Text(s) => Ok(s),
        node => Err(error(
            path.with_child(0),
            Some(tag.clone()),
            RdShapeErrorKind::UnexpectedContent {
                actual: RdNodeKind::of(node),
            },
        )),
    }
}

fn error(path: RdPath, tag: Option<RdTag>, kind: RdShapeErrorKind) -> RdShapeError {
    RdShapeError::new(path, tag, kind)
}
