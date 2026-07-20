use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RdLifecycleStage {
    Experimental,
    Stable,
    Superseded,
    Deprecated,
    Maturing,
    Questioning,
    SoftDeprecated,
    Defunct,
    Retired,
    Unknown(String),
}

impl RdLifecycleStage {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Experimental => "experimental",
            Self::Stable => "stable",
            Self::Superseded => "superseded",
            Self::Deprecated => "deprecated",
            Self::Maturing => "maturing",
            Self::Questioning => "questioning",
            Self::SoftDeprecated => "soft-deprecated",
            Self::Defunct => "defunct",
            Self::Retired => "retired",
            Self::Unknown(value) => value,
        }
    }

    pub fn is_current(&self) -> bool {
        matches!(
            self,
            Self::Experimental | Self::Stable | Self::Superseded | Self::Deprecated
        )
    }

    pub fn is_legacy(&self) -> bool {
        matches!(
            self,
            Self::Maturing
                | Self::Questioning
                | Self::SoftDeprecated
                | Self::Defunct
                | Self::Retired
        )
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

fn stage_from_figure(figure: &RdFigure<'_>) -> Option<RdLifecycleStage> {
    let basename = figure.file().rsplit('/').next()?;
    if !basename.starts_with("lifecycle-") {
        return None;
    }
    let stem = basename
        .rfind('.')
        .map_or(basename, |index| &basename[..index]);
    let spelling = stem.strip_prefix("lifecycle-")?;
    if spelling.is_empty() {
        return None;
    }
    Some(match spelling {
        value if value.eq_ignore_ascii_case("experimental") => RdLifecycleStage::Experimental,
        value if value.eq_ignore_ascii_case("stable") => RdLifecycleStage::Stable,
        value if value.eq_ignore_ascii_case("superseded") => RdLifecycleStage::Superseded,
        value if value.eq_ignore_ascii_case("deprecated") => RdLifecycleStage::Deprecated,
        value if value.eq_ignore_ascii_case("maturing") => RdLifecycleStage::Maturing,
        value if value.eq_ignore_ascii_case("questioning") => RdLifecycleStage::Questioning,
        value if value.eq_ignore_ascii_case("soft-deprecated") => RdLifecycleStage::SoftDeprecated,
        value if value.eq_ignore_ascii_case("defunct") => RdLifecycleStage::Defunct,
        value if value.eq_ignore_ascii_case("retired") => RdLifecycleStage::Retired,
        value => RdLifecycleStage::Unknown(value.to_owned()),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdLifecycleBadgeShape<'a> {
    conditional: RdConditional<'a>,
    href: RdHref<'a>,
    fallback: RdInlineSpan<'a>,
    fallback_text: String,
}

impl<'a> RdLifecycleBadgeShape<'a> {
    pub fn conditional(&self) -> &RdConditional<'a> {
        &self.conditional
    }
    pub fn href(&self) -> &RdHref<'a> {
        &self.href
    }
    pub fn fallback(&self) -> &RdInlineSpan<'a> {
        &self.fallback
    }
    pub fn fallback_text(&self) -> &str {
        &self.fallback_text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdLifecycleBadge<'a> {
    stage: RdLifecycleStage,
    figure: RdFigure<'a>,
    canonical_shape: Option<RdLifecycleBadgeShape<'a>>,
}

impl<'a> RdLifecycleBadge<'a> {
    pub fn stage(&self) -> &RdLifecycleStage {
        &self.stage
    }
    pub fn figure(&self) -> &RdFigure<'a> {
        &self.figure
    }
    pub fn path(&self) -> &RdPath {
        self.figure.path()
    }
    pub fn canonical_shape(&self) -> Option<&RdLifecycleBadgeShape<'a>> {
        self.canonical_shape.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdLifecycleBadges<'a> {
    badges: Vec<RdLifecycleBadge<'a>>,
    diagnostics: Vec<RdShapeError>,
}

impl<'a> RdLifecycleBadges<'a> {
    pub fn as_slice(&self) -> &[RdLifecycleBadge<'a>] {
        &self.badges
    }
    pub fn iter(&self) -> impl Iterator<Item = &RdLifecycleBadge<'a>> {
        self.badges.iter()
    }
    pub fn first(&self) -> Option<&RdLifecycleBadge<'a>> {
        self.badges.first()
    }
    pub fn diagnostics(&self) -> &[RdShapeError] {
        &self.diagnostics
    }
}

impl<'a> IntoIterator for &'a RdLifecycleBadges<'a> {
    type Item = &'a RdLifecycleBadge<'a>;
    type IntoIter = std::slice::Iter<'a, RdLifecycleBadge<'a>>;
    fn into_iter(self) -> Self::IntoIter {
        self.badges.iter()
    }
}

fn significant(nodes: &[RdNode]) -> impl Iterator<Item = (usize, &RdNode)> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| !is_inter_item_trivia(node))
}

fn canonical_match<'a>(
    node: &'a RdNode,
    path: &RdPath,
) -> Option<(
    RdLifecycleBadgeShape<'a>,
    RdFigure<'a>,
    RdLifecycleStage,
    RdPath,
)> {
    let conditional = node.inspect_conditional(path).ok().flatten()?;
    if conditional.kind() != RdConditionalKind::IfElse || conditional.format() != "html" {
        return None;
    }
    let (then_index, then_node) = exactly_one(conditional.then_branch())?;
    let href_tagged = then_node.as_tagged()?;
    let href_path = path.with_child(1).with_child(then_index);
    let href = href_tagged.inspect_href(&href_path).ok()?;
    let (url_index, url_node) = exactly_one(href.url())?;
    if !matches!(url_node, RdNode::Verb(value) if !value.is_empty()) {
        return None;
    }
    let (display_index, display_node) = exactly_one(href.display())?;
    let figure_path = href_path.with_child(1).with_child(display_index);
    let figure = display_node.inspect_figure(&figure_path).ok().flatten()?;
    let stage = stage_from_figure(&figure)?;
    let (fallback_index, fallback_node) = exactly_one(conditional.else_branch()?)?;
    let fallback_path = path.with_child(2).with_child(fallback_index);
    let fallback = fallback_node.inline_span(&fallback_path)?;
    if fallback.kind() != RdInlineSpanKind::Strong {
        return None;
    }
    let [RdNode::Text(text)] = fallback.body() else {
        return None;
    };
    let inner = text.strip_prefix('[')?.strip_suffix(']')?;
    if inner.is_empty() || normalize_stage(inner) != normalize_stage(stage.as_str()) {
        return None;
    }
    let _ = url_index;
    Some((
        RdLifecycleBadgeShape {
            conditional,
            href,
            fallback,
            fallback_text: inner.to_owned(),
        },
        figure,
        stage,
        figure_path,
    ))
}

fn exactly_one(nodes: &[RdNode]) -> Option<(usize, &RdNode)> {
    let mut items = significant(nodes);
    let item = items.next()?;
    items.next().is_none().then_some(item)
}

fn normalize_stage(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
        .to_ascii_lowercase()
}

struct Collector<'a> {
    badges: Vec<RdLifecycleBadge<'a>>,
    diagnostics: Vec<RdShapeError>,
    inspect: bool,
}

impl<'a> Collector<'a> {
    fn visit(&mut self, nodes: &'a [RdNode], base: &RdPath, skip: Option<&RdPath>) {
        for (index, node) in nodes.iter().enumerate() {
            let path = base.with_child(index);
            let mut nested_skip = None;
            if let Some((shape, figure, stage, figure_path)) = canonical_match(node, &path) {
                self.badges.push(RdLifecycleBadge {
                    stage,
                    figure,
                    canonical_shape: Some(shape),
                });
                nested_skip = Some(figure_path);
            }
            if skip != Some(&path) {
                match node.inspect_figure(&path) {
                    Ok(Some(figure)) => {
                        if let Some(stage) = stage_from_figure(&figure) {
                            self.badges.push(RdLifecycleBadge {
                                stage,
                                figure,
                                canonical_shape: None,
                            });
                        }
                    }
                    Ok(None) => {}
                    Err(error) => self.report(error),
                }
            }
            if matches!(node, RdNode::Raw(_)) {
                continue;
            }
            match node {
                RdNode::Tagged(tagged) => {
                    self.visit(tagged.children(), &path, nested_skip.as_ref().or(skip))
                }
                RdNode::Group(group) => {
                    self.visit(group.children(), &path, nested_skip.as_ref().or(skip))
                }
                _ => {}
            }
        }
    }

    fn report(&mut self, error: RdShapeError) {
        if self.inspect {
            self.diagnostics.push(error);
        }
    }
}

impl RdDocument {
    pub fn lifecycle_badges(&self) -> RdLifecycleBadges<'_> {
        self.collect_lifecycle(false)
    }
    pub fn inspect_lifecycle_badges(&self) -> RdLifecycleBadges<'_> {
        self.collect_lifecycle(true)
    }

    fn collect_lifecycle(&self, inspect: bool) -> RdLifecycleBadges<'_> {
        let mut collector = Collector {
            badges: Vec::new(),
            diagnostics: Vec::new(),
            inspect,
        };
        let mut first_description = None;
        for (index, node) in self.nodes().iter().enumerate() {
            let path = top_path(index);
            let Some(tagged) = node.as_tagged() else {
                if node
                    .as_raw()
                    .and_then(|raw| raw.tag())
                    .is_some_and(|tag| tag == RdTag::Description.as_rd_tag())
                {
                    collector.report(shape(
                        path.clone(),
                        Some(RdTag::Description),
                        RdShapeErrorKind::UnexpectedNode {
                            expected: RdExpectedNode::Tagged,
                            actual: RdNodeKind::Raw,
                        },
                    ));
                }
                continue;
            };
            if tagged.tag() != &RdTag::Description {
                continue;
            }
            if let Some(first) = first_description {
                collector.report(shape(
                    path.clone(),
                    Some(RdTag::Description),
                    RdShapeErrorKind::Duplicate {
                        construct: RdConstruct::Tag(RdTag::Description),
                        first_path: top_path(first),
                    },
                ));
            } else {
                first_description = Some(index);
            }
            if tagged.option().is_some() {
                collector.report(shape(
                    path.clone(),
                    Some(RdTag::Description),
                    RdShapeErrorKind::UnexpectedOption,
                ));
            }
            collector.visit(tagged.children(), &path, None);
        }
        RdLifecycleBadges {
            badges: collector.badges,
            diagnostics: collector.diagnostics,
        }
    }
}
