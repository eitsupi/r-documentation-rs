use rd_rds::inspection::{
    self, BodyValidation, DefaultPresence, FailureCause, FailurePhase, FormalsInspection,
    FormalsNotApplicable, FormalsUnavailable, InspectionExtent, InspectionOptions, StoredKind,
};
use rd_rds::{ByteCursor, Header, Limits};

const CLOSURE: &[u8] = include_bytes!("fixtures/data/closure_formals_v3.rds");

#[test]
fn inspects_r_generated_closures_without_file_or_codec_features() {
    for bytes in [
        include_bytes!("fixtures/data/closure_formals_v2.rds").as_slice(),
        CLOSURE,
        include_bytes!("fixtures/data/closure_formals_compiled_v2.rds").as_slice(),
        include_bytes!("fixtures/data/closure_formals_compiled_v3.rds").as_slice(),
    ] {
        let result = inspection::inspect(bytes).unwrap();
        assert_eq!(result.kind(), StoredKind::Closure);
        let FormalsInspection::Available(formals) = result.formals() else {
            panic!("expected closure formals: {result:?}");
        };
        assert_eq!(
            formals
                .iter()
                .map(|formal| formal.name())
                .collect::<Vec<_>>(),
            ["alpha", "...", "not syntactic", "非構文", "omega"]
        );
        assert_eq!(
            formals
                .iter()
                .map(|formal| formal.default())
                .collect::<Vec<_>>(),
            [
                DefaultPresence::Absent,
                DefaultPresence::Absent,
                DefaultPresence::Present,
                DefaultPresence::Present,
                DefaultPresence::Present
            ]
        );
        assert_eq!(result.extent().record_len(), Some(bytes.len()));
        assert_eq!(
            result.extent().body_validation(),
            Some(BodyValidation::NotValidated)
        );
    }
}

#[test]
fn body_payload_and_trailing_bytes_are_outside_the_inspected_prefix() {
    let original = inspection::inspect(CLOSURE).unwrap();
    let prefix_end = original.extent().body_offset().unwrap() + 4;
    let mut bytes = CLOSURE[..prefix_end].to_vec();
    bytes.extend_from_slice(&[0xff; 64]);
    let result = inspection::inspect_with_options(
        &bytes,
        InspectionOptions::default().max_bytes_visited(prefix_end),
    )
    .unwrap();
    assert_eq!(result.formals(), original.formals());
    assert_eq!(result.extent().body_kind(), original.extent().body_kind());
    assert_eq!(
        result.extent().body_validation(),
        Some(BodyValidation::NotValidated)
    );
    assert_eq!(result.extent().record_len(), Some(bytes.len()));
    assert!(rd_rds::parse(&bytes).is_err());
}

#[test]
fn distinguishes_root_errors_from_unavailable_closure_formals() {
    let error = inspection::inspect(b"not an XDR stream").unwrap_err();
    assert_eq!(error.phase(), FailurePhase::Root);
    assert_eq!(error.cause(), &FailureCause::Malformed);
    let body_offset = inspection::inspect(CLOSURE)
        .unwrap()
        .extent()
        .body_offset()
        .unwrap();
    let result = inspection::inspect(&CLOSURE[..body_offset]).unwrap();
    assert_eq!(result.kind(), StoredKind::Closure);
    let FormalsInspection::Unavailable(FormalsUnavailable::Prefix(failure)) = result.formals()
    else {
        panic!("expected unavailable formals: {result:?}");
    };
    assert_eq!(failure.phase(), FailurePhase::BodyTag);
    assert_eq!(failure.offset(), body_offset);
    assert_eq!(failure.cause(), &FailureCause::Malformed);
}

#[test]
fn exposes_byte_formal_and_shared_decoder_limits() {
    let error = inspection::inspect_with_options(
        CLOSURE,
        InspectionOptions::default().max_bytes_visited(2),
    )
    .unwrap_err();
    assert_eq!(error.phase(), FailurePhase::Root);
    assert_eq!(error.cause(), &FailureCause::ResourceLimit);
    for options in [
        InspectionOptions::default().max_formals(2),
        InspectionOptions::default().limits(Limits::default().max_references(0)),
    ] {
        let result = inspection::inspect_with_options(CLOSURE, options).unwrap();
        let FormalsInspection::Unavailable(FormalsUnavailable::Prefix(failure)) = result.formals()
        else {
            panic!("expected unavailable formals: {result:?}");
        };
        assert_eq!(failure.phase(), FailurePhase::Formals);
        assert_eq!(failure.cause(), &FailureCause::ResourceLimit);
    }
}

#[test]
fn preserves_unsupported_default_diagnostics() {
    let result = inspection::inspect(include_bytes!(
        "fixtures/data/closure_default_altrep_v3.rds"
    ))
    .unwrap();
    let FormalsInspection::Unavailable(FormalsUnavailable::Prefix(failure)) = result.formals()
    else {
        panic!("expected unavailable formals: {result:?}");
    };
    assert_eq!(failure.phase(), FailurePhase::Default(0));
    assert!(matches!(failure.cause(), FailureCause::Unsupported { .. }));
}

#[test]
fn observing_a_non_closure_tag_does_not_validate_its_payload() {
    let mut cursor = ByteCursor::new(CLOSURE);
    Header::parse(&mut cursor).unwrap();
    for (tag, kind) in [(13_u32, StoredKind::Integer), (238, StoredKind::Other(238))] {
        let mut bytes = CLOSURE[..cursor.position()].to_vec();
        bytes.extend_from_slice(&tag.to_be_bytes());
        let result = inspection::inspect(&bytes).unwrap();
        assert_eq!(result.kind(), kind);
        assert_eq!(result.extent(), &InspectionExtent::RootTagOnly);
        assert_eq!(
            result.formals(),
            &FormalsInspection::NotApplicable(FormalsNotApplicable::NonClosure)
        );
        assert!(rd_rds::parse(&bytes).is_err());
    }
}
