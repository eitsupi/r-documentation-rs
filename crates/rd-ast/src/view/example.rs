use super::*;

/// The source-level spelling of an example-control wrapper.
///
/// This classifies spelling only and does not prescribe execution, visibility,
/// testing, or output-comparison behavior. `\dontshow` and `\testonly` are
/// deliberately distinct variants even though R treats them as synonyms:
/// views are a shape/spelling layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RdExampleControlKind {
    DontRun,
    DontTest,
    DontShow,
    DontDiff,
    TestOnly,
}

/// A borrowed, structurally valid example-control wrapper view.
#[derive(Debug, Clone, PartialEq)]
pub struct RdExampleControl<'a> {
    path: RdPath,
    kind: RdExampleControlKind,
    body: &'a [RdNode],
}

impl<'a> RdExampleControl<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }

    pub fn kind(&self) -> RdExampleControlKind {
        self.kind
    }

    /// Returns all direct children of the wrapper exactly as stored: no clone,
    /// flattening, leaf filtering, or text concatenation is performed.
    /// Consumers own all policy about the body.
    pub fn body(&self) -> &'a [RdNode] {
        self.body
    }
}

fn example_control_kind(tag: &RdTag) -> Option<RdExampleControlKind> {
    match tag {
        RdTag::DontRun => Some(RdExampleControlKind::DontRun),
        RdTag::DontTest => Some(RdExampleControlKind::DontTest),
        RdTag::DontShow => Some(RdExampleControlKind::DontShow),
        RdTag::DontDiff => Some(RdExampleControlKind::DontDiff),
        RdTag::TestOnly => Some(RdExampleControlKind::TestOnly),
        _ => None,
    }
}

impl RdNode {
    /// Lossily views a canonical example-control wrapper without an option.
    pub fn example_control(&self, base_path: &RdPath) -> Option<RdExampleControl<'_>> {
        let tagged = self.as_tagged()?;
        let kind = example_control_kind(tagged.tag())?;
        tagged.option().is_none().then(|| RdExampleControl {
            path: base_path.clone(),
            kind,
            body: tagged.children(),
        })
    }

    /// Strictly inspects a canonical example-control wrapper without changing
    /// or validating any of its direct children.
    pub fn inspect_example_control(
        &self,
        base_path: &RdPath,
    ) -> Result<Option<RdExampleControl<'_>>, RdShapeError> {
        match self {
            RdNode::Tagged(tagged) => {
                let Some(kind) = example_control_kind(tagged.tag()) else {
                    return Ok(None);
                };
                if tagged.option().is_some() {
                    return Err(shape(
                        base_path.clone(),
                        Some(tagged.tag().clone()),
                        RdShapeErrorKind::UnexpectedOption,
                    ));
                }
                Ok(Some(RdExampleControl {
                    path: base_path.clone(),
                    kind,
                    body: tagged.children(),
                }))
            }
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if example_control_kind(&tag).is_none() {
                    return Ok(None);
                }
                Err(shape(
                    base_path.clone(),
                    Some(tag),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                ))
            }
            _ => Ok(None),
        }
    }
}
