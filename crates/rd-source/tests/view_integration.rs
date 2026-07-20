use rd_ast::{RdNode, RdPath, RdTag};

fn parse(input: &str) -> rd_ast::RdDocument {
    let parsed = rd_source::parse(input.as_bytes()).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "diagnostics: {:?}",
        parsed.diagnostics()
    );
    parsed.document().clone()
}

fn walk<'a>(nodes: &'a [RdNode], path: RdPath, out: &mut Vec<(&'a RdNode, RdPath)>) {
    for (index, node) in nodes.iter().enumerate() {
        let child_path = path.with_child(index);
        out.push((node, child_path.clone()));
        match node {
            RdNode::Tagged(tagged) => walk(tagged.children(), child_path, out),
            RdNode::Group(group) => walk(group.children(), child_path, out),
            _ => {}
        }
    }
}

fn nodes(document: &rd_ast::RdDocument) -> Vec<(&RdNode, RdPath)> {
    let mut out = Vec::new();
    walk(document.nodes(), RdPath::new(vec![]), &mut out);
    out
}

#[test]
fn producer_inline_mode_matrix() {
    let document = parse(
        r"\description{\emph{x} \code{f(x)} \verb{v} \url{u} \samp{s} \kbd{k} \env{e} \option{o}}",
    );
    let spans: Vec<_> = nodes(&document)
        .into_iter()
        .filter_map(|(node, path)| {
            node.inspect_inline_span(&path)
                .unwrap()
                .map(|view| (view.kind(), view.body().first().map(rd_ast::RdNodeKind::of)))
        })
        .collect();
    assert_eq!(spans.len(), 8);
    assert!(spans.iter().all(|(_, kind)| kind.is_some()));
    assert_eq!(spans[0].1, Some(rd_ast::RdNodeKind::Text));
    assert_eq!(spans[1].1, Some(rd_ast::RdNodeKind::RCode));
    for (_, kind) in &spans[2..] {
        assert_eq!(*kind, Some(rd_ast::RdNodeKind::Verb));
    }
}

#[test]
fn producer_structural_views_round_trip() {
    let document = parse(
        r"\description{\if{html}{content} \ifelse{latex}{a}{b} \enc{São Paulo}{Sao Paulo} \figure{img.png}{options: width=30} \figure{img.png} \linkS4class{myClass} \linkS4class[methods]{myClass}}",
    );
    let mut conditionals = Vec::new();
    let mut enc = Vec::new();
    let mut figures = Vec::new();
    let mut links = Vec::new();
    for (node, path) in nodes(&document) {
        if let Some(view) = node.inspect_conditional(&path).unwrap() {
            conditionals.push(view);
        }
        if let Some(view) = node.inspect_enc(&path).unwrap() {
            enc.push(view);
        }
        if let Some(view) = node.inspect_figure(&path).unwrap() {
            figures.push(view);
        }
        if let Some(view) = node.inspect_s4_class_link(&path).unwrap() {
            links.push(view);
        }
    }
    assert_eq!(conditionals.len(), 2);
    assert_eq!(conditionals[0].format(), "html");
    assert_eq!(conditionals[1].format(), "latex");
    assert_eq!(enc.len(), 1);
    assert_eq!(enc[0].encoded(), &[RdNode::Text("São Paulo".into())]);
    assert_eq!(figures.len(), 2);
    assert_eq!(figures[0].file(), "img.png");
    assert_eq!(
        figures[0].second().unwrap().option_attributes(),
        Some("width=30")
    );
    assert_eq!(figures[1].second(), None);
    assert_eq!(links[0].class_text(), Some("myClass".into()));
    assert_eq!(links[0].package(), None);
    assert_eq!(links[1].package_text(), Some("methods".into()));
}

#[test]
fn producer_method_keeps_call_as_parent_sibling() {
    let document = parse(r"\usage{\method{print}{data.frame}(x, ...)}");
    let mut methods = Vec::new();
    for (node, path) in nodes(&document) {
        if let Some(view) = node.inspect_method(&path).unwrap() {
            methods.push(view);
        }
    }
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].generic(), "print");
    assert_eq!(methods[0].qualifier(), "data.frame");
    let usage = document
        .nodes()
        .iter()
        .find_map(|node| match node {
            RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Usage => Some(tagged.children()),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(usage, [RdNode::Tagged(_), RdNode::RCode(code)] if code.contains("(x, ...)") )
    );
}

#[test]
fn producer_rlike_figure_does_not_absorb_sibling_brace() {
    let document = parse(r"\usage{\figure{f}{o}}");
    let figures: Vec<_> = nodes(&document)
        .into_iter()
        .filter_map(|(node, path)| node.inspect_figure(&path).unwrap())
        .collect();
    assert_eq!(figures.len(), 1);
    assert_eq!(figures[0].file(), "f");
    assert_eq!(figures[0].second(), None);
    let usage = document
        .nodes()
        .iter()
        .find_map(|node| match node {
            RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Usage => Some(tagged.children()),
            _ => None,
        })
        .unwrap();
    assert!(matches!(usage, [RdNode::Tagged(_), RdNode::RCode(code)] if code.contains("{o}")));
}

#[test]
fn producer_zero_argument_symbols_keep_empty_brace_sibling() {
    let document = parse(r"\description{\R{} code \dots{} \ldots{}}");
    let symbols: Vec<_> = nodes(&document)
        .into_iter()
        .filter_map(|(node, path)| node.text_symbol(&path))
        .collect();
    assert_eq!(symbols.len(), 3);
    let description = document
        .nodes()
        .iter()
        .find_map(|node| match node {
            RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Description => {
                Some(tagged.children())
            }
            _ => None,
        })
        .unwrap();
    assert!(matches!(description, [
        RdNode::Tagged(r), RdNode::Tagged(empty_r), RdNode::Text(_),
        RdNode::Tagged(dots), RdNode::Tagged(empty_dots), RdNode::Text(_),
        RdNode::Tagged(ldots), RdNode::Tagged(empty_ldots)
    ] if r.tag() == &RdTag::R && r.children().is_empty()
        && empty_r.tag() == &RdTag::List && empty_r.children().is_empty()
        && dots.tag() == &RdTag::Dots && dots.children().is_empty()
        && empty_dots.tag() == &RdTag::List && empty_dots.children().is_empty()
        && ldots.tag() == &RdTag::LDots && ldots.children().is_empty()
        && empty_ldots.tag() == &RdTag::List && empty_ldots.children().is_empty()));
}

#[test]
fn generation_header_matches_realistic_preamble_shape() {
    let document = parse(
        "% Generated by roxygen2: do not edit by hand\n% Please edit documentation in R/foo.R\n\\name{x}\\title{t}",
    );
    assert!(matches!(
        document.nodes(),
        [
            RdNode::Comment(marker),
            RdNode::Text(first_newline),
            RdNode::Comment(source),
            RdNode::Text(second_newline),
            RdNode::Tagged(_),
            RdNode::Tagged(_),
        ] if marker == "% Generated by roxygen2: do not edit by hand"
            && first_newline == "\n"
            && source == "% Please edit documentation in R/foo.R"
            && second_newline == "\n"
    ));
    let view = document.generation_header().unwrap();
    assert!(view.is_generated());
    assert_eq!(view.source_files(), &["R/foo.R"]);
}

#[test]
fn generation_header_reads_wrapped_and_crlf_preambles() {
    let unix = parse("% Please edit documentation in R/a.R,\n%   R/b.R\n\\name{x}\\title{t}");
    assert_eq!(
        unix.generation_header().unwrap().source_files(),
        &["R/a.R", "R/b.R"]
    );

    let crlf = parse("% Please edit documentation in R/a.R,\r\n%   R/b.R\r\n\\name{x}\\title{t}");
    assert_eq!(crlf.generation_header(), unix.generation_header());
}

#[test]
fn plain_rd_has_no_generation_header() {
    assert!(parse(r"\name{x}\title{t}").generation_header().is_none());
}

#[test]
fn generation_header_accepts_unknown_generator_and_sources() {
    let document = parse(
        "% Generated by rdocgen: do not edit by hand\n% Please edit documentation in R/foo.R\n\\name{x}\\title{t}",
    );
    let view = document.generation_header().unwrap();
    assert_eq!(
        view.generator(),
        Some(&rd_ast::RdGenerator::Unknown("rdocgen".into()))
    );
    assert_eq!(view.source_files(), &["R/foo.R"]);
}

#[test]
fn lifecycle_badge_view_reads_real_rd() {
    let document = parse(
        r"\name{x}\title{t}\description{\ifelse{html}{\href{https://lifecycle.r-lib.org/articles/stages.html#stable}{\figure{lifecycle-stable.svg}{options: alt='[Stable]'}}}{\strong{[Stable]}}}",
    );
    let badges = document.inspect_lifecycle_badges();
    assert_eq!(badges.as_slice().len(), 1);
    let badge = badges.first().unwrap();
    assert_eq!(badge.stage(), &rd_ast::RdLifecycleStage::Stable);
    assert_eq!(badge.figure().file(), "lifecycle-stable.svg");
    assert_eq!(badge.canonical_shape().unwrap().fallback_text(), "Stable");
    assert!(badges.diagnostics().is_empty());

    let bare = parse(r"\name{x}\title{t}\description{\figure{lifecycle-stable.svg}}");
    let bare_badges = bare.lifecycle_badges();
    let badge = bare_badges.first().unwrap();
    assert!(badge.canonical_shape().is_none());
}

#[test]
fn producer_system_macro_view_uses_curated_tags() {
    let document = parse(r"\description{\doi{10.1/x} \CRANpkg{stats} \sspace \I{abc}}");
    let description = document
        .nodes()
        .iter()
        .find_map(|node| match node {
            RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Description => {
                Some(tagged.children())
            }
            _ => None,
        })
        .unwrap();
    let items: Vec<_> = rd_ast::RdSystemMacroItems::children(
        description,
        &RdPath::new(vec![rd_ast::RdPathSegment::TopLevel(0)]),
    )
    .collect();
    let macros: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            rd_ast::RdSystemMacroItem::Macro(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(macros.len(), 4);
    assert!(matches!(
        macros[0].semantic(),
        rd_ast::RdSystemMacro::Doi { id: "10.1/x" }
    ));
    assert!(matches!(
        macros[1].semantic(),
        rd_ast::RdSystemMacro::CranPkg { package: "stats" }
    ));
    assert!(matches!(
        macros[2].semantic(),
        rd_ast::RdSystemMacro::Sspace
    ));
    assert!(
        matches!(macros[3].semantic(), rd_ast::RdSystemMacro::I { body } if body == [RdNode::Text("abc".into())])
    );
    assert!(
        macros
            .iter()
            .all(|m| m.origin() == rd_ast::RdSystemMacroOrigin::CuratedTag && m.consumed() == 1)
    );
}
