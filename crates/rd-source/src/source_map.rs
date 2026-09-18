use crate::{SourcePosition, SourceSpan};
use rd_ast::{RdAstPath, RdAstPathSegment};
use std::ops::Range;

/// Parser-local source coverage for one canonical AST node.
///
/// This deliberately mirrors the canonical tree rather than storing paths at
/// construction time.  Parser recovery and argument flattening can therefore
/// move a node without leaving stale producer-local entries behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceExtentNode {
    pub(crate) bytes: Range<usize>,
    pub(crate) option: Option<SourceExtentSequence>,
    pub(crate) children: Vec<SourceExtentNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceExtentSequence {
    pub(crate) bytes: Range<usize>,
    pub(crate) nodes: Vec<SourceExtentNode>,
}

impl SourceExtentNode {
    pub(crate) fn leaf(bytes: Range<usize>) -> Self {
        Self {
            bytes,
            option: None,
            children: Vec::new(),
        }
    }

    pub(crate) fn with_children(
        bytes: Range<usize>,
        option: Option<SourceExtentSequence>,
        children: Option<Vec<SourceExtentNode>>,
    ) -> Self {
        Self {
            bytes,
            option,
            children: children.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceExtents {
    pub(crate) root: Range<usize>,
    pub(crate) top_level: Vec<SourceExtentNode>,
}

impl SourceExtents {
    #[allow(dead_code)]
    pub(crate) fn entry_count(&self) -> usize {
        1 + self
            .top_level
            .iter()
            .map(SourceExtentNode::entry_count)
            .sum::<usize>()
    }

    #[allow(dead_code)]
    pub(crate) fn at(&self, path: &RdAstPath) -> Option<&Range<usize>> {
        if path.segments().is_empty() {
            return Some(&self.root);
        }
        let [RdAstPathSegment::TopLevel(index), rest @ ..] = path.segments() else {
            return None;
        };
        let mut node = self.top_level.get(*index)?;
        let mut option_nodes: Option<&[SourceExtentNode]> = None;
        for (position, segment) in rest.iter().enumerate() {
            match segment {
                RdAstPathSegment::Child(index) => {
                    node = match option_nodes.take() {
                        Some(nodes) => nodes.get(*index),
                        None => node.children.get(*index),
                    }?;
                }
                RdAstPathSegment::Option => {
                    if option_nodes.is_some() {
                        return None;
                    }
                    let option = node.option.as_ref()?;
                    if position + 1 == rest.len() {
                        return Some(&option.bytes);
                    }
                    option_nodes = Some(&option.nodes);
                }
                RdAstPathSegment::TopLevel(_) => return None,
                _ => return None,
            }
        }
        if option_nodes.is_some() {
            return None;
        }
        Some(&node.bytes)
    }
}

impl SourceExtentNode {
    fn entry_count(&self) -> usize {
        1 + self.option.as_ref().map_or(0, |option| {
            1 + option
                .nodes
                .iter()
                .map(SourceExtentNode::entry_count)
                .sum::<usize>()
        }) + self
            .children
            .iter()
            .map(SourceExtentNode::entry_count)
            .sum::<usize>()
    }
}

pub(crate) struct SourceMap {
    newlines: Vec<usize>,
    input: String,
}
impl SourceMap {
    pub(crate) fn new(input: &str) -> Self {
        let mut newlines = Vec::new();
        let bytes = input.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'\n' || (bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n')) {
                newlines.push(i);
            }
            i += 1;
        }
        Self {
            newlines,
            input: input.into(),
        }
    }
    pub(crate) fn span(&self, range: std::ops::Range<usize>) -> SourceSpan {
        SourceSpan::new(
            range.clone(),
            self.position(range.start),
            self.position(range.end),
        )
    }
    pub(crate) fn position(&self, offset: usize) -> SourcePosition {
        let line = self.newlines.partition_point(|n| *n < offset);
        let line_start = if line == 0 {
            0
        } else {
            self.newlines[line - 1] + 1
        };
        let end = offset.min(self.input.len());
        let column = self.input[line_start..end].chars().count() as u32 + 1;
        SourcePosition::new(line as u32 + 1, column)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_count_scalars_and_crlf_as_one_line() {
        let map = SourceMap::new("é\r\n日本語");
        assert_eq!(map.position(0), SourcePosition::new(1, 1));
        assert_eq!(map.position("é".len()), SourcePosition::new(1, 2));
        assert_eq!(map.position("é\r\n".len()), SourcePosition::new(2, 1));
        assert_eq!(map.position("é\r\n日本語".len()), SourcePosition::new(2, 4));
        assert_eq!(map.span(0..map.input.len()).bytes(), 0..map.input.len());
    }
}
