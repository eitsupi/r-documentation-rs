use super::*;

pub(super) fn lower_leaf(
    context: &mut LowerContext,
    location_tag: Option<&str>,
    attribute: Option<&str>,
    tag: &str,
    value: &RValue,
) -> Result<Option<RdNode>, LowerError> {
    let RValue::Character(_) = value else {
        return Ok(None);
    };

    let text = match tag {
        "TEXT" => Some(RdNode::Text(lower_character_vector(
            context,
            location_tag,
            attribute,
            value,
        )?)),
        "RCODE" => Some(RdNode::RCode(lower_character_vector(
            context,
            location_tag,
            attribute,
            value,
        )?)),
        "VERB" => Some(RdNode::Verb(lower_character_vector(
            context,
            location_tag,
            attribute,
            value,
        )?)),
        "COMMENT" => Some(RdNode::Comment(lower_character_vector(
            context,
            location_tag,
            attribute,
            value,
        )?)),
        _ => None,
    };

    Ok(text)
}

pub(super) fn lower_character_vector(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: Option<&str>,
    value: &RValue,
) -> Result<String, LowerError> {
    let RValue::Character(strings) = value else {
        return Err(LowerError::InvalidStringEncoding {
            location: context.location(tag, attribute),
            encoding: StringEncoding::Native,
        });
    };

    let mut text = String::new();
    for (index, string) in strings.iter().enumerate() {
        let Some(decoded) = string.as_str() else {
            continue;
        };
        let decoded = decoded.map_err(|_| {
            context.scoped(RdPathSegment::CharacterElement(index), |context| {
                context.invalid_string(tag, attribute, string)
            })
        })?;
        text.push_str(decoded.as_ref());
    }
    Ok(text)
}

pub(super) fn rd_tag_string(
    context: &mut LowerContext,
    object: &RObject,
) -> Result<Option<String>, LowerError> {
    if object
        .attributes()
        .iter()
        .filter(|attribute| attribute.name().as_str() == "Rd_tag")
        .count()
        != 1
    {
        return Ok(None);
    }
    let Some(tag) = object.attributes().get("Rd_tag") else {
        return Ok(None);
    };

    if !valid_rd_tag_value(tag) {
        return Ok(None);
    }
    let RValue::Character(values) = tag.value() else {
        unreachable!("valid Rd_tag value must be a character scalar");
    };
    let value = &values[0];
    let Some(decoded) = value.as_str() else {
        return Ok(None);
    };
    Ok(Some(
        context
            .scoped(RdPathSegment::CharacterElement(0), |context| {
                decoded.map_err(|_| context.invalid_string(None, Some("Rd_tag"), value))
            })?
            .into_owned(),
    ))
}
