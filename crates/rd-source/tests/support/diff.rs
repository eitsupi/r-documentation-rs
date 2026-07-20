use rd_ast::{RdDocument, RdNode, RdNodeKind, RdPath, RdPathSegment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionPresence {
    Absent,
    PresentEmpty,
    PresentNonEmpty,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffKind {
    MissingNode,
    UnexpectedNode,
    NodeKindMismatch {
        expected: RdNodeKind,
        actual: RdNodeKind,
    },
    TagMismatch {
        expected: String,
        actual: String,
    },
    LeafTextMismatch {
        leaf_kind: RdNodeKind,
        expected: String,
        actual: String,
    },
    OptionPresenceMismatch {
        expected: OptionPresence,
        actual: OptionPresence,
    },
    ChildCountMismatch {
        expected: usize,
        actual: usize,
    },
    RawNodeNotComparable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub path: RdPath,
    pub kind: DiffKind,
}

fn escaped(s: &str) -> String {
    s.escape_debug().to_string()
}
fn kind(n: &RdNode) -> RdNodeKind {
    RdNodeKind::of(n)
}
fn option(n: &rd_ast::RdTagged) -> OptionPresence {
    match n.option() {
        None => OptionPresence::Absent,
        Some([]) => OptionPresence::PresentEmpty,
        Some(_) => OptionPresence::PresentNonEmpty,
    }
}
fn raw_option(n: &rd_ast::RawRdNode) -> OptionPresence {
    match n.option() {
        None => OptionPresence::Absent,
        Some([]) => OptionPresence::PresentEmpty,
        Some(_) => OptionPresence::PresentNonEmpty,
    }
}

pub fn compare(expected: &RdDocument, actual: &RdDocument) -> Vec<Difference> {
    let mut out = Vec::new();
    if expected.nodes().len() != actual.nodes().len() {
        out.push(Difference {
            path: RdPath::new(Vec::new()),
            kind: DiffKind::ChildCountMismatch {
                expected: expected.nodes().len(),
                actual: actual.nodes().len(),
            },
        });
    }
    let common = expected.nodes().len().min(actual.nodes().len());
    for i in 0..common {
        compare_node(
            &expected.nodes()[i],
            &actual.nodes()[i],
            RdPath::new(vec![RdPathSegment::TopLevel(i)]),
            &mut out,
        );
    }
    for i in common..expected.nodes().len() {
        out.push(Difference {
            path: RdPath::new(vec![RdPathSegment::TopLevel(i)]),
            kind: DiffKind::MissingNode,
        });
    }
    for i in common..actual.nodes().len() {
        out.push(Difference {
            path: RdPath::new(vec![RdPathSegment::TopLevel(i)]),
            kind: DiffKind::UnexpectedNode,
        });
    }
    out
}

fn compare_node(expected: &RdNode, actual: &RdNode, path: RdPath, out: &mut Vec<Difference>) {
    if kind(expected) != kind(actual) {
        out.push(Difference {
            path,
            kind: DiffKind::NodeKindMismatch {
                expected: kind(expected),
                actual: kind(actual),
            },
        });
        return;
    }
    match (expected, actual) {
        (RdNode::Text(a), RdNode::Text(b))
        | (RdNode::RCode(a), RdNode::RCode(b))
        | (RdNode::Verb(a), RdNode::Verb(b))
        | (RdNode::Comment(a), RdNode::Comment(b)) => {
            if a != b {
                out.push(Difference {
                    path,
                    kind: DiffKind::LeafTextMismatch {
                        leaf_kind: kind(expected),
                        expected: escaped(a),
                        actual: escaped(b),
                    },
                });
            }
        }
        (RdNode::Tagged(a), RdNode::Tagged(b)) => {
            if a.tag() != b.tag() {
                out.push(Difference {
                    path: path.clone(),
                    kind: DiffKind::TagMismatch {
                        expected: a.tag().as_rd_tag().to_string(),
                        actual: b.tag().as_rd_tag().to_string(),
                    },
                });
            }
            if option(a) != option(b) {
                out.push(Difference {
                    path: path.with_option(),
                    kind: DiffKind::OptionPresenceMismatch {
                        expected: option(a),
                        actual: option(b),
                    },
                });
            }
            compare_children(a.children(), b.children(), path.clone(), out);
            if let (Some(x), Some(y)) = (a.option(), b.option()) {
                compare_children(x, y, path.with_option(), out);
            }
        }
        (RdNode::Group(a), RdNode::Group(b)) => {
            compare_children(a.children(), b.children(), path, out)
        }
        (RdNode::Raw(a), RdNode::Raw(b)) => {
            if a.tag() != b.tag()
                || a.payload() != b.payload()
                || a.children() != b.children()
                || a.attributes() != b.attributes()
                || raw_option(a) != raw_option(b)
                || a.option() != b.option()
            {
                out.push(Difference {
                    path,
                    kind: DiffKind::RawNodeNotComparable,
                });
            }
        }
        _ => out.push(Difference {
            path,
            kind: DiffKind::RawNodeNotComparable,
        }),
    }
}

fn compare_children(
    expected: &[RdNode],
    actual: &[RdNode],
    path: RdPath,
    out: &mut Vec<Difference>,
) {
    if expected.len() != actual.len() {
        out.push(Difference {
            path: path.clone(),
            kind: DiffKind::ChildCountMismatch {
                expected: expected.len(),
                actual: actual.len(),
            },
        });
    }
    let common = expected.len().min(actual.len());
    for i in 0..common {
        compare_node(&expected[i], &actual[i], path.with_child(i), out);
    }
    for i in common..expected.len() {
        out.push(Difference {
            path: path.with_child(i),
            kind: DiffKind::MissingNode,
        });
    }
    for i in common..actual.len() {
        out.push(Difference {
            path: path.with_child(i),
            kind: DiffKind::UnexpectedNode,
        });
    }
}

impl std::fmt::Display for Difference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {:?}", self.path, self.kind)
    }
}
