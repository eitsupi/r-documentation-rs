use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct RdEnc<'a> {
    path: RdPath,
    encoded: &'a [RdNode],
    ascii: &'a [RdNode],
}

impl<'a> RdEnc<'a> {
    pub fn path(&self) -> &RdPath {
        &self.path
    }
    /// Encoding-dependent Latex-mode markup; encoding-side selection is consumer policy.
    pub fn encoded(&self) -> &'a [RdNode] {
        self.encoded
    }
    /// Encoding-dependent Latex-mode markup; encoding-side selection is consumer policy.
    pub fn ascii(&self) -> &'a [RdNode] {
        self.ascii
    }
}

impl RdNode {
    pub fn enc(&self, base_path: &RdPath) -> Option<RdEnc<'_>> {
        self.inspect_enc(base_path).ok().flatten()
    }

    pub fn inspect_enc(&self, base_path: &RdPath) -> Result<Option<RdEnc<'_>>, RdShapeError> {
        let tagged = match self {
            RdNode::Tagged(tagged) => tagged,
            RdNode::Raw(raw) => {
                let Some(tag) = raw.tag().map(RdTag::from_rd_tag) else {
                    return Ok(None);
                };
                if tag != RdTag::Enc {
                    return Ok(None);
                }
                return Err(shape(
                    base_path.clone(),
                    Some(tag),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Tagged,
                        actual: RdNodeKind::Raw,
                    },
                ));
            }
            _ => return Ok(None),
        };
        if tagged.tag() != &RdTag::Enc {
            return Ok(None);
        }
        if tagged.option().is_some() {
            return Err(shape(
                base_path.clone(),
                Some(tagged.tag().clone()),
                RdShapeErrorKind::UnexpectedOption,
            ));
        }
        let children = tagged.children();
        if children.len() != 2 {
            return Err(shape(
                base_path.clone(),
                Some(tagged.tag().clone()),
                RdShapeErrorKind::WrongArity {
                    expected: RdArity::Exactly(2),
                    actual: children.len(),
                },
            ));
        }
        for (index, child) in children.iter().enumerate() {
            if child.as_group().is_none() {
                return Err(shape(
                    base_path.with_child(index),
                    Some(tagged.tag().clone()),
                    RdShapeErrorKind::UnexpectedNode {
                        expected: RdExpectedNode::Group,
                        actual: RdNodeKind::of(child),
                    },
                ));
            }
        }
        Ok(Some(RdEnc {
            path: base_path.clone(),
            encoded: children[0].as_group().unwrap().children(),
            ascii: children[1].as_group().unwrap().children(),
        }))
    }
}
