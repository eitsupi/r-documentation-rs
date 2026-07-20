use super::*;
use crate::{RdLifecycleStage, RdNode, RdTag, producer};

fn figure(file: &str) -> RdNode {
    RdNode::tagged(
        RdTag::Figure,
        None,
        vec![RdNode::group(vec![RdNode::Verb(file.into())])],
    )
}

fn description(children: Vec<RdNode>) -> RdDocument {
    RdDocument::new(vec![RdNode::tagged(RdTag::Description, None, children)])
}

fn canonical_badge(
    file: &str,
    format: &str,
    conditional: RdTag,
    then_children: Vec<RdNode>,
    fallback: &str,
    url: &str,
) -> RdNode {
    let mut children = vec![
        RdNode::group(vec![RdNode::Text(format.into())]),
        RdNode::group(then_children),
    ];
    if conditional == RdTag::IfElse {
        children.push(RdNode::group(vec![RdNode::tagged(
            RdTag::Strong,
            None,
            vec![RdNode::Text(fallback.into())],
        )]));
    }
    let then_children = children[1].as_group().unwrap().children();
    if then_children.is_empty() {
        children[1] = RdNode::group(vec![RdNode::tagged(
            RdTag::Href,
            None,
            vec![
                RdNode::group(if url.is_empty() {
                    vec![]
                } else {
                    vec![RdNode::Verb(url.into())]
                }),
                RdNode::group(vec![figure(file)]),
            ],
        )]);
    }
    RdNode::tagged(conditional, None, children)
}

fn canonical_badge_with_trivia(file: &str, fallback: &str) -> RdNode {
    RdNode::tagged(
        RdTag::IfElse,
        None,
        vec![
            // The format group tolerates no trivia: format() is an exact
            // Text-scalar projection compared against "html" verbatim.
            RdNode::group(vec![RdNode::Text("html".into())]),
            RdNode::group(vec![
                RdNode::Comment("% then".into()),
                RdNode::tagged(
                    RdTag::Href,
                    None,
                    vec![
                        RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                        RdNode::group(vec![figure(file)]),
                    ],
                ),
                RdNode::Text("\t".into()),
            ]),
            RdNode::group(vec![
                RdNode::Text(" ".into()),
                RdNode::tagged(RdTag::Strong, None, vec![RdNode::Text(fallback.into())]),
                RdNode::Comment("% else".into()),
            ]),
        ],
    )
}

#[test]
fn stages_and_filename_rules() {
    let names = [
        ("experimental", RdLifecycleStage::Experimental),
        ("stable", RdLifecycleStage::Stable),
        ("superseded", RdLifecycleStage::Superseded),
        ("deprecated", RdLifecycleStage::Deprecated),
        ("maturing", RdLifecycleStage::Maturing),
        ("questioning", RdLifecycleStage::Questioning),
        ("soft-deprecated", RdLifecycleStage::SoftDeprecated),
        ("defunct", RdLifecycleStage::Defunct),
        ("retired", RdLifecycleStage::Retired),
    ];
    for (name, expected) in names {
        let document = description(vec![figure(&format!("lifecycle-{name}.svg"))]);
        let badges = document.lifecycle_badges();
        assert_eq!(badges.first().unwrap().stage(), &expected);
        assert_eq!(badges.first().unwrap().stage().as_str(), name);
    }
    assert!(RdLifecycleStage::Stable.is_current());
    assert!(RdLifecycleStage::Retired.is_legacy());
    let unknown = RdLifecycleStage::Unknown("preview".into());
    assert!(!unknown.is_current() && !unknown.is_legacy() && !unknown.is_known());
    assert_eq!(
        description(vec![figure("lifecycle-STABLE.PNG")])
            .lifecycle_badges()
            .first()
            .unwrap()
            .stage(),
        &RdLifecycleStage::Stable
    );
    assert!(
        description(vec![figure("Lifecycle-stable.svg")])
            .lifecycle_badges()
            .first()
            .is_none()
    );
    assert!(
        description(vec![figure("lifecycle-stable")])
            .lifecycle_badges()
            .first()
            .is_some()
    );
    assert_eq!(
        description(vec![figure("lifecycle-preview.svg")])
            .lifecycle_badges()
            .first()
            .unwrap()
            .stage(),
        &RdLifecycleStage::Unknown("preview".into())
    );
    assert!(
        description(vec![figure("lifecycle-.svg"), figure("ordinary.svg")])
            .lifecycle_badges()
            .first()
            .is_none()
    );
}

#[test]
fn bare_badge_has_path_and_no_shape() {
    let document = description(vec![figure("lifecycle-stable.svg")]);
    let badges = document.lifecycle_badges();
    let badge = badges.first().unwrap();
    assert_eq!(badge.figure().file(), "lifecycle-stable.svg");
    assert_eq!(
        badge.path().segments(),
        &[RdPathSegment::TopLevel(0), RdPathSegment::Child(0),]
    );
    assert!(badge.canonical_shape().is_none());
}

#[test]
fn canonical_badge_is_matched_before_recursing() {
    let conditional = RdNode::tagged(
        RdTag::IfElse,
        None,
        vec![
            RdNode::group(vec![RdNode::Text("html".into())]),
            RdNode::group(vec![RdNode::tagged(
                RdTag::Href,
                None,
                vec![
                    RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                    RdNode::group(vec![figure("lifecycle-stable.svg")]),
                ],
            )]),
            RdNode::group(vec![RdNode::tagged(
                RdTag::Strong,
                None,
                vec![RdNode::Text("[Stable]".into())],
            )]),
        ],
    );
    let document = description(vec![conditional]);
    let badges = document.lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 1);
    let badge = badges.first().unwrap();
    let shape = badge.canonical_shape().unwrap();
    assert_eq!(
        shape.href().url(),
        &[RdNode::Verb("https://example.test".into())]
    );
    assert_eq!(shape.fallback_text(), "Stable");
}

#[test]
fn strict_canonical_mismatches_are_permissive_hits() {
    let cases = [
        canonical_badge(
            "lifecycle-stable.svg",
            "latex",
            RdTag::IfElse,
            vec![],
            "[Stable]",
            "https://example.test",
        ),
        canonical_badge(
            "lifecycle-stable.svg",
            "html",
            RdTag::If,
            vec![],
            "[Stable]",
            "https://example.test",
        ),
        RdNode::tagged(
            RdTag::IfElse,
            None,
            vec![
                RdNode::group(vec![RdNode::Text("html".into())]),
                RdNode::group(vec![figure("lifecycle-stable.svg")]),
                RdNode::group(vec![RdNode::tagged(
                    RdTag::Strong,
                    None,
                    vec![RdNode::Text("[Stable]".into())],
                )]),
            ],
        ),
        RdNode::tagged(
            RdTag::IfElse,
            None,
            vec![
                RdNode::group(vec![RdNode::Text("html".into())]),
                RdNode::group(vec![
                    RdNode::tagged(
                        RdTag::Href,
                        None,
                        vec![
                            RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                            RdNode::group(vec![figure("lifecycle-stable.svg")]),
                        ],
                    ),
                    RdNode::Text("significant".into()),
                ]),
                RdNode::group(vec![RdNode::tagged(
                    RdTag::Strong,
                    None,
                    vec![RdNode::Text("[Stable]".into())],
                )]),
            ],
        ),
        canonical_badge(
            "lifecycle-stable.svg",
            "html",
            RdTag::IfElse,
            vec![RdNode::tagged(
                RdTag::Href,
                None,
                vec![
                    RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                    RdNode::group(vec![figure("lifecycle-stable.svg")]),
                ],
            )],
            "[Experimental]",
            "https://example.test",
        ),
        canonical_badge(
            "lifecycle-stable.svg",
            "html",
            RdTag::IfElse,
            vec![RdNode::tagged(
                RdTag::Href,
                None,
                vec![
                    RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                    RdNode::group(vec![figure("lifecycle-stable.svg")]),
                ],
            )],
            "Stable",
            "https://example.test",
        ),
        canonical_badge(
            "lifecycle-stable.svg",
            "html",
            RdTag::IfElse,
            vec![RdNode::tagged(
                RdTag::Href,
                None,
                vec![
                    RdNode::group(vec![]),
                    RdNode::group(vec![figure("lifecycle-stable.svg")]),
                ],
            )],
            "[Stable]",
            "",
        ),
    ];

    for candidate in cases {
        let document = description(vec![candidate]);
        let badges = document.inspect_lifecycle_badges();
        assert_eq!(badges.as_slice().len(), 1);
        assert!(badges.first().unwrap().canonical_shape().is_none());
        assert!(badges.diagnostics().is_empty());
    }
}

#[test]
fn trivia_inside_canonical_groups_is_ignored() {
    let document = description(vec![canonical_badge_with_trivia(
        "lifecycle-stable.svg",
        "[Stable]",
    )]);
    let badges = document.lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 1);
    assert!(badges.first().unwrap().canonical_shape().is_some());
}

#[test]
fn nested_badges_are_collected_in_source_order() {
    let first = figure("lifecycle-experimental.svg");
    let canonical = canonical_badge_with_trivia("lifecycle-stable.svg", "[Stable]");
    let last = figure("lifecycle-retired.svg");
    let document = description(vec![RdNode::tagged(
        RdTag::Unknown("opaque".into()),
        None,
        vec![RdNode::group(vec![first]), canonical, last],
    )]);
    let badges = document.lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 3);
    assert_eq!(
        badges.as_slice()[0].figure().file(),
        "lifecycle-experimental.svg"
    );
    assert_eq!(badges.as_slice()[1].figure().file(), "lifecycle-stable.svg");
    assert_eq!(
        badges.as_slice()[2].figure().file(),
        "lifecycle-retired.svg"
    );
    assert_eq!(
        badges.first().unwrap().figure().file(),
        "lifecycle-experimental.svg"
    );
}

#[test]
fn option_and_raw_children_are_not_recursed() {
    let option_figure = RdNode::tagged(
        RdTag::Unknown("opaque".into()),
        Some(vec![figure("lifecycle-stable.svg")]),
        vec![],
    );
    let raw_figure = RdNode::Raw(producer::raw_node(
        Some("opaque".into()),
        None,
        vec![figure("lifecycle-retired.svg")],
        None,
        vec![],
    ));
    let document = description(vec![option_figure, raw_figure]);
    let badges = document.lifecycle_badges();
    assert!(badges.as_slice().is_empty());
    assert!(badges.diagnostics().is_empty());
}

#[test]
fn malformed_and_raw_candidates_are_reported_only_in_inspect_mode() {
    let raw_description = RdNode::Raw(producer::raw_node(
        Some(r"\description".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    let raw_figure = RdNode::Raw(producer::raw_node(
        Some(r"\figure".into()),
        None,
        vec![],
        None,
        vec![],
    ));
    let malformed_figure = RdNode::tagged(
        RdTag::Figure,
        None,
        vec![RdNode::group(vec![RdNode::Text(
            "lifecycle-stable.svg".into(),
        )])],
    );
    let document = RdDocument::new(vec![
        raw_description,
        description(vec![raw_figure, malformed_figure])
            .into_nodes()
            .into_iter()
            .next()
            .unwrap(),
    ]);
    let inspected = document.inspect_lifecycle_badges();
    assert!(inspected.as_slice().is_empty());
    assert_eq!(inspected.diagnostics().len(), 3);
    assert!(inspected.diagnostics().iter().any(|error| matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            actual: RdNodeKind::Raw,
            ..
        }
    )));
    assert_eq!(
        inspected
            .diagnostics()
            .iter()
            .filter(|error| matches!(
                error.kind(),
                RdShapeErrorKind::UnexpectedNode {
                    actual: RdNodeKind::Raw,
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn description_option_reports_but_badges_are_collected() {
    let document = RdDocument::new(vec![RdNode::tagged(
        RdTag::Description,
        Some(vec![RdNode::Text("option".into())]),
        vec![figure("lifecycle-stable.svg")],
    )]);
    let badges = document.inspect_lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 1);
    assert!(
        badges
            .diagnostics()
            .iter()
            .any(|error| matches!(error.kind(), RdShapeErrorKind::UnexpectedOption))
    );
}

#[test]
fn unknown_stage_fallbacks_are_normalized_strictly() {
    let href = |file: &str| {
        RdNode::tagged(
            RdTag::Href,
            None,
            vec![
                RdNode::group(vec![RdNode::Verb("https://example.test".into())]),
                RdNode::group(vec![figure(file)]),
            ],
        )
    };
    for fallback in ["[preview]", "[Preview]"] {
        let document = description(vec![canonical_badge(
            "lifecycle-preview.svg",
            "html",
            RdTag::IfElse,
            vec![href("lifecycle-preview.svg")],
            fallback,
            "https://example.test",
        )]);
        let badges = document.lifecycle_badges();
        assert!(badges.first().unwrap().canonical_shape().is_some());
        assert_eq!(
            badges.first().unwrap().stage(),
            &RdLifecycleStage::Unknown("preview".into())
        );
    }
    let document = description(vec![canonical_badge(
        "lifecycle-preview.svg",
        "html",
        RdTag::IfElse,
        vec![href("lifecycle-preview.svg")],
        "[sneak peek]",
        "https://example.test",
    )]);
    let badges = document.lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 1);
    assert!(badges.first().unwrap().canonical_shape().is_none());

    // An uppercase filename spelling keeps its Unknown spelling but still
    // matches a case-variant fallback for canonical-shape evidence.
    for fallback in ["[preview]", "[Preview]"] {
        let document = description(vec![canonical_badge(
            "lifecycle-Preview.svg",
            "html",
            RdTag::IfElse,
            vec![href("lifecycle-Preview.svg")],
            fallback,
            "https://example.test",
        )]);
        let badges = document.lifecycle_badges();
        assert_eq!(
            badges.first().unwrap().stage(),
            &RdLifecycleStage::Unknown("Preview".into())
        );
        assert!(badges.first().unwrap().canonical_shape().is_some());
    }
}

#[test]
fn nested_figure_inside_malformed_figure_is_still_discovered() {
    let malformed_outer = RdNode::tagged(
        RdTag::Figure,
        None,
        vec![RdNode::group(vec![figure("lifecycle-stable.svg")])],
    );
    let document = description(vec![malformed_outer]);
    let lossy = document.lifecycle_badges();
    assert_eq!(lossy.as_slice().len(), 1);
    assert_eq!(lossy.first().unwrap().stage(), &RdLifecycleStage::Stable);
    assert!(lossy.diagnostics().is_empty());
    let inspected = document.inspect_lifecycle_badges();
    assert_eq!(inspected.as_slice().len(), 1);
    assert_eq!(inspected.diagnostics().len(), 1);
}

#[test]
fn all_descriptions_are_scanned_and_duplicates_inspected() {
    let document = RdDocument::new(vec![
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![figure("lifecycle-stable.svg")],
        ),
        RdNode::tagged(
            RdTag::Description,
            None,
            vec![figure("lifecycle-retired.svg")],
        ),
    ]);
    assert_eq!(document.lifecycle_badges().as_slice().len(), 2);
    assert!(document.lifecycle_badges().diagnostics().is_empty());
    assert_eq!(document.inspect_lifecycle_badges().diagnostics().len(), 1);
}
