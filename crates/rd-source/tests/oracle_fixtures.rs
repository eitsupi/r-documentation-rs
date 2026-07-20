mod support;

use std::collections::BTreeSet;

use rd_ast::{RdDocument, RdNode, RdTag};
use support::{
    diff::{DiffKind, OptionPresence, compare},
    fixture::{Comparison, SourceStatus, fixture_root, load_cases, load_oracle, parse_cases},
    obligations::check,
};

#[test]
fn manifest_rd_and_oracle_basenames_correspond_exactly() {
    let root = fixture_root();
    let cases = load_cases(&root).unwrap();
    let names: BTreeSet<_> = cases.iter().map(|c| c.name.clone()).collect();
    let rd: BTreeSet<_> = std::fs::read_dir(root.join("rd"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .path()
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let oracle: BTreeSet<_> = std::fs::read_dir(root.join("oracle"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == "rds")
                .then(|| e.path().file_stem().unwrap().to_string_lossy().into_owned())
        })
        .collect();
    assert_eq!(names, rd);
    let oracle_cases: BTreeSet<_> = cases
        .iter()
        .filter(|c| !c.is_source_only())
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(oracle_cases, oracle);
}

#[test]
fn enabled_subject_fixtures_follow_comparison_policy() {
    let root = fixture_root();
    let cases = load_cases(&root).unwrap();
    let mut enabled = 0;
    let mut compared = 0;
    let mut divergent = 0;
    for case in cases
        .iter()
        .filter(|case| case.source_status == Some(SourceStatus::Valid))
    {
        enabled += 1;
        let bytes = std::fs::read(root.join(&case.rd)).unwrap();
        let parsed =
            rd_source::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", case.name));
        assert!(
            parsed.diagnostics().is_empty(),
            "{}: unexpected diagnostics: {:?}",
            case.name,
            parsed.diagnostics()
        );
        match case.comparison {
            Comparison::OracleParity => {
                check(parsed.document(), case, case.flat_obligations()).unwrap();
                let expected = load_oracle(&root, case).unwrap();
                let differences = compare(&expected, parsed.document());
                assert!(
                    differences.is_empty(),
                    "{} differences:\n{}",
                    case.name,
                    differences
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                compared += 1;
            }
            Comparison::IntentionalDivergence => {
                check(parsed.document(), case, case.source_obligations()).unwrap();
                let expected = load_oracle(&root, case).unwrap();
                check(&expected, case, case.oracle_obligations()).unwrap();
                divergent += 1;
            }
            Comparison::SourceOnly => {
                check(parsed.document(), case, case.source_obligations()).unwrap();
            }
        }
    }
    assert_eq!(enabled, 76);
    assert_eq!(compared, 72);
    assert_eq!(divergent, 0);
}

#[test]
fn corpus_is_complete() {
    let cases = load_cases(&fixture_root()).unwrap();
    assert_eq!(cases.len(), 76);
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.source_status == Some(SourceStatus::Valid))
            .count(),
        76
    );
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.source_status.is_none())
            .count(),
        0
    );
}

#[test]
fn every_oracle_lowers_and_satisfies_its_own_obligations() {
    let root = fixture_root();
    for case in load_cases(&root).unwrap() {
        if case.is_source_only() {
            continue;
        }
        let document = load_oracle(&root, &case).unwrap();
        let obligations = match case.comparison {
            Comparison::OracleParity => case.flat_obligations(),
            Comparison::IntentionalDivergence => case.oracle_obligations(),
            Comparison::SourceOnly => unreachable!(),
        };
        check(&document, &case, obligations).unwrap();
    }
}

#[test]
fn strict_manifest_deserializes_toml_values_without_custom_codecs() {
    let text = r###"
[[case]]
name = "special # case"
category = "test"
rd = "rd/x.Rd"
oracle = "oracle/x.rds"
phase = "test"
obligations = ["one", "two"]
comments = 0
comment_values = ["# | \" \\ \n \t 日本語"]
expected_leaves = [{ kind = "TEXT", value = "# | \" \\ \n \t 日本語" }, { kind = "VERB", value = "two" }]
expected_sequence = [{ path = "root/0", kinds = ["TEXT", "VERB"] }]
"###;
    let case = parse_cases(text).unwrap().pop().unwrap();
    assert_eq!(case.obligations, ["one", "two"]);
    assert_eq!(case.comment_values.unwrap()[0], "# | \" \\ \n \t 日本語");
    assert_eq!(
        case.expected_leaves.unwrap()[0].value,
        "# | \" \\ \n \t 日本語"
    );
}

#[test]
fn strict_manifest_rejects_unknown_fields_and_wrong_types() {
    let base = r#"[[case]]
name = "x"
category = "test"
rd = "rd/x.Rd"
oracle = "oracle/x.rds"
phase = "test"
obligations = []
comments = 0
"#;
    assert!(parse_cases(&format!("{base}unknown = true\n")).is_err());
    let wrong_type = base.replace("comments = 0", r#"comments = "zero""#);
    assert!(parse_cases(&wrong_type).is_err());
}

#[test]
fn comparator_reports_all_mismatch_kinds_with_paths() {
    let path = |i| rd_ast::RdPath::new(vec![rd_ast::RdPathSegment::TopLevel(i)]);
    let expected = RdDocument::new(vec![
        RdNode::tagged(RdTag::Link, Some(vec![]), vec![RdNode::Text("a\n".into())]),
        RdNode::Comment("% c".into()),
        RdNode::group(vec![]),
    ]);
    let actual = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Title,
            Some(vec![RdNode::Text("x".into())]),
            vec![RdNode::Text("b\t".into())],
        ),
        RdNode::Text("unexpected".into()),
    ]);
    let diffs = compare(&expected, &actual);
    assert!(
        diffs
            .iter()
            .any(|d| d.path == path(0) && matches!(d.kind, DiffKind::TagMismatch { .. }))
    );
    assert!(diffs.iter().any(|d| d.path == path(0).with_option()
        && matches!(
            d.kind,
            DiffKind::OptionPresenceMismatch {
                expected: OptionPresence::PresentEmpty,
                actual: OptionPresence::PresentNonEmpty
            }
        )));
    assert!(
        diffs.iter().any(|d| d.path == path(0).with_child(0)
            && matches!(d.kind, DiffKind::LeafTextMismatch { .. }))
    );
    assert!(
        diffs
            .iter()
            .any(|d| d.path == path(1) && matches!(d.kind, DiffKind::NodeKindMismatch { .. }))
    );
    assert!(
        diffs
            .iter()
            .any(|d| matches!(d.kind, DiffKind::ChildCountMismatch { .. }))
    );
    assert!(
        diffs
            .iter()
            .any(|d| d.path == path(2) && matches!(d.kind, DiffKind::MissingNode))
    );
    assert!(
        diffs
            .iter()
            .any(|d| d.path == path(1).with_child(0) && matches!(d.kind, DiffKind::UnexpectedNode))
            || diffs
                .iter()
                .any(|d| matches!(d.kind, DiffKind::UnexpectedNode))
    );
}

#[test]
fn comparator_reports_raw_fallback() {
    let raw =
        rd_ast::producer::raw_node(None, None, vec![], Some(rd_ast::RawRdValue::Null), vec![]);
    let a = RdDocument::new(vec![RdNode::Raw(raw.clone())]);
    let b = RdDocument::new(vec![RdNode::Raw(rd_ast::producer::raw_node(
        Some("x".into()),
        None,
        vec![],
        Some(rd_ast::RawRdValue::Null),
        vec![],
    ))]);
    let diffs = compare(&a, &b);
    assert!(matches!(diffs[0].kind, DiffKind::RawNodeNotComparable));

    let with_option_a = RdDocument::new(vec![RdNode::Raw(rd_ast::producer::raw_node(
        None,
        Some(vec![RdNode::Text("a".into())]),
        vec![],
        Some(rd_ast::RawRdValue::Null),
        vec![],
    ))]);
    let with_option_b = RdDocument::new(vec![RdNode::Raw(rd_ast::producer::raw_node(
        None,
        Some(vec![RdNode::Text("b".into())]),
        vec![],
        Some(rd_ast::RawRdValue::Null),
        vec![],
    ))]);
    assert!(matches!(
        compare(&with_option_a, &with_option_b)[0].kind,
        DiffKind::RawNodeNotComparable
    ));
}
