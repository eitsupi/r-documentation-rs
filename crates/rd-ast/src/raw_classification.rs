//! Classification of the lowering's remaining `Raw` exception, pinned
//! against supported RDS-producer output.
//!
//! This is a regression guard for the current RDS lowering, not the final
//! semantic contract for USERMACRO nodes. A future USERMACRO representation
//! must revise this guard together with the lowering change.

use crate::{RawRdNode, RawRdValue, RdNode};

/// The currently accepted and unexpected shapes of an RDS-lowered `Raw` node.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RawNodeClassification {
    /// The empirically observed USERMACRO definition shape across supported
    /// RDS producers (installed help databases and direct `saveRDS` of
    /// `parse_Rd` output): one text child, no option, and the exact
    /// `srcref`/`macro` attribute shapes checked by [`classify_raw_node`].
    ExpectedUserMacroDefinition,
    /// A Raw node outside that pinned exception.
    Unexpected(UnexpectedRawNode),
}

/// Details explaining why a Raw node did not match the pinned shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnexpectedRawNode {
    tag: Option<String>,
    reason: RawNodeReason,
    offending_attributes: Vec<String>,
}

impl UnexpectedRawNode {
    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }
    pub fn reason(&self) -> &RawNodeReason {
        &self.reason
    }
    pub fn offending_attributes(&self) -> &[String] {
        &self.offending_attributes
    }
}

/// Coarse structural reason for an unexpected Raw node.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RawNodeReason {
    WrongTag,
    OptionPresent,
    ChildrenShape,
    PayloadPresent,
    AttributeSet,
    AttributeValueShape,
}

/// Classifies a Raw node against the RDS-producer-backed USERMACRO lowering
/// guard.
pub fn classify_raw_node(raw: &RawRdNode) -> RawNodeClassification {
    let names: Vec<&str> = raw
        .attributes()
        .iter()
        .map(|attribute| attribute.name())
        .collect();
    let unexpected_names: Vec<String> = names
        .iter()
        .filter(|name| **name != "srcref" && **name != "macro")
        .map(|name| (*name).to_string())
        .collect();

    if raw.tag() != Some("USERMACRO") {
        let all_names = names.iter().map(|name| (*name).to_string()).collect();
        return unexpected(raw, RawNodeReason::WrongTag, all_names);
    }
    if raw.option().is_some() {
        let mut offenders = unexpected_names;
        offenders.push("Rd_option".to_string());
        return unexpected(raw, RawNodeReason::OptionPresent, offenders);
    }
    if raw.payload().is_some() {
        return unexpected(raw, RawNodeReason::PayloadPresent, unexpected_names);
    }
    if raw.children().len() != 1 || !matches!(raw.children()[0], RdNode::Text(_)) {
        return unexpected(raw, RawNodeReason::ChildrenShape, unexpected_names);
    }
    if names.len() != 2
        || names.iter().filter(|name| **name == "srcref").count() != 1
        || names.iter().filter(|name| **name == "macro").count() != 1
    {
        let mut offenders = unexpected_names;
        for name in ["srcref", "macro"] {
            let count = names.iter().filter(|candidate| **candidate == name).count();
            if count == 0 {
                offenders.push(format!("missing:{name}"));
            } else if count > 1 {
                offenders.extend(std::iter::repeat_n(name.to_string(), count));
            }
        }
        return unexpected(raw, RawNodeReason::AttributeSet, offenders);
    }

    let srcref = raw
        .attributes()
        .iter()
        .find(|attribute| attribute.name() == "srcref");
    let macro_attribute = raw
        .attributes()
        .iter()
        .find(|attribute| attribute.name() == "macro");
    let valid_srcref = srcref.is_some_and(|attribute| valid_srcref_value(attribute.value()));
    let valid_macro = macro_attribute.is_some_and(|attribute| {
        matches!(attribute.value().value(), RawRdValue::Character(values) if values.len() == 1 && values[0].is_some())
            && attribute.value().attributes().is_empty()
    });
    if !valid_srcref || !valid_macro {
        let malformed = raw
            .attributes()
            .iter()
            .filter(|attribute| {
                (attribute.name() == "srcref" && !valid_srcref)
                    || (attribute.name() == "macro" && !valid_macro)
            })
            .map(|attribute| attribute.name().to_string())
            .collect();
        return unexpected(raw, RawNodeReason::AttributeValueShape, malformed);
    }

    RawNodeClassification::ExpectedUserMacroDefinition
}

fn valid_srcref_value(object: &crate::RawRdObject) -> bool {
    let valid_integer = matches!(object.value(), RawRdValue::Integer(values) if values.len() == 6 && values.iter().all(Option::is_some));
    let attributes = object.attributes();
    if !valid_integer || attributes.len() != 2 {
        return false;
    }
    let srcfile = attributes
        .iter()
        .find(|attribute| attribute.name() == "srcfile");
    let class = attributes
        .iter()
        .find(|attribute| attribute.name() == "class");
    // Installed help databases persist the srcfile environment through a
    // reference hook; a direct saveRDS of parse_Rd output serializes it as an
    // ordinary (non-singleton) environment. Both are supported RDS-producer
    // provenances. Singleton environments never appear as a srcfile. The
    // decoder collapses package and namespace environments into the same
    // Other handle, so this arm nominally admits them too; that synthetic
    // provenance is tolerated because a real srcfile is never one and any
    // system-macro collapse still requires structural definition and
    // expansion validation that runs after this guard.
    srcfile.is_some_and(|attribute| {
        attribute.value().attributes().is_empty()
            && (matches!(attribute.value().value(), RawRdValue::Persisted(values) if values.len() == 1 && values[0].is_some())
                || matches!(
                    attribute.value().value(),
                    RawRdValue::Environment(crate::RawRdEnvironment::Other)
                ))
    }) && class.is_some_and(|attribute| {
        attribute.value().attributes().is_empty()
            && matches!(attribute.value().value(), RawRdValue::Character(values) if values.len() == 1 && values[0].as_deref() == Some("srcref"))
    })
}

fn unexpected(
    raw: &RawRdNode,
    reason: RawNodeReason,
    offending_attributes: Vec<String>,
) -> RawNodeClassification {
    RawNodeClassification::Unexpected(UnexpectedRawNode {
        tag: raw.tag().map(str::to_string),
        reason,
        offending_attributes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RawRdValue, producer};

    fn text() -> RdNode {
        RdNode::Text("body".to_string())
    }
    fn attr(name: &str, value: RawRdValue) -> crate::RdAttribute {
        producer::raw_attribute(name.to_string(), producer::raw_object(value, Vec::new()))
    }
    fn srcref_with(srcfile_value: RawRdValue) -> crate::RdAttribute {
        let srcfile = producer::raw_attribute(
            "srcfile".into(),
            producer::raw_object(srcfile_value, Vec::new()),
        );
        let class = producer::raw_attribute(
            "class".into(),
            producer::raw_object(
                RawRdValue::Character(vec![Some("srcref".into())]),
                Vec::new(),
            ),
        );
        producer::raw_attribute(
            "srcref".into(),
            producer::raw_object(RawRdValue::Integer(vec![Some(1); 6]), vec![srcfile, class]),
        )
    }
    fn srcref() -> crate::RdAttribute {
        srcref_with(RawRdValue::Persisted(vec![Some("env::1".into())]))
    }
    fn usermacro(
        attributes: Vec<crate::RdAttribute>,
        option: Option<Vec<RdNode>>,
        children: Vec<RdNode>,
    ) -> RawRdNode {
        producer::raw_node(Some("USERMACRO".into()), option, children, None, attributes)
    }

    #[test]
    fn accepts_pinned_usermacro_shape() {
        let raw = usermacro(
            vec![
                srcref(),
                attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
            ],
            None,
            vec![text()],
        );
        assert_eq!(
            classify_raw_node(&raw),
            RawNodeClassification::ExpectedUserMacroDefinition
        );
    }

    #[test]
    fn accepts_both_supported_srcfile_provenances() {
        use crate::RawRdEnvironment;
        let environment = usermacro(
            vec![
                srcref_with(RawRdValue::Environment(RawRdEnvironment::Other)),
                attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
            ],
            None,
            vec![text()],
        );
        assert_eq!(
            classify_raw_node(&environment),
            RawNodeClassification::ExpectedUserMacroDefinition
        );
        for singleton in [
            RawRdEnvironment::Global,
            RawRdEnvironment::Base,
            RawRdEnvironment::Empty,
        ] {
            let rejected = usermacro(
                vec![
                    srcref_with(RawRdValue::Environment(singleton)),
                    attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
                ],
                None,
                vec![text()],
            );
            assert!(matches!(
                classify_raw_node(&rejected),
                RawNodeClassification::Unexpected(UnexpectedRawNode {
                    reason: RawNodeReason::AttributeValueShape,
                    ..
                })
            ));
        }
    }

    #[test]
    fn payload_is_rejected_from_pinned_usermacro_shape() {
        let raw = producer::raw_node(
            Some("USERMACRO".into()),
            None,
            vec![text()],
            Some(RawRdValue::Null),
            vec![
                srcref(),
                attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
            ],
        );
        assert!(matches!(
            classify_raw_node(&raw),
            RawNodeClassification::Unexpected(UnexpectedRawNode {
                reason: RawNodeReason::PayloadPresent,
                ..
            })
        ));
    }

    #[test]
    fn rejects_wrong_tag_extra_attribute_wrong_value_and_option() {
        let base = || {
            vec![
                srcref(),
                attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
            ]
        };
        let wrong_tag = producer::raw_node(Some("OTHER".into()), None, vec![text()], None, base());
        assert!(matches!(
            classify_raw_node(&wrong_tag),
            RawNodeClassification::Unexpected(UnexpectedRawNode {
                reason: RawNodeReason::WrongTag,
                ..
            })
        ));
        let extra = usermacro(
            vec![
                srcref(),
                attr("macro", RawRdValue::Character(vec![Some(r"\eval".into())])),
                attr("extra", RawRdValue::Null),
            ],
            None,
            vec![text()],
        );
        assert!(matches!(
            classify_raw_node(&extra),
            RawNodeClassification::Unexpected(UnexpectedRawNode {
                reason: RawNodeReason::AttributeSet,
                ..
            })
        ));
        let wrong_value = usermacro(
            vec![srcref(), attr("macro", RawRdValue::Integer(vec![Some(1)]))],
            None,
            vec![text()],
        );
        assert!(matches!(
            classify_raw_node(&wrong_value),
            RawNodeClassification::Unexpected(UnexpectedRawNode {
                reason: RawNodeReason::AttributeValueShape,
                ..
            })
        ));
        let with_option = usermacro(base(), Some(vec![text()]), vec![text()]);
        let RawNodeClassification::Unexpected(details) = classify_raw_node(&with_option) else {
            unreachable!("option should be unexpected")
        };
        assert!(
            details
                .offending_attributes()
                .contains(&"Rd_option".to_string())
        );
    }

    #[test]
    fn reports_missing_and_duplicate_required_attributes() {
        let missing = usermacro(
            vec![attr("macro", RawRdValue::Character(vec![Some("x".into())]))],
            None,
            vec![text()],
        );
        let RawNodeClassification::Unexpected(details) = classify_raw_node(&missing) else {
            unreachable!("missing attribute should be unexpected")
        };
        assert!(
            details
                .offending_attributes()
                .contains(&"missing:srcref".to_string())
        );

        let duplicate = usermacro(
            vec![
                srcref(),
                attr("macro", RawRdValue::Character(vec![Some("x".into())])),
                attr("macro", RawRdValue::Character(vec![Some("y".into())])),
            ],
            None,
            vec![text()],
        );
        let RawNodeClassification::Unexpected(details) = classify_raw_node(&duplicate) else {
            unreachable!("duplicate attribute should be unexpected")
        };
        assert_eq!(
            details.offending_attributes(),
            &["macro".to_string(), "macro".to_string()]
        );
    }
}
