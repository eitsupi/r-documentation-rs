#![cfg(feature = "rds")]

use std::{fs, io::Read, path::PathBuf};

use flate2::read::GzDecoder;
use rd_ast::{
    RdColumnAlign, RdDocument, RdDynamicMarkupEvent, RdDynamicMarkupState, RdEquationDisplay,
    RdLifecycleStage, RdLinkDestination, RdLinkTopic, RdListItem, RdNode, RdPath, RdPathSegment,
    RdSexprResults, RdSexprStage, RdTag, lower_r_object, text_contents,
};
use rd_rds::parse;

fn fixture(version: u8) -> rd_rds::RObject {
    // These crate-local copies originate from crates/rd-rds/tests/fixtures/data/ and committed
    // R generator scripts; keep them byte-identical so packaged rd-ast tests run standalone.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
    let name = format!("rd_semantics_v{version}.rds");
    let bytes = fs::read(dir.join(&name)).unwrap_or_else(|err| panic!("read {name}: {err}"));
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();
    parse(&decompressed).unwrap()
}

fn tagged<'a>(
    nodes: &'a [RdNode],
    wanted: &RdTag,
    path: &RdPath,
) -> Vec<(&'a rd_ast::RdTagged, RdPath)> {
    let mut found = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let child_path = if path.segments().is_empty() {
            RdPath::new(vec![RdPathSegment::TopLevel(index)])
        } else {
            path.with_child(index)
        };
        if let Some(node) = node.as_tagged() {
            if node.tag() == wanted {
                found.push((node, child_path.clone()));
            }
            found.extend(tagged(node.children(), wanted, &child_path));
        } else if let Some(group) = node.as_group() {
            found.extend(tagged(group.children(), wanted, &child_path));
        }
    }
    found
}

fn normalize(nodes: &[RdNode]) -> String {
    text_contents(nodes)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn real_r_semantics_fixture_conforms_for_rds_versions() {
    for version in [2, 3] {
        let document: RdDocument = lower_r_object(&fixture(version)).unwrap();
        let root = RdPath::new(vec![]);

        let links = tagged(document.nodes(), &RdTag::Link, &root);
        assert_eq!(links.len(), 4, "version {version}");
        assert!(matches!(
            links[0].0.inspect_link(&links[0].1).unwrap().destination(),
            RdLinkDestination::DisplayText { .. }
        ));
        assert!(matches!(
            links[1].0.inspect_link(&links[1].1).unwrap().destination(),
            RdLinkDestination::Package { package, topic: RdLinkTopic::DisplayText(_)
            } if *package == "pkg"
        ));
        assert!(matches!(
            links[2].0.inspect_link(&links[2].1).unwrap().destination(),
            RdLinkDestination::Package { package, topic: RdLinkTopic::Explicit(topic) }
            if *package == "pkg" && *topic == "topic"
        ));
        assert!(matches!(
            links[3].0.inspect_link(&links[3].1).unwrap().destination(),
            RdLinkDestination::Explicit { topic } if *topic == "dest"
        ));
        assert_eq!(
            normalize(links[2].0.inspect_link(&links[2].1).unwrap().display()),
            "display with markup"
        );

        let hrefs = tagged(document.nodes(), &RdTag::Href, &root);
        assert_eq!(hrefs.len(), 2);
        let href = hrefs[0].0.inspect_href(&hrefs[0].1).unwrap();
        assert_eq!(normalize(href.url()), "https://example.org");
        assert_eq!(normalize(href.display()), "the site");

        let badges = document.lifecycle_badges();
        assert_eq!(badges.as_slice().len(), 1, "version {version}");
        let badge = badges.first().unwrap();
        assert_eq!(badge.stage(), &RdLifecycleStage::Stable);
        assert_eq!(badge.figure().file(), "lifecycle-stable.svg");
        assert!(badge.canonical_shape().is_some());
        let shape = badge.canonical_shape().unwrap();
        assert_eq!(shape.fallback_text(), "Stable");
        assert!(document.inspect_lifecycle_badges().diagnostics().is_empty());

        let itemize = tagged(document.nodes(), &RdTag::Itemize, &root);
        let items = itemize[0]
            .0
            .inspect_list(&itemize[0].1)
            .unwrap()
            .items()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(items.len(), 3);
        assert!(
            matches!(&items[1], RdListItem::Delimited(item) if normalize(item.body()).starts_with("[label]"))
        );
        let enumerate = tagged(document.nodes(), &RdTag::Enumerate, &root);
        assert_eq!(
            enumerate[0]
                .0
                .inspect_list(&enumerate[0].1)
                .unwrap()
                .items()
                .count(),
            2
        );
        let describe = tagged(document.nodes(), &RdTag::Describe, &root);
        let described = describe[0]
            .0
            .inspect_list(&describe[0].1)
            .unwrap()
            .items()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(described.len(), 2);
        assert!(
            matches!((&described[0], &described[1]), (RdListItem::Described(a), RdListItem::Described(b)) if normalize(a.label()) == "first label" && normalize(a.body()) == "first body" && normalize(b.label()) == "second label" && normalize(b.body()) == "second body with markup")
        );

        let tables = tagged(document.nodes(), &RdTag::Tabular, &root);
        assert_eq!(tables.len(), 1);
        let table = tables[0].0.inspect_tabular(&tables[0].1).unwrap();
        assert_eq!(
            table.columns(),
            &[
                RdColumnAlign::Left,
                RdColumnAlign::Center,
                RdColumnAlign::Right
            ]
        );
        assert!(
            table.diagnostics().is_empty(),
            "unexpected table diagnostics: {:?}",
            table.diagnostics()
        );
        assert_eq!(table.rows().len(), 3);
        assert_eq!(table.rows()[0].cells().len(), 3);
        assert_eq!(normalize(table.rows()[0].cells()[0].nodes()), "left one");
        assert_eq!(normalize(table.rows()[0].cells()[1].nodes()), "center one");
        assert_eq!(normalize(table.rows()[0].cells()[2].nodes()), "right one");
        // R preserves the separator's surrounding space in this empty cell;
        // the semantic cell content is empty after whitespace normalization.
        assert_eq!(normalize(table.rows()[1].cells()[1].nodes()), "");
        assert_eq!(normalize(table.rows()[2].cells()[0].nodes()), "final left");

        let equations = tagged(document.nodes(), &RdTag::Eqn, &root);
        assert_eq!(equations.len(), 2);
        assert_eq!(
            normalize(
                equations[0]
                    .0
                    .inspect_equation(&equations[0].1)
                    .unwrap()
                    .latex()
            ),
            "x^2"
        );
        assert_eq!(
            equations[0]
                .0
                .inspect_equation(&equations[0].1)
                .unwrap()
                .ascii(),
            None
        );
        let equation = equations[1].0.inspect_equation(&equations[1].1).unwrap();
        assert_eq!(normalize(equation.latex()), "x^2");
        assert_eq!(normalize(equation.ascii().unwrap()), "x squared");
        let deqn = tagged(document.nodes(), &RdTag::Deqn, &root);
        let equation = deqn[0].0.inspect_equation(&deqn[0].1).unwrap();
        assert_eq!(equation.display(), RdEquationDisplay::Block);
        assert_eq!(normalize(equation.latex()), r"\sum_i x_i");
        assert_eq!(normalize(equation.ascii().unwrap()), "sum x");

        let opts = tagged(document.nodes(), &RdTag::RdOpts, &root);
        assert_eq!(opts.len(), 1, "version {version}");
        assert_eq!(opts[0].0.children().len(), 1);
        assert!(matches!(opts[0].0.children()[0], RdNode::Verb(_)));
        let opts_view = opts[0].0.inspect_rd_opts(&opts[0].1).unwrap();
        assert_eq!(
            opts_view.option_overrides().stage,
            Some(RdSexprStage::Build)
        );
        assert_eq!(
            opts_view.option_overrides().results,
            Some(RdSexprResults::Verbatim)
        );

        let sexprs = tagged(document.nodes(), &RdTag::Sexpr, &root);
        assert_eq!(sexprs.len(), 2, "version {version}");
        let events = document
            .inspect_dynamic_markup()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let dynamic: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    RdDynamicMarkupEvent::OptionsChanged { .. } | RdDynamicMarkupEvent::Sexpr(_)
                )
            })
            .collect();
        assert_eq!(dynamic.len(), 3);
        assert!(matches!(dynamic[0], RdDynamicMarkupEvent::Sexpr(resolved)
            if resolved.effective_options().stage == RdSexprStage::Install
                && resolved.state() == RdDynamicMarkupState::Unresolved { stage: RdSexprStage::Install }));
        assert!(
            matches!(dynamic[1], RdDynamicMarkupEvent::OptionsChanged { effective, .. }
            if effective.stage == RdSexprStage::Build && effective.results == RdSexprResults::Verbatim)
        );
        assert!(matches!(dynamic[2], RdDynamicMarkupEvent::Sexpr(resolved)
            if resolved.effective_options().stage == RdSexprStage::Render
                && resolved.effective_options().results == RdSexprResults::Rd
                && resolved.state() == RdDynamicMarkupState::Unresolved { stage: RdSexprStage::Render }));
        assert_eq!(
            sexprs[0].1.segments(),
            &[RdPathSegment::TopLevel(4), RdPathSegment::Child(1)]
        );
        assert_eq!(
            sexprs[1].0.inspect_sexpr(&sexprs[1].1).unwrap().code(),
            "2 + 2"
        );
    }
}
