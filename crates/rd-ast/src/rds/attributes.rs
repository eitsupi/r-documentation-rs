use super::raw::lower_raw_object;
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AttributeDisposition {
    Consumed,
    Discard,
    Reject,
}
pub(super) fn classify_attribute(
    context: &NodeContext<'_>,
    attribute: &Attribute,
) -> AttributeDisposition {
    match attribute.name().as_str() {
        "Rd_tag"
            if context
                .attributes
                .iter()
                .filter(|a| a.name().as_str() == "Rd_tag")
                .count()
                == 1
                && valid_rd_tag_value(attribute.value()) =>
        {
            AttributeDisposition::Consumed
        }
        "Rd_tag" => AttributeDisposition::Reject,
        "Rd_option" => AttributeDisposition::Consumed,
        "srcref" => AttributeDisposition::Discard,
        "dynamicFlag" if valid_dynamic_flag(context, attribute) => AttributeDisposition::Discard,
        "class" if valid_expanded_fragment_class(context, attribute) => {
            AttributeDisposition::Discard
        }
        "class" if valid_list_class(context, attribute) => AttributeDisposition::Discard,
        "Rdfile" if valid_expanded_fragment_rdfile(context, attribute) => {
            AttributeDisposition::Discard
        }
        _ => AttributeDisposition::Reject,
    }
}

pub(super) fn valid_rd_tag_value(value: &RObject) -> bool {
    value.attributes().is_empty() && is_rd_tag_shape_valid(value.value())
}

pub(super) fn is_rd_tag_shape_valid(value: &RValue) -> bool {
    let RValue::Character(values) = value else {
        return false;
    };
    let [value] = values.as_slice() else {
        return false;
    };
    value.as_str().is_some()
}

pub(super) fn valid_dynamic_flag(context: &NodeContext<'_>, attribute: &Attribute) -> bool {
    matches!(
        (&context.value, attribute.value().value()),
        (RValue::List(_), RValue::Integer(values))
            if values.len() == 1
                && values[0].is_some()
                && attribute.value().attributes().is_empty()
    )
}

pub(super) fn valid_list_class(context: &NodeContext<'_>, attribute: &Attribute) -> bool {
    if context.tag != Some("LIST") || !matches!(context.value, RValue::List(_)) {
        return false;
    }

    attribute.value().attributes().is_empty()
        && is_character_scalar_equal(attribute.value().value(), "Rd")
}

pub(super) fn valid_expanded_fragment_class(
    context: &NodeContext<'_>,
    attribute: &Attribute,
) -> bool {
    matches!(context.tag, Some(r"\ifelse" | r"\href"))
        && matches!(context.value, RValue::List(_))
        && context.attributes.get("Rdfile").is_some()
        && attribute.value().attributes().is_empty()
        && is_character_scalar_equal(attribute.value().value(), "Rd")
}

pub(super) fn valid_expanded_fragment_rdfile(
    context: &NodeContext<'_>,
    attribute: &Attribute,
) -> bool {
    matches!(context.tag, Some("\\ifelse" | "\\href"))
        && matches!(context.value, RValue::List(_))
        && context
            .attributes
            .get("class")
            .is_some_and(valid_expanded_fragment_class_value)
        && attribute.value().attributes().is_empty()
        && is_valid_character_scalar(attribute.value().value())
}

pub(super) fn valid_expanded_fragment_class_value(class: &RObject) -> bool {
    class.attributes().is_empty() && is_character_scalar_equal(class.value(), "Rd")
}

pub(super) fn is_valid_character_scalar(value: &RValue) -> bool {
    let RValue::Character(values) = value else {
        return false;
    };
    let [value] = values.as_slice() else {
        return false;
    };
    matches!(value.as_str(), Some(Ok(_)))
}

pub(super) fn is_character_scalar_equal(value: &RValue, expected: &str) -> bool {
    let RValue::Character(values) = value else {
        return false;
    };
    let [value] = values.as_slice() else {
        return false;
    };
    matches!(value.as_str(), Some(Ok(value)) if value.as_ref() == expected)
}
pub(super) fn lower_attributes(
    context: &mut LowerContext,
    object: &RObject,
    tag: Option<&str>,
) -> Result<Vec<RdAttribute>, LowerError> {
    let mut attributes = Vec::new();

    for attribute in object.attributes().iter() {
        let name = attribute.name().as_str();
        if name == "Rd_option"
            || (name == "Rd_tag"
                && object
                    .attributes()
                    .iter()
                    .filter(|candidate| candidate.name().as_str() == "Rd_tag")
                    .count()
                    == 1
                && valid_rd_tag_value(attribute.value()))
        {
            continue;
        }

        attributes.push(
            context.scoped(RdPathSegment::Attribute(name.to_string()), |context| {
                lower_attribute(context, tag, attribute)
            })?,
        );
    }

    Ok(attributes)
}

pub(super) fn lower_attribute(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: &Attribute,
) -> Result<RdAttribute, LowerError> {
    Ok(producer::raw_attribute(
        attribute.name().as_str().to_string(),
        context.scoped(RdPathSegment::AttributeValue, |context| {
            lower_raw_object(
                context,
                tag,
                Some(attribute.name().as_str()),
                attribute.value(),
            )
        })?,
    ))
}
