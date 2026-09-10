use super::*;

fn closure_bytes() -> Vec<u8> {
    let mut bytes = vec![b'X', b'\n'];
    bytes.extend_from_slice(&[0, 0, 0, 3, 0, 4, 6, 1, 0, 3, 5, 0, 0, 0, 0, 5]);
    bytes.extend_from_slice(b"UTF-8");
    bytes.extend_from_slice(&[0, 0, 4, CLOSXP, 0, 0, 0, wire::EMPTYENV_SXP]);
    bytes.extend_from_slice(&[0, 0, 0, wire::NILSXP]);
    bytes.extend_from_slice(&[0, 0, 0, wire::BCODESXP]);
    bytes
}

fn closure_with_attribute_chain(length: usize) -> Vec<u8> {
    let mut bytes = closure_bytes();
    bytes[25] |= 0x02;

    let mut attributes = vec![0, 0, 0, wire::NILSXP];
    for index in (0..length).rev() {
        let name = format!("attribute_{index}");
        let mut cell = vec![0, 0, 4, wire::LISTSXP];
        cell.extend_from_slice(&[0, 0, 0, wire::SYMSXP]);
        cell.extend_from_slice(&[0, 0, 0, wire::CHARSXP]);
        cell.extend_from_slice(&(name.len() as u32).to_be_bytes());
        cell.extend_from_slice(name.as_bytes());
        cell.extend_from_slice(&[0, 0, 0, wire::NILSXP]);
        cell.extend_from_slice(&attributes);
        attributes = cell;
    }

    bytes.splice(27..27, attributes);
    bytes
}

fn replace_dots_tag_with_reference(bytes: &mut Vec<u8>, reference: u32) {
    let marker = [
        0,
        0,
        0,
        wire::SYMSXP,
        0,
        4,
        0,
        wire::CHARSXP,
        0,
        0,
        0,
        3,
        b'.',
        b'.',
        b'.',
    ];
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("dots formal tag marker");
    bytes.splice(
        start..start + marker.len(),
        [
            0,
            0,
            0,
            wire::REFSXP,
            (reference >> 24) as u8,
            (reference >> 16) as u8,
            (reference >> 8) as u8,
            reference as u8,
        ],
    );
}

#[test]
fn closure_prefix_stops_after_body_tag() {
    let result = inspect_stored_object(&closure_bytes(), InspectionOptions::default()).unwrap();
    assert_eq!(result.kind, SexpKind::Closure);
    assert!(
        matches!(result.formals, FormalsInspection::Available(ref values) if values.is_empty())
    );
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals {
            body_kind: SexpKind::ByteCode,
            ..
        }
    ));
}

#[test]
fn header_limit_is_top_level_failure() {
    let error = inspect_stored_object(
        &closure_bytes(),
        InspectionOptions::default().max_bytes_visited(2),
    )
    .unwrap_err();
    assert_eq!(error.phase, FailurePhase::Root);
    assert_eq!(error.reason, FailureReason::ResourceLimit);
}

#[test]
fn truncated_body_tag_is_retained_as_unavailable() {
    let mut bytes = closure_bytes();
    bytes.truncate(bytes.len() - 1);
    let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::BodyTag,
            reason: FailureReason::Malformed,
            ..
        })
    ));
}

#[test]
fn body_payload_is_not_read_after_a_valid_body_tag() {
    let mut bytes = closure_bytes();
    bytes.extend_from_slice(&[0xff, 0xff, 0xff]);
    let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals { .. }
    ));
    assert!(crate::parse(&bytes).is_err());
}

#[test]
fn prefix_failures_keep_their_phase_and_reason() {
    let mut bytes = closure_bytes();
    bytes[30] = BCODESXP;
    let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Environment,
            reason: FailureReason::Unsupported { .. },
            ..
        })
    ));

    let mut missing_tag = closure_bytes();
    missing_tag[25] = 0;
    let result = inspect_stored_object(&missing_tag, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Environment,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    let result = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds"),
        InspectionOptions::default().limits(Limits::default().max_references(0)),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::ResourceLimit,
            ..
        })
    ));
}

#[test]
fn formal_limit_does_not_return_partial_formals() {
    let result = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds"),
        InspectionOptions::default().max_formals(2),
    )
    .unwrap();
    assert!(matches!(result.formals, FormalsInspection::Unavailable(_)));
}

#[test]
fn depth_limit_tracks_nested_fields_not_formal_ordinals() {
    let result = inspect_stored_object(
        &closure_bytes(),
        InspectionOptions::default().limits(Limits::default().max_depth(0)),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Environment,
            reason: FailureReason::ResourceLimit,
            ..
        })
    ));

    let result = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds"),
        InspectionOptions::default().limits(Limits::default().max_depth(1)),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::ResourceLimit,
            ..
        })
    ));
}

#[test]
fn formal_tags_accept_shared_symbols_and_reject_bad_references() {
    let mut shared = include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
    replace_dots_tag_with_reference(&mut shared, 1);
    let result = inspect_stored_object(&shared, InspectionOptions::default()).unwrap();
    let FormalsInspection::Available(formals) = result.formals else {
        panic!("shared symbol formal should be available");
    };
    assert_eq!(formals[0].name, "alpha");
    assert_eq!(formals[1].name, "alpha");

    let mut invalid = include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
    replace_dots_tag_with_reference(&mut invalid, 99);
    let result = inspect_stored_object(&invalid, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    for environment_code in [wire::ENVSXP, wire::PERSISTSXP] {
        let mut non_symbol =
            include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        non_symbol[30] = environment_code;
        let payload = if environment_code == wire::ENVSXP {
            [0; 20].to_vec()
        } else {
            [0; 8].to_vec()
        };
        non_symbol.splice(31..31, payload);
        replace_dots_tag_with_reference(&mut non_symbol, 1);
        let result = inspect_stored_object(&non_symbol, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Formals,
                reason: FailureReason::Malformed,
                ..
            })
        ));
    }
}

#[test]
fn malformed_prefix_shapes_keep_specific_failure_phases() {
    let mut attributes = closure_bytes();
    attributes[25] = 6;
    attributes.splice(27..27, [0, 0, 0, EXTPTRSXP]);
    let result = inspect_stored_object(&attributes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Attributes,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    let mut missing_attribute_tag = closure_bytes();
    missing_attribute_tag[25] |= 0x02;
    missing_attribute_tag[27..31].copy_from_slice(&[0, 0, 0, wire::LISTSXP]);
    let result =
        inspect_stored_object(&missing_attribute_tag, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Attributes,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    let mut formal_attributes =
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
    let cell = formal_attributes
        .windows(4)
        .position(|window| window == [0, 0, 4, wire::LISTSXP])
        .unwrap();
    formal_attributes[cell + 2] |= 0x02;
    formal_attributes[cell + 7] = EXTPTRSXP;
    let result = inspect_stored_object(&formal_attributes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    let mut non_symbol =
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
    let marker = [
        0,
        0,
        0,
        wire::SYMSXP,
        0,
        4,
        0,
        wire::CHARSXP,
        0,
        0,
        0,
        3,
        b'.',
        b'.',
        b'.',
    ];
    let tag = non_symbol
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap();
    non_symbol[tag + 3] = wire::CHARSXP;
    let result = inspect_stored_object(&non_symbol, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    let mut invalid_cdr =
        include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
    let pair = [0, 0, 4, wire::LISTSXP];
    let positions = invalid_cdr
        .windows(pair.len())
        .enumerate()
        .filter_map(|(index, window)| (window == pair).then_some(index))
        .collect::<Vec<_>>();
    invalid_cdr[positions[1] + 3] = 26;
    let result = inspect_stored_object(&invalid_cdr, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Formals,
            reason: FailureReason::Malformed,
            ..
        })
    ));

    for type_code in [BCODESXP, ALTREP_SXP] {
        let mut default =
            include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").to_vec();
        let missing = [0, 0, 0, wire::MISSINGARG_SXP];
        let offset = default
            .windows(missing.len())
            .position(|window| window == missing)
            .unwrap();
        default[offset + 3] = type_code;
        let result = inspect_stored_object(&default, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: FailurePhase::Default(0),
                reason: FailureReason::Unsupported { .. },
                ..
            })
        ));
    }
}

#[test]
fn attribute_tags_honor_the_nested_depth_limit() {
    let bytes = include_bytes!("../../tests/fixtures/data/closure_attributes_s4_v3.rds");
    let name_offset = bytes
        .windows(b"inspection_s4".len())
        .position(|window| window == b"inspection_s4")
        .expect("S4 attribute name");
    let tag_offset = name_offset - 12;
    let result = inspect_stored_object(
        bytes,
        InspectionOptions::default().limits(Limits::default().max_depth(1)),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Attributes,
            offset,
            reason: FailureReason::ResourceLimit,
        }) if offset == tag_offset
    ));
}

#[test]
fn sibling_attribute_chain_honors_the_nested_depth_limit() {
    let bytes = closure_with_attribute_chain(3);
    let result = inspect_stored_object(
        &bytes,
        InspectionOptions::default().limits(Limits::default().max_depth(2)),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::Attributes,
            reason: FailureReason::ResourceLimit,
            ..
        })
    ));
}

#[test]
fn generated_diagnostic_fixtures_retain_their_failure_phase() {
    let cases = [
        (
            include_bytes!("../../tests/fixtures/data/closure_environment_compiled_v3.rds")
                .as_slice(),
            FailurePhase::Environment,
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_default_compiled_v3.rds").as_slice(),
            FailurePhase::Default(0),
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_default_altrep_v3.rds").as_slice(),
            FailurePhase::Default(0),
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_attributes_compiled_v3.rds")
                .as_slice(),
            FailurePhase::Attributes,
        ),
    ];
    for (bytes, phase) in cases {
        let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.formals,
            FormalsInspection::Unavailable(PrefixFailure {
                phase: actual,
                reason: FailureReason::Unsupported { .. },
                ..
            }) if actual == phase
        ));
    }

    let s4 = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_attributes_s4_v3.rds"),
        InspectionOptions::default(),
    )
    .unwrap();
    assert!(matches!(s4.formals, FormalsInspection::Available(_)));
}

#[test]
fn generated_namespace_environment_preserves_the_prefix() {
    let result = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_namespace_v3.rds"),
        InspectionOptions::default(),
    )
    .unwrap();
    assert!(matches!(result.formals, FormalsInspection::Available(_)));
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals { .. }
    ));
}

#[test]
fn generated_persisted_environment_preserves_the_prefix() {
    let result = inspect_stored_object(
        include_bytes!("../../tests/fixtures/data/closure_environment_persisted_v3.rds"),
        InspectionOptions::default(),
    )
    .unwrap();
    assert!(matches!(result.formals, FormalsInspection::Available(_)));
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals { .. }
    ));
}

#[test]
fn byte_cap_applies_only_when_a_new_read_crosses_it() {
    let mut bytes = closure_bytes();
    let baseline = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
    let InspectionExtent::ThroughFormals { body_offset, .. } = baseline.extent else {
        panic!("closure prefix should reach the body tag");
    };
    bytes.extend(std::iter::repeat_n(0xff, 4096));

    let result = inspect_stored_object(
        &bytes,
        InspectionOptions::default().max_bytes_visited(body_offset + 4),
    )
    .unwrap();
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals { .. }
    ));

    let result = inspect_stored_object(
        &bytes,
        InspectionOptions::default().max_bytes_visited(body_offset + 3),
    )
    .unwrap();
    assert!(matches!(
        result.formals,
        FormalsInspection::Unavailable(PrefixFailure {
            phase: FailurePhase::BodyTag,
            reason: FailureReason::ResourceLimit,
            ..
        })
    ));
}

#[test]
fn unknown_body_tag_is_observed_without_body_validation() {
    let mut bytes = closure_bytes();
    *bytes.last_mut().unwrap() = 200;
    let result = inspect_stored_object(&bytes, InspectionOptions::default()).unwrap();
    assert!(matches!(
        result.extent,
        InspectionExtent::ThroughFormals {
            body_kind: SexpKind::Other(200),
            ..
        }
    ));
    assert!(crate::parse(&bytes).is_err());
}

#[test]
fn generated_closure_fixtures_preserve_formal_order_and_defaults() {
    for (bytes, names) in [
        (
            include_bytes!("../../tests/fixtures/data/closure_formals_v2.rds").as_slice(),
            ["alpha", "...", "not syntactic", "非構文", "omega"],
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_formals_v3.rds").as_slice(),
            ["alpha", "...", "not syntactic", "非構文", "omega"],
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_formals_compiled_v2.rds").as_slice(),
            ["alpha", "...", "not syntactic", "非構文", "omega"],
        ),
        (
            include_bytes!("../../tests/fixtures/data/closure_formals_compiled_v3.rds").as_slice(),
            ["alpha", "...", "not syntactic", "非構文", "omega"],
        ),
    ] {
        let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
        let FormalsInspection::Available(formals) = result.formals else {
            panic!("closure formals should be available");
        };
        assert_eq!(
            formals
                .iter()
                .map(|formal| formal.name.as_str())
                .collect::<Vec<_>>(),
            names
        );
        assert!(matches!(formals[0].default, DefaultPresence::Absent));
        assert!(matches!(formals[1].default, DefaultPresence::Absent));
        assert!(matches!(formals[2].default, DefaultPresence::Present));
        assert!(matches!(formals[3].default, DefaultPresence::Present));
        assert!(matches!(formals[4].default, DefaultPresence::Present));
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals { .. }
        ));
    }

    for bytes in [
        include_bytes!("../../tests/fixtures/data/closure_formals_compiled_v2.rds").as_slice(),
        include_bytes!("../../tests/fixtures/data/closure_formals_compiled_v3.rds").as_slice(),
    ] {
        let result = inspect_stored_object(bytes, InspectionOptions::default()).unwrap();
        assert!(matches!(
            result.extent,
            InspectionExtent::ThroughFormals {
                body_kind: SexpKind::ByteCode,
                ..
            }
        ));
    }
}
