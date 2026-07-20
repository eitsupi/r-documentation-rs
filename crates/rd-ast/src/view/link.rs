use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct RdLink<'a> {
    path: RdPath,
    display: &'a [RdNode],
    destination: RdLinkDestination<'a>,
}

impl<'a> RdLink<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn display(&self) -> &'a [RdNode] {
        self.display
    }
    pub fn destination(&self) -> &RdLinkDestination<'a> {
        &self.destination
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdLinkDestination<'a> {
    DisplayText {
        nodes: &'a [RdNode],
    },
    Explicit {
        topic: &'a str,
    },
    Package {
        package: &'a str,
        topic: RdLinkTopic<'a>,
    },
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdLinkTopic<'a> {
    DisplayText(&'a [RdNode]),
    Explicit(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdHref<'a> {
    path: RdPath,
    url: &'a [RdNode],
    display: &'a [RdNode],
}
impl<'a> RdHref<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn url(&self) -> &'a [RdNode] {
        self.url
    }
    pub fn display(&self) -> &'a [RdNode] {
        self.display
    }
}

/// A source-level `\linkS4class` view. Link resolution and rendering policy
/// belongs to consumers, and differs from [`RdLink`].
#[derive(Debug, Clone, PartialEq)]
pub struct RdS4ClassLink<'a> {
    path: RdPath,
    class: &'a [RdNode],
    package: Option<&'a [RdNode]>,
}

impl<'a> RdS4ClassLink<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    pub fn class(&self) -> &'a [RdNode] {
        self.class
    }
    pub fn package(&self) -> Option<&'a [RdNode]> {
        self.package
    }
    pub fn class_text(&self) -> Option<String> {
        text_only(self.class)
    }
    pub fn package_text(&self) -> Option<String> {
        self.package.and_then(text_only)
    }
}

fn text_only(nodes: &[RdNode]) -> Option<String> {
    let mut result = String::new();
    for node in nodes {
        let RdNode::Text(value) = node else {
            return None;
        };
        result.push_str(value);
    }
    Some(result)
}

impl RdNode {
    pub fn s4_class_link(&self, base_path: &RdPath) -> Option<RdS4ClassLink<'_>> {
        self.inspect_s4_class_link(base_path).ok().flatten()
    }
    pub fn inspect_s4_class_link(
        &self,
        base_path: &RdPath,
    ) -> Result<Option<RdS4ClassLink<'_>>, RdShapeError> {
        let tagged = match self {
            RdNode::Tagged(tagged) => tagged,
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if tag == RdTag::LinkS4Class {
                    return Err(shape(
                        base_path.clone(),
                        Some(tag),
                        RdShapeErrorKind::UnexpectedNode {
                            expected: RdExpectedNode::Tagged,
                            actual: RdNodeKind::Raw,
                        },
                    ));
                }
                return Ok(None);
            }
            _ => return Ok(None),
        };
        if tagged.tag() != &RdTag::LinkS4Class {
            return Ok(None);
        }
        Ok(Some(RdS4ClassLink {
            path: base_path.clone(),
            class: tagged.children(),
            package: tagged.option(),
        }))
    }
}

impl RdTagged {
    /// A wrong tag uses `UnexpectedNode`; `actual: Tagged` identifies the
    /// node kind while the expected variant identifies the requested view.
    pub fn inspect_link<'a>(&'a self, base_path: &RdPath) -> Result<RdLink<'a>, RdShapeError> {
        if self.tag() != &RdTag::Link {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Link,
                    actual: RdNodeKind::Tagged,
                },
            ));
        }
        let display = self.children();
        let all_text = |nodes: &[RdNode], path: &RdPath| -> Result<(), RdShapeError> {
            for (index, node) in nodes.iter().enumerate() {
                if !matches!(node, RdNode::Text(_)) {
                    return Err(shape(
                        path.with_child(index),
                        Some(RdTag::Link),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::of(node),
                        },
                    ));
                }
            }
            Ok(())
        };
        let destination = match self.option() {
            None => {
                all_text(display, base_path)?;
                RdLinkDestination::DisplayText { nodes: display }
            }
            Some(option) => {
                let option_path = base_path.with_option();
                let [node] = option else {
                    return Err(shape(
                        option_path,
                        Some(RdTag::Link),
                        RdShapeErrorKind::WrongArity {
                            expected: RdArity::Exactly(1),
                            actual: option.len(),
                        },
                    ));
                };
                let RdNode::Text(value) = node else {
                    return Err(shape(
                        option_path.with_child(0),
                        Some(RdTag::Link),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::of(node),
                        },
                    ));
                };
                let value = value.as_str();
                if let Some(topic) = value.strip_prefix('=') {
                    RdLinkDestination::Explicit { topic }
                } else if let Some((package, topic)) = value.split_once(':') {
                    RdLinkDestination::Package {
                        package,
                        topic: RdLinkTopic::Explicit(topic),
                    }
                } else {
                    all_text(display, base_path)?;
                    RdLinkDestination::Package {
                        package: value,
                        topic: RdLinkTopic::DisplayText(display),
                    }
                }
            }
        };
        Ok(RdLink {
            path: base_path.clone(),
            display,
            destination,
        })
    }

    pub fn inspect_href<'a>(&'a self, base_path: &RdPath) -> Result<RdHref<'a>, RdShapeError> {
        if self.tag() != &RdTag::Href {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Href,
                    actual: RdNodeKind::Tagged,
                },
            ));
        }
        if self.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Href),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        let children = self.children();
        if children.len() != 2 {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Href),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(2),
                    actual: children.len(),
                },
            ));
        }
        let [url, display] = children else {
            unreachable!()
        };
        let url = url.as_group().ok_or_else(|| {
            shape(
                base_path.with_child(0),
                Some(RdTag::Href),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(url),
                },
            )
        })?;
        let display = display.as_group().ok_or_else(|| {
            shape(
                base_path.with_child(1),
                Some(RdTag::Href),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(display),
                },
            )
        })?;
        Ok(RdHref {
            path: base_path.clone(),
            url: url.children(),
            display: display.children(),
        })
    }
}
