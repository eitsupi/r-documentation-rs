use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct RdList<'a> {
    path: RdAstPath,
    kind: RdListKind,
    children: &'a [RdNode],
}

impl<'a> RdList<'a> {
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }
    pub fn kind(&self) -> RdListKind {
        self.kind
    }
    pub fn children(&self) -> &'a [RdNode] {
        self.children
    }
    /// Returns the list children as a positioned sibling sequence.
    pub fn children_ref(&self) -> RdNodesRef<'a> {
        RdNodesRef::from_slice(self.children, self.path.clone())
    }

    pub fn items(&self) -> impl Iterator<Item = Result<RdListItem<'a>, RdShapeError>> + '_ {
        ListItems {
            list: self,
            index: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdListKind {
    Itemize,
    Enumerate,
    Describe,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum RdListItem<'a> {
    Delimited(RdDelimitedItem<'a>),
    Described(RdDescribedItem<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdDelimitedItem<'a> {
    path: RdAstPath,
    body: &'a [RdNode],
    body_ref: RdNodesRef<'a>,
}

impl<'a> RdDelimitedItem<'a> {
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }
    /// Returns the consumed body range with absolute sibling indices.
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        self.body_ref.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RdDescribedItem<'a> {
    path: RdAstPath,
    label_group: &'a RdNode,
    body_group: &'a RdNode,
    label: &'a [RdNode],
    body: &'a [RdNode],
}

impl<'a> RdDescribedItem<'a> {
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }
    pub fn label(&self) -> &'a [RdNode] {
        self.label
    }
    /// Returns the label as a positioned sibling sequence.
    pub fn label_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.label_group, self.path.with_child(0)).children()
    }
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }
    /// Returns the description as a positioned sibling sequence.
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        RdNodeRef::new(self.body_group, self.path.with_child(1)).children()
    }
}
impl RdTagged {
    /// Strictly inspects a single list container. Itemize and enumerate use
    /// zero-child item markers; describe uses two positional groups per item.
    pub(crate) fn inspect_list<'a>(
        &'a self,
        base_path: &RdAstPath,
    ) -> Result<RdList<'a>, RdShapeError> {
        let kind = match self.tag() {
            RdTag::Itemize => RdListKind::Itemize,
            RdTag::Enumerate => RdListKind::Enumerate,
            RdTag::Describe => RdListKind::Describe,
            tag => {
                return Err(shape(
                    base_path.clone(),
                    Some(tag.clone()),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::List,
                        actual: RdNodeKind::Tagged,
                    },
                ));
            }
        };
        if self.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        Ok(RdList {
            path: base_path.clone(),
            kind,
            children: self.children(),
        })
    }
}

struct ListItems<'list, 'a> {
    list: &'list RdList<'a>,
    index: usize,
}

impl<'list, 'a> Iterator for ListItems<'list, 'a> {
    type Item = Result<RdListItem<'a>, RdShapeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.list.kind == RdListKind::Describe {
            while self.index < self.list.children.len() {
                let index = self.index;
                self.index += 1;
                let node = &self.list.children[index];
                if is_inter_item_trivia(node) {
                    continue;
                }
                let path = self.list.path.with_child(index);
                let Some(tagged) = node.as_tagged() else {
                    return Some(Err(shape(
                        path,
                        node_tag(node),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::of(node),
                        },
                    )));
                };
                if tagged.tag() != &RdTag::Item {
                    return Some(Err(shape(
                        path,
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::UnexpectedContent {
                            actual: RdNodeKind::Tagged,
                        },
                    )));
                }
                return Some(inspect_two_group_item(tagged, &path).map(|(label, body)| {
                    let [label_group, body_group] = tagged.children() else {
                        unreachable!()
                    };
                    RdListItem::Described(RdDescribedItem {
                        path,
                        label_group,
                        body_group,
                        label,
                        body,
                    })
                }));
            }
            return None;
        }

        while self.index < self.list.children.len() {
            let index = self.index;
            let node = &self.list.children[index];
            if is_inter_item_trivia(node) {
                self.index += 1;
                continue;
            }
            let path = self.list.path.with_child(index);
            let Some(tagged) = node.as_tagged() else {
                self.index += 1;
                return Some(Err(shape(
                    path,
                    node_tag(node),
                    RdShapeErrorKind::UnexpectedContent {
                        actual: RdNodeKind::of(node),
                    },
                )));
            };
            if tagged.tag() != &RdTag::Item {
                self.index += 1;
                return Some(Err(shape(
                    path,
                    Some(tagged.tag().clone()),
                    RdShapeErrorKind::UnexpectedContent {
                        actual: RdNodeKind::Tagged,
                    },
                )));
            }
            self.index += 1;
            let body_start = self.index;
            while self.index < self.list.children.len() {
                let candidate = &self.list.children[self.index];
                if candidate
                    .as_tagged()
                    .is_some_and(|item| item.tag() == &RdTag::Item)
                {
                    break;
                }
                self.index += 1;
            }
            let body = &self.list.children[body_start..self.index];
            if tagged.option().is_some() {
                return Some(Err(shape(
                    path,
                    Some(RdTag::Item),
                    RdShapeErrorKind::UnexpectedOption,
                )));
            }
            if !tagged.children().is_empty() {
                // Delimiter markers must have no children; this deliberately
                // reports Exactly(0) while retaining the marker boundary.
                return Some(Err(shape(
                    path,
                    Some(RdTag::Item),
                    RdShapeErrorKind::WrongArity {
                        expected: RdArity::Exactly(0),
                        actual: tagged.children().len(),
                    },
                )));
            }
            let body_ref = self
                .list
                .children_ref()
                .slice(body_start..self.index)
                .expect("validated delimited item body range");
            return Some(Ok(RdListItem::Delimited(RdDelimitedItem {
                path,
                body,
                body_ref,
            })));
        }
        None
    }
}

pub(super) fn inspect_two_group_item<'a>(
    tagged: &'a RdTagged,
    item_path: &RdAstPath,
) -> Result<(&'a [RdNode], &'a [RdNode]), RdShapeError> {
    if tagged.option().is_some() {
        return Err(shape(
            item_path.clone(),
            Some(RdTag::Item),
            RdShapeErrorKind::UnexpectedOption,
        ));
    }
    if tagged.children().len() != 2 {
        return Err(shape(
            item_path.clone(),
            Some(RdTag::Item),
            RdShapeErrorKind::WrongArity {
                expected: RdArity::Exactly(2),
                actual: tagged.children().len(),
            },
        ));
    }
    let [label, body] = tagged.children() else {
        unreachable!()
    };
    for (index, child) in tagged.children().iter().enumerate() {
        if child.as_group().is_none() {
            return Err(shape(
                item_path.with_child(index),
                Some(RdTag::Item),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(child),
                },
            ));
        }
    }
    Ok((
        label.as_group().unwrap().children(),
        body.as_group().unwrap().children(),
    ))
}
