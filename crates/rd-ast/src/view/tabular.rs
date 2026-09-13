use super::*;

/// Alignment of one column in a `\\tabular` column specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdColumnAlign {
    Left,
    Center,
    Right,
}

/// A borrowed, structurally valid `\\tabular` view.
///
/// The colspec path is `path().with_child(0)`. Cell paths are anchored to the
/// body Group: a non-empty cell uses the body child index of its first node,
/// while an empty cell uses the index of the separator that closed it. A row
/// uses `anchor_path()` for its first content or boundary node and
/// `nodes_ref()` for its complete body range; an entirely empty row uses the
/// separator that opened its row region. Colspec characters accept only `l`, `c`, and
/// `r`; whitespace and every other character are reported and skipped.
#[derive(Debug, Clone, PartialEq)]
pub struct RdTabular<'a> {
    path: RdAstPath,
    colspec: &'a [RdNode],
    body: &'a [RdNode],
    columns: Vec<RdColumnAlign>,
    rows: Vec<RdTableRow<'a>>,
    diagnostics: Vec<RdShapeError>,
}

impl<'a> RdTabular<'a> {
    pub fn path(&self) -> &RdAstPath {
        &self.path
    }
    pub fn columns(&self) -> &[RdColumnAlign] {
        &self.columns
    }
    /// Returns the column specification as a positioned sibling sequence.
    pub fn colspec_ref(&self) -> RdNodesRef<'a> {
        RdNodesRef::from_slice(self.colspec, self.path.with_child(0))
    }
    /// Returns the table body as a positioned sibling sequence.
    pub fn body_ref(&self) -> RdNodesRef<'a> {
        RdNodesRef::from_slice(self.body, self.path.with_child(1))
    }
    pub fn rows(&self) -> &[RdTableRow<'a>] {
        &self.rows
    }
    pub fn diagnostics(&self) -> &[RdShapeError] {
        &self.diagnostics
    }
}

/// One row of a borrowed `\\tabular` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdTableRow<'a> {
    anchor_path: RdAstPath,
    nodes: RdNodesRef<'a>,
    cells: Vec<RdTableCell<'a>>,
}

impl<'a> RdTableRow<'a> {
    /// Returns the row's diagnostic anchor. This is the first content or
    /// boundary node, and does not describe the complete row range.
    pub fn anchor_path(&self) -> &RdAstPath {
        &self.anchor_path
    }
    /// Returns the row's body sequence, including internal tab separators and
    /// excluding its terminating carriage-return separator.
    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.nodes.clone()
    }
    pub fn cells(&self) -> &[RdTableCell<'a>] {
        &self.cells
    }
}

/// One cell of a borrowed `\\tabular` view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdTableCell<'a> {
    anchor_path: RdAstPath,
    nodes: RdNodesRef<'a>,
}

impl<'a> RdTableCell<'a> {
    /// Returns the cell's diagnostic anchor. Empty cells use their real
    /// separator or row-boundary node as the anchor.
    pub fn anchor_path(&self) -> &RdAstPath {
        &self.anchor_path
    }
    pub fn nodes(&self) -> &'a [RdNode] {
        self.nodes.as_slice()
    }
    /// Returns the cell's source nodes with their body-container indices.
    pub fn nodes_ref(&self) -> RdNodesRef<'a> {
        self.nodes.clone()
    }
}

impl RdTagged {
    /// Strictly inspects a `\\tabular` container. Container-shape failures
    /// are returned atomically; malformed separators, column-specification
    /// characters, and row widths are retained as diagnostics on the view.
    /// A terminal `\\tab` does not create a trailing empty cell, matching
    /// R's `Rd2HTML` rendering.
    pub(crate) fn inspect_tabular<'a>(
        &'a self,
        base_path: &RdAstPath,
    ) -> Result<RdTabular<'a>, RdShapeError> {
        if self.tag() != &RdTag::Tabular {
            return Err(shape(
                base_path.clone(),
                Some(self.tag().clone()),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Tabular,
                    actual: RdNodeKind::Tagged,
                },
            ));
        }
        if self.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Tabular),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        if self.children().len() != 2 {
            return Err(shape(
                base_path.clone(),
                Some(RdTag::Tabular),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(2),
                    actual: self.children().len(),
                },
            ));
        }
        let [colspec_node, body_node] = self.children() else {
            unreachable!()
        };
        let colspec_path = base_path.with_child(0);
        let colspec_group = colspec_node.as_group().ok_or_else(|| {
            shape(
                colspec_path.clone(),
                Some(RdTag::Tabular),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(colspec_node),
                },
            )
        })?;
        let body_path = base_path.with_child(1);
        let body_group = body_node.as_group().ok_or_else(|| {
            shape(
                body_path.clone(),
                Some(RdTag::Tabular),
                RdShapeErrorKind::UnexpectedNode {
                    expected: RdExpectedNode::Group,
                    actual: RdNodeKind::of(body_node),
                },
            )
        })?;

        let colspec_children = colspec_group.children();
        if colspec_children.len() != 1 {
            return Err(shape(
                colspec_path,
                Some(RdTag::Tabular),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(1),
                    actual: colspec_children.len(),
                },
            ));
        }
        let RdNode::Text(spec) = &colspec_children[0] else {
            return Err(shape(
                base_path.with_child(0).with_child(0),
                Some(RdTag::Tabular),
                RdShapeErrorKind::UnexpectedContent {
                    actual: RdNodeKind::of(&colspec_children[0]),
                },
            ));
        };

        let mut columns = Vec::new();
        let mut diagnostics = Vec::new();
        let spec_path = base_path.with_child(0).with_child(0);
        for (start, character) in spec.char_indices() {
            let end = start + character.len_utf8();
            let alignment = match character {
                'l' => Some(RdColumnAlign::Left),
                'c' => Some(RdColumnAlign::Center),
                'r' => Some(RdColumnAlign::Right),
                _ => {
                    diagnostics.push(RdShapeError::with_leaf_byte_range(
                        spec_path.clone(),
                        Some(RdTag::Tabular),
                        RdShapeErrorKind::InvalidValue {
                            construct: RdConstruct::ColumnSpec,
                            value: character.to_string(),
                        },
                        start..end,
                    ));
                    None
                }
            };
            if let Some(alignment) = alignment {
                columns.push(alignment);
            }
        }

        let body = body_group.children();
        let mut rows = Vec::new();
        let mut current_cells = Vec::new();
        let mut cell_start = 0;
        let mut row_start = 0;
        let mut row_anchor = body_path.clone();
        let mut row_has_content = false;

        for (index, node) in body.iter().enumerate() {
            let separator = node.as_tagged().and_then(|tagged| {
                if tagged.tag() == &RdTag::Tab {
                    Some(RdTag::Tab)
                } else if tagged.tag() == &RdTag::Cr {
                    Some(RdTag::Cr)
                } else {
                    None
                }
            });
            let Some(separator) = separator else {
                if !row_has_content && current_cells.is_empty() {
                    row_anchor = body_path.with_child(index);
                }
                row_has_content = true;
                continue;
            };
            let tagged = node.as_tagged().unwrap();
            let separator_path = body_path.with_child(index);
            if !row_has_content && current_cells.is_empty() {
                row_anchor = separator_path.clone();
            }
            if tagged.option().is_some() {
                diagnostics.push(shape(
                    separator_path.clone(),
                    Some(separator.clone()),
                    RdShapeErrorKind::UnexpectedOption,
                ));
            }
            if !tagged.children().is_empty() {
                diagnostics.push(shape(
                    separator_path.clone(),
                    Some(separator.clone()),
                    RdShapeErrorKind::WrongArity {
                        expected: RdArity::Exactly(0),
                        actual: tagged.children().len(),
                    },
                ));
            }
            let cell_path = if cell_start < index {
                body_path.with_child(cell_start)
            } else {
                separator_path.clone()
            };
            current_cells.push(RdTableCell {
                anchor_path: cell_path,
                nodes: RdNodesRef::from_slice_at(
                    &body[cell_start..index],
                    body_path.clone(),
                    cell_start,
                ),
            });
            if separator == RdTag::Cr {
                finish_table_row(
                    &mut rows,
                    &mut current_cells,
                    &mut diagnostics,
                    row_anchor.clone(),
                    RowRegion {
                        body,
                        body_path: body_path.clone(),
                        start: row_start,
                        end: index,
                    },
                    columns.len(),
                );
                cell_start = index + 1;
                row_start = index + 1;
                row_anchor = body_path.with_child(index);
                row_has_content = false;
            } else {
                cell_start = index + 1;
            }
        }
        if cell_start < body.len() {
            let cell_path = body_path.with_child(cell_start);
            current_cells.push(RdTableCell {
                anchor_path: cell_path.clone(),
                nodes: RdNodesRef::from_slice_at(
                    &body[cell_start..],
                    body_path.clone(),
                    cell_start,
                ),
            });
            if !row_has_content {
                row_anchor = cell_path.clone();
            }
            finish_table_row(
                &mut rows,
                &mut current_cells,
                &mut diagnostics,
                row_anchor,
                RowRegion {
                    body,
                    body_path,
                    start: row_start,
                    end: body.len(),
                },
                columns.len(),
            );
        } else if !current_cells.is_empty() {
            finish_table_row(
                &mut rows,
                &mut current_cells,
                &mut diagnostics,
                row_anchor,
                RowRegion {
                    body,
                    body_path,
                    start: row_start,
                    end: body.len(),
                },
                columns.len(),
            );
        }

        Ok(RdTabular {
            path: base_path.clone(),
            colspec: colspec_group.children(),
            body,
            columns,
            rows,
            diagnostics,
        })
    }
}
struct RowRegion<'a> {
    body: &'a [RdNode],
    body_path: RdAstPath,
    start: usize,
    end: usize,
}

fn finish_table_row<'a>(
    rows: &mut Vec<RdTableRow<'a>>,
    cells: &mut Vec<RdTableCell<'a>>,
    diagnostics: &mut Vec<RdShapeError>,
    anchor_path: RdAstPath,
    region: RowRegion<'a>,
    expected_columns: usize,
) {
    if cells.len() != expected_columns {
        diagnostics.push(shape(
            anchor_path.clone(),
            Some(RdTag::Cr),
            RdShapeErrorKind::CountMismatch {
                construct: RdConstruct::TableRow,
                expected: expected_columns,
                actual: cells.len(),
            },
        ));
    }
    rows.push(RdTableRow {
        anchor_path,
        nodes: RdNodesRef::from_slice_at(
            &region.body[region.start..region.end],
            region.body_path,
            region.start,
        ),
        cells: std::mem::take(cells),
    });
}
