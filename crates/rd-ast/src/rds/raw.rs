use super::structured::lower_character_vector;
use super::*;

pub(super) fn lower_raw_children(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: Option<&str>,
    value: &RValue,
) -> Result<(Vec<RdNode>, Option<RawRdValue>), LowerError> {
    match value {
        RValue::List(children) => {
            lower_children(context, attribute, children).map(|children| (children, None))
        }
        RValue::Character(_) => {
            if let Some(tag) = tag
                && let Some(leaf) = lower_leaf(context, Some(tag), attribute, tag, value)?
            {
                return Ok((vec![leaf], None));
            }

            Ok((
                vec![RdNode::Text(lower_character_vector(
                    context, tag, attribute, value,
                )?)],
                None,
            ))
        }
        _ => Ok((
            Vec::new(),
            Some(lower_raw_value(context, tag, attribute, value)?),
        )),
    }
}

pub(super) fn lower_raw_object(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: Option<&str>,
    object: &RObject,
) -> Result<RawRdObject, LowerError> {
    let attributes = object
        .attributes()
        .iter()
        .map(|nested| {
            context.scoped(
                RdPathSegment::Attribute(nested.name().as_str().to_string()),
                |context| lower_attribute(context, tag, nested),
            )
        })
        .collect::<Result<_, _>>()?;

    Ok(producer::raw_object(
        lower_raw_value(context, tag, attribute, object.value())?,
        attributes,
    ))
}

pub(super) fn lower_raw_value(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: Option<&str>,
    value: &RValue,
) -> Result<RawRdValue, LowerError> {
    Ok(match value {
        RValue::Null => RawRdValue::Null,
        RValue::Logical(values) => RawRdValue::Logical(values.clone()),
        RValue::Integer(values) => RawRdValue::Integer(values.clone()),
        RValue::Real(values) => {
            RawRdValue::Real(values.iter().copied().map(lower_raw_real).collect())
        }
        RValue::Character(values) => {
            RawRdValue::Character(lower_raw_strings(context, tag, attribute, values)?)
        }
        RValue::List(values) => RawRdValue::List(
            values
                .iter()
                .enumerate()
                .map(|(index, object)| {
                    context.scoped(RdPathSegment::ListElement(index), |context| {
                        lower_raw_object(context, tag, attribute, object)
                    })
                })
                .collect::<Result<_, _>>()?,
        ),
        RValue::Symbol(symbol) => RawRdValue::Symbol(symbol.as_str().to_string()),
        RValue::Persisted(value) => RawRdValue::Persisted(lower_raw_strings(
            context,
            tag,
            attribute,
            value.as_slice(),
        )?),
        RValue::Environment(value) => RawRdValue::Environment(match value {
            EnvHandle::Global => RawRdEnvironment::Global,
            EnvHandle::Base => RawRdEnvironment::Base,
            EnvHandle::Empty => RawRdEnvironment::Empty,
            EnvHandle::Other => RawRdEnvironment::Other,
            _ => RawRdEnvironment::Other,
        }),
        _ => {
            return Err(LowerError::UnsupportedValue {
                location: context.location(tag, attribute),
            });
        }
    })
}

pub(super) fn lower_raw_real(value: Option<f64>) -> RawRdReal {
    match value {
        None => RawRdReal::Na,
        Some(value) if value.is_nan() => RawRdReal::Nan,
        Some(value) if value == f64::INFINITY => RawRdReal::PositiveInfinity,
        Some(value) if value == f64::NEG_INFINITY => RawRdReal::NegativeInfinity,
        Some(value) => RawRdReal::Finite(value),
    }
}

pub(super) fn lower_raw_strings(
    context: &mut LowerContext,
    tag: Option<&str>,
    attribute: Option<&str>,
    values: &[RStr],
) -> Result<Vec<Option<String>>, LowerError> {
    let mut decoded = Vec::with_capacity(values.len());
    for (index, raw_value) in values.iter().enumerate() {
        let Some(value) = raw_value.as_str() else {
            decoded.push(None);
            continue;
        };
        decoded.push(Some(
            value
                .map_err(|_| {
                    context.scoped(RdPathSegment::CharacterElement(index), |context| {
                        context.invalid_string(tag, attribute, raw_value)
                    })
                })?
                .into_owned(),
        ));
    }
    Ok(decoded)
}
