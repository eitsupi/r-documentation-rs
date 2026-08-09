use crate::ParseError;
use rd_ast::{RdNode, RdTag};

#[test]
fn quoted_link_restores_string_state_after_nested_markup() {
    let parsed = crate::parse(br#"\examples{"\link{x} } %"}"#).unwrap();
    assert!(parsed.diagnostics().is_empty());
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        examples.children(),
        &[
            RdNode::RCode(r#"""#.into()),
            RdNode::tagged(
                RdTag::from_rd_tag(r"\link"),
                None,
                vec![RdNode::Text("x".into())]
            ),
            RdNode::RCode(r#" } %""#.into()),
        ]
    );
}

#[test]
fn quoted_non_whitelist_macro_is_literal_without_diagnostic() {
    let parsed = crate::parse(br#"\examples{"\emph{plain} \code{plain}"}"#).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r#""\emph{plain} \code{plain}""#.into())]
    );
}

#[test]
fn hash_after_raw_prefix_keeps_non_whitelist_macro_literal() {
    let parsed = crate::parse(br##"\examples{x <- r"# \emph{y}"}"##).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r##"x <- r"# \emph{y}""##.into())]
    );
}

#[test]
fn hash_after_raw_prefix_allows_quoted_link_macro() {
    let parsed = crate::parse(br##"\examples{x <- r"# \link{y}"}"##).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[
            RdNode::RCode(r##"x <- r"# "##.into()),
            RdNode::tagged(
                RdTag::from_rd_tag(r"\link"),
                None,
                vec![RdNode::Text("y".into())]
            ),
            RdNode::RCode(r#"""#.into()),
        ]
    );
}

#[test]
fn rlike_hash_comment_suppresses_macros_until_newline() {
    let parsed = crate::parse(
        br##"\examples{# \link{opaque}
\link{visible}}"##,
    )
    .unwrap();
    assert!(parsed.diagnostics().is_empty());
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        examples.children(),
        &[
            RdNode::RCode("# \\link{opaque}\n".into()),
            RdNode::tagged(
                RdTag::from_rd_tag(r"\link"),
                None,
                vec![RdNode::Text("visible".into())]
            ),
        ]
    );
}

#[test]
fn escaped_backslash_keeps_link_literal_inside_quote() {
    let parsed = crate::parse(br#"\examples{"\\link{x}"}"#).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r#""\link{x}""#.into())]
    );
}

#[test]
fn leaf_rules_preserve_boundaries_and_decode_without_flush() {
    let parsed = crate::parse(b"a\\%b\\{c\\}\n\n").unwrap();
    let nodes = parsed.document().nodes();
    assert_eq!(
        nodes,
        &[
            rd_ast::RdNode::Text("a%b{c}\n".into()),
            rd_ast::RdNode::Text("\n".into())
        ]
    );
}

#[test]
fn comments_keep_percent_and_exclude_following_newline() {
    let parsed = crate::parse(b"a% inside\nb").unwrap();
    assert_eq!(
        parsed.document().nodes(),
        &[
            RdNode::Text("a".into()),
            RdNode::Comment("% inside".into()),
            RdNode::Text("\n".into()),
            RdNode::Text("b".into()),
        ]
    );
}

#[test]
fn bare_raw_prefix_does_not_disable_comments() {
    let parsed = crate::parse(
        br"\usage{f(r% comment containing }
next()}",
    )
    .unwrap();
    let usage = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        usage.children(),
        &[
            RdNode::RCode("f(r".into()),
            RdNode::Comment("% comment containing }".into()),
            RdNode::RCode("\n".into()),
            RdNode::RCode("next()".into()),
        ]
    );

    let parsed = crate::parse(
        br##"\examples{r"(body % literal })"
next()}"##,
    )
    .unwrap();
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        examples.children(),
        &[
            RdNode::RCode("r\"(body % literal })\"\n".into()),
            RdNode::RCode("next()".into()),
        ]
    );
}

#[test]
fn method_has_groups_including_empty_slots() {
    let parsed = crate::parse(br"\usage{\method{}{}}").unwrap();
    let usage = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(usage.children().len(), 1);
    let method = usage.children()[0].as_tagged().unwrap();
    assert!(
        method.children()[0]
            .as_group()
            .unwrap()
            .children()
            .is_empty()
    );
    assert!(
        method.children()[1]
            .as_group()
            .unwrap()
            .children()
            .is_empty()
    );
}

#[test]
fn verbatim_flushes_each_line() {
    let parsed = crate::parse(b"\\preformatted{one\ntwo\nthree}").unwrap();
    let body = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        body.children(),
        &[
            RdNode::Verb("one\n".into()),
            RdNode::Verb("two\n".into()),
            RdNode::Verb("three".into()),
        ]
    );
}

#[test]
fn rlike_braces_are_literal_until_argument_close() {
    let parsed = crate::parse(br"\usage{f({1})}").unwrap();
    let usage = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(usage.children(), &[RdNode::RCode("f({1})".into())]);
}

#[test]
fn rlike_comment_counts_balanced_and_escaped_braces() {
    let parsed = crate::parse(
        br"\examples{
# note {grp} \link{x}
# note \{escaped\} {real}
x <- 1
}",
    )
    .unwrap();
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        examples.children(),
        &[
            RdNode::RCode("\n".into()),
            RdNode::RCode("# note {grp} \\link{x}\n".into()),
            RdNode::RCode("# note {escaped} {real}\n".into()),
            RdNode::RCode("x <- 1\n".into()),
        ]
    );
}

#[test]
fn rlike_comment_owning_brace_closes_argument_and_raw_prefix_is_literal() {
    let parsed = crate::parse(
        br"\examples{
\dontshow{x <- 1 # the whole list}
r# comment \link{x} {literal}
}",
    )
    .unwrap();
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        examples.children(),
        &[
            RdNode::RCode("\n".into()),
            RdNode::tagged(
                RdTag::DontShow,
                None,
                vec![RdNode::RCode("x <- 1 # the whole list".into())],
            ),
            RdNode::RCode("\n".into()),
            RdNode::RCode("r# comment \\link{x} {literal}\n".into()),
        ]
    );
}

#[test]
fn option_presence_distinguishes_absent_empty_and_nonempty() {
    let parsed = crate::parse(br"\link{a}\link[]{b}\link[pkg]{c}").unwrap();
    let nodes = parsed.document().nodes();
    assert_eq!(nodes[0].as_tagged().unwrap().option(), None);
    assert_eq!(
        nodes[1].as_tagged().unwrap().option(),
        Some(&[] as &[RdNode])
    );
    assert_eq!(
        nodes[2].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("pkg".into())][..])
    );
}

#[test]
fn option_requires_tight_adjacency() {
    let parsed = crate::parse(br"\link [pkg]{target}").unwrap();
    let nodes = parsed.document().nodes();
    assert_eq!(nodes[0].as_tagged().unwrap().option(), None);
    // The root-level bare group is stripped by recovery (CONTRACT §6 rule
    // 5), so only the retained contents follow the literal bracket text.
    assert_eq!(
        nodes[1..],
        [RdNode::Text(" [pkg]".into()), RdNode::Text("target".into())]
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code() == &crate::DiagnosticCode::UnexpectedOpeningDelimiter)
    );
}

#[test]
fn sexpr_option_is_text_and_body_is_rcode() {
    let parsed = crate::parse(br"\Sexpr[stage=render]{x + 1}").unwrap();
    let sexpr = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        sexpr.option(),
        Some(&[RdNode::Text("stage=render".into())][..])
    );
    assert_eq!(sexpr.children(), &[RdNode::RCode("x + 1".into())]);
}

#[test]
fn unsupported_encoding_is_hard_error_with_argument_span() {
    let source = br"\encoding{latin1}";
    let error = crate::parse(source).unwrap_err();
    let ParseError::UnsupportedEncoding { name, span } = error else {
        panic!("expected unsupported encoding error");
    };
    assert_eq!(name, "latin1");
    assert_eq!(span.unwrap().bytes(), 10..16);
}

#[test]
fn section_arguments_are_ungrouped_positional_groups() {
    let parsed = crate::parse(br"\section{title}{\emph{body}}").unwrap();
    let section = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(section.children().len(), 2);
    assert_eq!(
        section.children()[0].as_group().unwrap().children().len(),
        1
    );
    assert_eq!(
        section.children()[1].as_group().unwrap().children().len(),
        1
    );
}

#[test]
fn equation_optional_argument_is_consumed_only_when_braced() {
    let parsed = crate::parse(br"\eqn{x} text \eqn{x}{y} \deqn{x}{y}").unwrap();
    let tags: Vec<_> = parsed
        .document()
        .nodes()
        .iter()
        .filter_map(|node| node.as_tagged())
        .collect();
    assert_eq!(tags[0].children().len(), 1);
    assert_eq!(tags[1].children().len(), 2);
    assert_eq!(tags[2].children().len(), 2);
}

#[test]
fn zero_argument_tags_do_not_consume_following_content() {
    let parsed = crate::parse(br"\tab cell \cr next").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[]
    );
    assert_eq!(parsed.document().nodes()[1], RdNode::Text(" cell ".into()));
}

#[test]
fn system_macros_have_first_class_semantics_in_latex() {
    let parsed = crate::parse(br"\description{\CRANpkg{stats}\sspace\I{x}}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    let children = parsed.document().nodes()[0].as_tagged().unwrap().children();
    assert_eq!(children[0].as_tagged().unwrap().tag(), &RdTag::CranPkg);
    assert_eq!(
        children[0].as_tagged().unwrap().children(),
        &[RdNode::Text("stats".into())]
    );
    assert_eq!(children[1].as_tagged().unwrap().tag(), &RdTag::Sspace);
    assert!(children[1].as_tagged().unwrap().children().is_empty());
    assert_eq!(children[2].as_tagged().unwrap().tag(), &RdTag::I);
    assert_eq!(
        children[2].as_tagged().unwrap().children(),
        &[RdNode::Text("x".into())]
    );
}

#[test]
fn system_macros_have_contextual_semantics_in_rlike() {
    let parsed = crate::parse(br"\usage{f(\CRANpkg{stats}, \sspace, \I{x})}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    let children = parsed.document().nodes()[0].as_tagged().unwrap().children();
    assert_eq!(children[1].as_tagged().unwrap().tag(), &RdTag::CranPkg);
    assert_eq!(
        children[1].as_tagged().unwrap().children(),
        &[RdNode::Text("stats".into())]
    );
    assert_eq!(children[3].as_tagged().unwrap().tag(), &RdTag::Sspace);
    assert!(children[3].as_tagged().unwrap().children().is_empty());
    assert_eq!(children[5].as_tagged().unwrap().tag(), &RdTag::I);
    assert_eq!(
        children[5].as_tagged().unwrap().children(),
        &[RdNode::RCode("x".into())]
    );
}

#[test]
fn system_macro_context_and_surplus_group_rules_are_pinned() {
    let parsed = crate::parse(br"\I{x}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().tag(),
        &RdTag::I
    );

    for source in [br"\CRANpkg{stats}".as_slice(), br"\sspace".as_slice()] {
        let parsed = crate::parse(source).unwrap();
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|d| d.code() == &crate::DiagnosticCode::TagNotAllowedHere)
        );
        assert!(matches!(parsed.document().nodes()[0], RdNode::Tagged(_)));
    }

    let parsed = crate::parse(br"\description{\CRANpkg{x}{y}\I{x}{y}\sspace{}}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    let children = parsed.document().nodes()[0].as_tagged().unwrap().children();
    assert_eq!(
        children
            .iter()
            .filter(|node| node.as_tagged().is_some())
            .count(),
        6
    );
    assert_eq!(
        children
            .iter()
            .filter(|node| node
                .as_tagged()
                .is_some_and(|tag| tag.tag() == &RdTag::List))
            .count(),
        3
    );
}

#[test]
fn sspace_surplus_group_stays_rlike_parent_code() {
    for source in [
        br"\usage{f(\sspace{})}".as_slice(),
        br"\usage{f(\sspace{} + 1)}".as_slice(),
    ] {
        let parsed = crate::parse(source).unwrap();
        assert!(parsed.diagnostics().is_empty());
        let children = parsed.document().nodes()[0].as_tagged().unwrap().children();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0], RdNode::RCode("f(".into()));
        assert_eq!(children[1].as_tagged().unwrap().tag(), &RdTag::Sspace);
        assert!(children[1].as_tagged().unwrap().children().is_empty());
        assert!(matches!(&children[2], RdNode::RCode(code) if code.contains("{}")));
        assert!(!children.iter().any(|node| {
            node.as_tagged()
                .is_some_and(|tag| tag.tag() == &RdTag::List)
        }));
    }
}

#[test]
fn every_known_macro_has_a_tag_spec_except_non_macro_tags() {
    for tag in RdTag::KNOWN {
        let spelling = tag.as_rd_tag();
        if matches!(spelling, "LIST" | "#ifdef" | "#ifndef") {
            assert!(
                super::spec::tag_spec(spelling, super::spec::Context::Document).is_none(),
                "{spelling}"
            );
        } else {
            assert!(
                super::spec::tag_spec(spelling, super::spec::Context::Document).is_some(),
                "missing TagSpec for {spelling}"
            );
        }
    }
}

#[test]
fn known_and_unknown_tags_have_distinct_diagnostics_and_nodes() {
    let known = crate::parse(br"\description{\samp{x}}").unwrap();
    assert!(
        known
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() != &crate::DiagnosticCode::UnknownTag)
    );

    let unknown = crate::parse(br"\description{\madeUpMacro{x}}").unwrap();
    assert!(
        unknown
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::UnknownTag)
    );
    assert_eq!(
        unknown.document().nodes()[0]
            .as_tagged()
            .unwrap()
            .children()[0]
            .as_tagged()
            .unwrap()
            .tag(),
        &RdTag::Unknown(r"\madeUpMacro".into())
    );
}

#[test]
// CONTRACT.md §13 class 3: general user macros lose association and have no v1 macro environment.
fn recovery_newcommand_is_unknown_and_does_not_define_a_macro() {
    let parsed = crate::parse(
        br"\newcommand{\mymac}{\emph{#1}}
\description{\mymac{a}}",
    )
    .unwrap();
    assert_eq!(parsed.diagnostics().len(), 3);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::UnknownTag)
            .count(),
        3
    );
    assert_eq!(
        parsed.document().nodes(),
        &[
            RdNode::tagged(
                RdTag::Unknown(r"\newcommand".into()),
                None,
                vec![RdNode::tagged(
                    RdTag::Unknown(r"\mymac".into()),
                    None,
                    vec![],
                )],
            ),
            RdNode::tagged(
                RdTag::List,
                None,
                vec![RdNode::tagged(
                    RdTag::Emph,
                    None,
                    vec![RdNode::Text("#1".into())],
                )],
            ),
            RdNode::Text("\n".into()),
            RdNode::tagged(
                RdTag::Description,
                None,
                vec![RdNode::tagged(
                    RdTag::Unknown(r"\mymac".into()),
                    None,
                    vec![RdNode::Text("a".into())],
                )],
            ),
        ]
    );
}

#[test]
// CONTRACT.md §13 class 3: general user macros lose association and have no v1 macro environment.
fn recovery_renewcommand_is_unknown_without_a_macro_environment() {
    let parsed = crate::parse(br"\renewcommand{\mymac}{\emph{#1}}").unwrap();
    assert_eq!(parsed.diagnostics().len(), 2);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::UnknownTag)
            .count(),
        2
    );
    assert_eq!(
        parsed.document().nodes(),
        &[
            RdNode::tagged(
                RdTag::Unknown(r"\renewcommand".into()),
                None,
                vec![RdNode::tagged(
                    RdTag::Unknown(r"\mymac".into()),
                    None,
                    vec![],
                )],
            ),
            RdNode::tagged(
                RdTag::List,
                None,
                vec![RdNode::tagged(
                    RdTag::Emph,
                    None,
                    vec![RdNode::Text("#1".into())],
                )],
            ),
        ]
    );
}

#[test]
// CONTRACT.md §13 class 3: general user macros lose association and have no v1 macro environment.
fn recovery_unknown_macro_keeps_only_first_argument_as_invocation_child() {
    let parsed = crate::parse(br"\unknownmac{a}{b}{c}").unwrap();
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::UnknownTag)
            .count(),
        1
    );
    assert_eq!(
        parsed.document().nodes(),
        &[
            RdNode::tagged(
                RdTag::Unknown(r"\unknownmac".into()),
                None,
                vec![RdNode::Text("a".into())],
            ),
            RdNode::tagged(RdTag::List, None, vec![RdNode::Text("b".into())]),
            RdNode::tagged(RdTag::List, None, vec![RdNode::Text("c".into())]),
        ]
    );
}

#[test]
// CONTRACT.md §13 class 3: general user macros lose association and have no v1 macro environment.
fn recovery_unknown_macro_keeps_later_arguments_as_list_siblings_in_a_section() {
    let parsed = crate::parse(br"\description{before \unknownmac{a}{b}{c} after}").unwrap();
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::UnknownTag)
            .count(),
        1
    );
    assert_eq!(
        parsed.document().nodes(),
        &[RdNode::tagged(
            RdTag::Description,
            None,
            vec![
                RdNode::Text("before ".into()),
                RdNode::tagged(
                    RdTag::Unknown(r"\unknownmac".into()),
                    None,
                    vec![RdNode::Text("a".into())],
                ),
                RdNode::tagged(RdTag::List, None, vec![RdNode::Text("b".into())]),
                RdNode::tagged(RdTag::List, None, vec![RdNode::Text("c".into())]),
                RdNode::Text(" after".into()),
            ],
        )]
    );
}

fn assert_error(
    parsed: &crate::Parsed,
    code: crate::DiagnosticCode,
    bytes: std::ops::Range<usize>,
) {
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(parsed.diagnostics()[0].code(), &code);
    assert_eq!(parsed.diagnostics()[0].severity(), &crate::Severity::Error);
    assert_eq!(parsed.diagnostics()[0].span().bytes(), bytes);
}

#[test]
fn recovery_preserves_unclosed_group_and_nested_opener_spans() {
    let parsed = crate::parse(br"\description{").unwrap();
    assert_error(&parsed, crate::DiagnosticCode::UnclosedGroup, 12..13);
    let parsed = crate::parse(br"\description{{x").unwrap();
    assert_eq!(parsed.diagnostics().len(), 2);
    assert_eq!(parsed.diagnostics()[0].span().bytes(), 13..14);
    assert_eq!(parsed.diagnostics()[1].span().bytes(), 12..13);
}

#[test]
fn recovery_closes_options_at_argument_openers_and_eof() {
    let parsed = crate::parse(br"\link[unclosed").unwrap();
    assert_eq!(parsed.diagnostics().len(), 2);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnclosedOption
    );
    assert_eq!(parsed.diagnostics()[0].span().bytes(), 5..6);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("unclosed".into())][..])
    );
    let parsed = crate::parse(br"\link[pkg{target}").unwrap();
    assert_eq!(
        parsed.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnclosedOption
    );
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("pkg".into())][..])
    );
}

#[test]
fn recovery_drops_stray_closers_and_strips_root_group() {
    let parsed = crate::parse(b"}tail").unwrap();
    assert_error(
        &parsed,
        crate::DiagnosticCode::UnexpectedClosingDelimiter,
        0..1,
    );
    assert_eq!(parsed.document().nodes(), &[RdNode::Text("tail".into())]);
    let parsed = crate::parse(b"{stripped}").unwrap();
    assert_error(
        &parsed,
        crate::DiagnosticCode::UnexpectedOpeningDelimiter,
        0..1,
    );
    assert_eq!(
        parsed.document().nodes(),
        &[RdNode::Text("stripped".into())]
    );
}

#[test]
fn surplus_group_after_max_arity_is_a_silent_list_sibling() {
    let parsed = crate::parse(br"\eqn{a}{b}{c}").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    let nodes = parsed.document().nodes();
    assert_eq!(nodes[0].as_tagged().unwrap().children().len(), 2);
    assert_eq!(
        nodes[1],
        RdNode::tagged(RdTag::List, None, vec![RdNode::Text("c".into())])
    );
}

#[test]
fn unclosed_option_synchronizes_at_section_level_macro() {
    let parsed = crate::parse(b"\\link[bad\n\\name{x}").unwrap();
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code() == &crate::DiagnosticCode::UnclosedOption)
    );
    let nodes = parsed.document().nodes();
    assert_eq!(
        nodes[0].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("bad\n".into())][..])
    );
    let name = nodes
        .iter()
        .filter_map(RdNode::as_tagged)
        .find(|tagged| tagged.tag() == &RdTag::Name)
        .expect(r"\name must be preserved at document level");
    assert_eq!(name.children(), &[RdNode::Verb("x".into())]);
}

#[test]
fn unclosed_option_synchronizes_at_title_and_section() {
    let parsed = crate::parse(b"\\link[bad\n\\title{x}").unwrap();
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code() == &crate::DiagnosticCode::UnclosedOption)
    );
    let nodes = parsed.document().nodes();
    assert_eq!(
        nodes[0].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("bad\n".into())][..])
    );
    let title = nodes
        .iter()
        .filter_map(RdNode::as_tagged)
        .find(|tagged| tagged.tag() == &RdTag::Title)
        .expect(r"\title must be preserved at document level");
    assert_eq!(title.children(), &[RdNode::Text("x".into())]);

    let parsed = crate::parse(b"\\link[bad\n\\section{title}{body}").unwrap();
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code() == &crate::DiagnosticCode::UnclosedOption)
    );
    let nodes = parsed.document().nodes();
    assert_eq!(
        nodes[0].as_tagged().unwrap().option(),
        Some(&[RdNode::Text("bad\n".into())][..])
    );
    let section = nodes
        .iter()
        .filter_map(RdNode::as_tagged)
        .find(|tagged| tagged.tag() == &RdTag::Section)
        .expect(r"\section must be preserved at document level");
    assert_eq!(section.children().len(), 2);
    assert_eq!(
        section.children()[0],
        RdNode::group(vec![RdNode::Text("title".into())])
    );
    assert_eq!(
        section.children()[1],
        RdNode::group(vec![RdNode::Text("body".into())])
    );
}

#[test]
fn nested_option_does_not_synchronize_at_document_only_macro() {
    let parsed = crate::parse(br"\description{\link[a\name{b}]{c}}").unwrap();
    assert!(
        parsed
            .diagnostics()
            .iter()
            .all(|d| d.code() != &crate::DiagnosticCode::UnclosedOption)
    );
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|d| d.code() == &crate::DiagnosticCode::TagNotAllowedHere)
    );
    let description = parsed.document().nodes()[0].as_tagged().unwrap();
    let link = description.children()[0].as_tagged().unwrap();
    assert_eq!(
        link.option(),
        Some(
            &[
                RdNode::Text("a".into()),
                RdNode::tagged(RdTag::Name, None, vec![RdNode::Verb("b".into())])
            ][..]
        )
    );
    assert_eq!(link.children(), &[RdNode::Text("c".into())]);
}

#[test]
fn stripped_root_group_contents_keep_document_context() {
    let parsed = crate::parse(br"{\name{x}}").unwrap();
    assert_error(
        &parsed,
        crate::DiagnosticCode::UnexpectedOpeningDelimiter,
        0..1,
    );
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().tag(),
        &RdTag::Name
    );
}

#[test]
fn root_groups_after_sections_are_also_stripped() {
    let parsed = crate::parse(b"\\name{x}\n{stripped}").unwrap();
    assert_error(
        &parsed,
        crate::DiagnosticCode::UnexpectedOpeningDelimiter,
        9..10,
    );
    assert_eq!(
        &parsed.document().nodes()[2..],
        &[RdNode::Text("stripped".into())]
    );
}

#[test]
fn recovery_preserves_unknown_tags_in_latex_and_rlike() {
    let parsed = crate::parse(br"\foo{arg}").unwrap();
    assert_error(&parsed, crate::DiagnosticCode::UnknownTag, 0..4);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().tag(),
        &RdTag::Unknown(r"\foo".into())
    );
    let parsed = crate::parse(br"\usage{\foo}").unwrap();
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnknownTag
    );
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children()[0]
            .as_tagged()
            .unwrap()
            .tag(),
        &RdTag::Unknown(r"\foo".into())
    );
}

#[test]
fn recovery_retains_known_tag_in_wrong_context() {
    let parsed = crate::parse(br"\description{\name{x}}").unwrap();
    assert_error(&parsed, crate::DiagnosticCode::TagNotAllowedHere, 13..18);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children()[0]
            .as_tagged()
            .unwrap()
            .tag(),
        &RdTag::Name
    );
}

#[test]
fn special_at_document_top_level_is_diagnosed_and_retained() {
    let parsed = crate::parse(br"\special{date + x}").unwrap();
    assert_error(&parsed, crate::DiagnosticCode::TagNotAllowedHere, 0..8);
    assert_eq!(
        parsed.document().nodes(),
        &[RdNode::tagged(
            RdTag::Special,
            None,
            vec![RdNode::RCode("date + x".into())],
        )]
    );
}

#[test]
fn item_uses_two_groups_in_arguments_and_describe() {
    let parsed =
        crate::parse(br"\arguments{\item{x}{description}}\describe{\item{y}{details}}").unwrap();
    for container in parsed
        .document()
        .nodes()
        .iter()
        .filter_map(RdNode::as_tagged)
    {
        let item = container.children()[0].as_tagged().unwrap();
        assert_eq!(item.tag(), &RdTag::Item);
        assert_eq!(item.children().len(), 2);
        assert!(
            item.children()
                .iter()
                .all(|child| child.as_group().is_some())
        );
    }
}

#[test]
fn item_in_itemize_is_a_marker_before_a_list_group() {
    let parsed = crate::parse(br"\itemize{\item{first item}}").unwrap();
    let itemize = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        itemize.children()[0].as_tagged().unwrap().tag(),
        &RdTag::Item
    );
    assert_eq!(itemize.children()[0].as_tagged().unwrap().children(), &[]);
    assert_eq!(
        itemize.children()[1],
        RdNode::tagged(RdTag::List, None, vec![RdNode::Text("first item".into())])
    );
}

#[test]
fn value_item_keeps_surrounding_text_as_siblings() {
    let parsed = crate::parse(br"\value{lead \item{x}{description} tail}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    let value = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(value.children()[0], RdNode::Text("lead ".into()));
    assert_eq!(value.children()[1].as_tagged().unwrap().tag(), &RdTag::Item);
    assert_eq!(value.children()[1].as_tagged().unwrap().children().len(), 2);
    assert_eq!(value.children()[2], RdNode::Text(" tail".into()));
}

#[test]
fn item_policy_restores_after_nested_itemize() {
    let parsed = crate::parse(br"\value{\itemize{\item inner}\item{x}{result}}").unwrap();
    assert!(parsed.diagnostics().is_empty());
    let value = parsed.document().nodes()[0].as_tagged().unwrap();
    let itemize = value.children()[0].as_tagged().unwrap();
    assert_eq!(itemize.children()[0].as_tagged().unwrap().children(), &[]);
    assert_eq!(itemize.children()[1], RdNode::Text(" inner".into()));
    assert_eq!(value.children()[1].as_tagged().unwrap().tag(), &RdTag::Item);
    assert_eq!(value.children()[1].as_tagged().unwrap().children().len(), 2);
}

#[test]
fn item_inside_inline_latex_is_unknown() {
    let parsed = crate::parse(br"\value{\emph{\item{x}{description}}}").unwrap();
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnknownTag
    );
    let emph = parsed.document().nodes()[0].as_tagged().unwrap().children()[0]
        .as_tagged()
        .unwrap();
    assert_eq!(emph.children()[0], RdNode::Text(r"\item".into()));
    assert_eq!(emph.children()[1].as_tagged().unwrap().tag(), &RdTag::List);
    assert_eq!(emph.children()[2].as_tagged().unwrap().tag(), &RdTag::List);
}

#[test]
fn href_has_verbatim_url_and_latex_display_groups() {
    let parsed = crate::parse(br"\href{https://example.org}{the site}").unwrap();
    let href = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(href.children().len(), 2);
    assert_eq!(
        href.children()[0],
        RdNode::group(vec![RdNode::Verb("https://example.org".into())])
    );
    assert_eq!(
        href.children()[1],
        RdNode::group(vec![RdNode::Text("the site".into())])
    );
}

#[test]
fn equation_comment_is_reinterpreted_for_brace_termination() {
    let parsed = crate::parse(br"\eqn{a%b}").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::group(vec![RdNode::Verb("a%b".into())])]
    );
}

#[test]
fn quoted_r_code_braces_are_literal() {
    let parsed = crate::parse(br##"\code{f("}")}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r#"f("}")"#.into())]
    );
}

#[test]
fn raw_strings_preserve_boundaries_and_backslash_escapes() {
    let parsed =
        crate::parse(br##"\code{r"---(body " } ] ' \" % \} near )x)---"; after()}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(
            r##"r"---(body " } ] ' \" % \} near )x)---"; after()"##.into()
        )]
    );
}

#[test]
fn raw_strings_close_after_nested_parens() {
    let parsed = crate::parse(br##"\code{r"((a))"}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r##"r"((a))""##.into())]
    );
}

#[test]
fn raw_strings_close_after_dash_qualified_prefix_sharing_near_closers() {
    let parsed = crate::parse(br##"\code{r"---(\1--)-)---"}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r##"r"---(\1--)-)---""##.into())]
    );
}

#[test]
fn raw_strings_reexamine_byte_after_failed_partial_closer() {
    let parsed = crate::parse(br##"\code{r"---(near )x)---"}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r##"r"---(near )x)---""##.into())]
    );
}

#[test]
fn single_quote_raw_strings_preserve_boundaries_and_mixed_delimiters() {
    for source in [
        r#"\code{r'(body } ] ) " % \ \} near )x)'; after()}"#,
        r#"\code{r'---(near )x)---'; after()}"#,
        r#"\code{R'-[upper ] ) " % \ \} x]-'; after()}"#,
        r#"\code{r'(a)"b)'}"#,
        r#"\code{r"(a)'b)"}"#,
    ] {
        let parsed = crate::parse(source.as_bytes()).unwrap();
        assert_eq!(parsed.diagnostics(), &[]);
        let expected = source
            .strip_prefix(r"\code{")
            .unwrap()
            .strip_suffix('}')
            .unwrap();
        assert_eq!(
            parsed.document().nodes()[0].as_tagged().unwrap().children(),
            &[RdNode::RCode(expected.into())]
        );
    }
}

#[test]
fn backtick_after_raw_prefix_is_an_ordinary_quote() {
    let parsed = crate::parse(br"\code{r`100%`}").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode("r`100%`".into())]
    );
}

#[test]
fn long_raw_strings_do_not_consume_relex_budget() {
    let body = "x".repeat(100_000);
    let input = format!(r#"\code{{r"---({})---"; after()}}"#, body);
    let parsed = crate::parse(input.as_bytes()).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(format!(r#"r"---({})---"; after()"#, body))]
    );
}

// Behavior pinned against tools::parse_Rd from R 4.6.1.
#[test]
fn rlike_raw_opener_does_not_cross_newlines_or_structural_markup() {
    let parsed = crate::parse(
        br"\usage{r
(x)}",
    )
    .unwrap();
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode("r\n".into()), RdNode::RCode("(x)".into())]
    );

    let parsed = crate::parse(
        br####"\usage{r"
(x)"}"####,
    )
    .unwrap();
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[
            RdNode::RCode("r\"\n".into()),
            RdNode::RCode(r#"(x)""#.into())
        ]
    );

    let parsed = crate::parse(
        br####"\usage{r"(x
y)"}"####,
    )
    .unwrap();
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode("r\"(x\ny)\"".into())]
    );

    let parsed = crate::parse(br"\usage{r\emph{tag}(x)}").unwrap();
    let usage = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(usage.children()[0], RdNode::RCode("r".into()));
    assert_eq!(
        usage.children()[1],
        RdNode::tagged(RdTag::Emph, None, vec![RdNode::Text("tag".into())])
    );
    assert_eq!(usage.children()[2], RdNode::RCode("(x)".into()));
}

#[test]
fn encoding_declarations_with_markup_are_unsupported() {
    let error = crate::parse(br"\encoding{\emph{latin1}UTF-8}").unwrap_err();
    let ParseError::UnsupportedEncoding { name, .. } = error else {
        panic!("expected unsupported encoding error");
    };
    assert_eq!(name, r"\emph{latin1}UTF-8");
}

#[test]
fn unclosed_encoding_spans_stay_ordered() {
    let error = crate::parse(br"\encoding{").unwrap_err();
    let ParseError::UnsupportedEncoding { span, .. } = error else {
        panic!("expected unsupported encoding error");
    };
    let span = span.unwrap();
    assert_eq!(span.bytes(), 10..10);
}

#[test]
fn comment_token_suffix_after_terminating_brace_is_preserved() {
    let parsed = crate::parse(br"\eqn{a%b}{c} tail").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    let equation = parsed.document().nodes()[0].as_tagged().unwrap();
    assert_eq!(
        equation.children(),
        &[
            RdNode::group(vec![RdNode::Verb("a%b".into())]),
            RdNode::group(vec![RdNode::Verb("c".into())]),
        ]
    );
    assert_eq!(parsed.document().nodes()[1], RdNode::Text(" tail".into()));
}

#[test]
fn quoted_percent_keeps_trailing_parent_content() {
    let parsed = crate::parse(br##"\code{"100%"} tail"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r#""100%""#.into())]
    );
    assert_eq!(parsed.document().nodes()[1], RdNode::Text(" tail".into()));
}

#[test]
fn percent_inside_quoted_r_string_stays_literal() {
    let parsed = crate::parse(br##"\code{paste("100%", x)}"##).unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::RCode(r#"paste("100%", x)"#.into())]
    );
}

#[test]
fn nesting_limit_rejects_deep_input_without_partial_document() {
    let input = format!("{}x{}", "{".repeat(4_000), "}".repeat(4_000));
    let error = crate::parse(input.as_bytes()).unwrap_err();
    let ParseError::NestingLimitExceeded { span } = error else {
        panic!("expected nesting limit error");
    };
    assert_eq!(
        span.bytes(),
        crate::parser::MAX_FRAME_DEPTH - 1..crate::parser::MAX_FRAME_DEPTH
    );
}

#[test]
fn input_size_limit_is_checked_before_lexing() {
    let input = vec![b'x'; crate::MAX_INPUT_SIZE + 1];
    assert_eq!(crate::parse(&input), Err(ParseError::InputTooLarge));
}

#[test]
fn equation_percent_relex_budget_has_a_realistic_boundary() {
    let moderate = format!(r"\eqn{{{}x}}", "%".repeat(100));
    let parsed = crate::parse(moderate.as_bytes()).unwrap();
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::group(vec![RdNode::Verb(format!(
            "{}x",
            "%".repeat(100)
        ))])]
    );

    let pathological = format!(r"\eqn{{{}x}}", "%".repeat(2_000));
    assert_eq!(
        crate::parse(pathological.as_bytes()),
        Err(ParseError::InputTooLarge)
    );
}

#[test]
fn equation_percent_relex_budget_counts_token_shifts() {
    let input = format!(r"\eqn{{{}x}}{}", "%a".repeat(200), "\n".repeat(40_000));
    assert_eq!(
        crate::parse(input.as_bytes()),
        Err(ParseError::InputTooLarge)
    );
}

#[test]
fn equation_percent_relex_budget_ignores_shiftless_splices() {
    let input = format!(r"\eqn{{{}}}", "%text\n".repeat(600));
    assert!(crate::parse(input.as_bytes()).is_ok());
}

#[test]
fn deterministic_property_inputs_never_panic_or_produce_raw_nodes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rd");
    // read_dir order is unspecified; sort so the selected seeds (and any
    // failure) reproduce identically across filesystems.
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    let seeds: Vec<_> = paths
        .iter()
        .take(8)
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    let mut rng = XorShift64(0x6b5f_1d2a_9e37_79b9);
    for case in 0..320 {
        let input = match case % 3 {
            0 => {
                let length = (rng.next() as usize % 160) + 1;
                random_bytes(&mut rng, length)
            }
            1 => {
                let length = (rng.next() as usize % 220) + 1;
                random_ascii(&mut rng, length)
            }
            _ => {
                let source = &seeds[case % seeds.len()];
                mutate_fixture(source, &mut rng, case)
            }
        };
        let result = std::panic::catch_unwind(|| crate::parse(&input));
        assert!(result.is_ok(), "parser panicked for case {case}");
        match result.unwrap() {
            Err(error) => assert!(matches!(
                error,
                ParseError::InvalidUtf8 { .. }
                    | ParseError::NulByte { .. }
                    | ParseError::UnsupportedEncoding { .. }
                    | ParseError::InputTooLarge
                    | ParseError::NestingLimitExceeded { .. }
            )),
            Ok(parsed) => {
                for diagnostic in parsed.diagnostics() {
                    let span = diagnostic.span();
                    assert!(span.bytes().start <= span.bytes().end);
                    assert!(span.bytes().end <= input.len());
                    assert!(span.start().line() >= 1 && span.start().column() >= 1);
                    assert!(span.end().line() >= 1 && span.end().column() >= 1);
                }
                walk_nodes(parsed.document().nodes());
            }
        }
    }
}

struct XorShift64(u64);
impl XorShift64 {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 7;
        self.0 ^= self.0 >> 9;
        self.0 ^= self.0 << 8;
        self.0
    }
}

fn random_bytes(rng: &mut XorShift64, length: usize) -> Vec<u8> {
    (0..length).map(|_| rng.next() as u8).collect()
}

fn random_ascii(rng: &mut XorShift64, length: usize) -> Vec<u8> {
    const SPECIAL: &[u8] = b"\\{}[]%\"\n\r";
    (0..length)
        .map(|_| {
            if rng.next().is_multiple_of(3) {
                SPECIAL[rng.next() as usize % SPECIAL.len()]
            } else {
                b' ' + (rng.next() % 95) as u8
            }
        })
        .collect()
}

fn mutate_fixture(source: &[u8], rng: &mut XorShift64, case: usize) -> Vec<u8> {
    let mut output = source.to_vec();
    match (case / 3) % 3 {
        0 => output.truncate((rng.next() as usize) % (output.len() + 1)),
        1 => {
            if !output.is_empty() {
                let index = rng.next() as usize % output.len();
                output[index] ^= 1 << (rng.next() % 8);
            }
        }
        _ => {
            let start = rng.next() as usize % (output.len() + 1);
            let end = start + (rng.next() as usize % (output.len() - start + 1));
            output.extend_from_within(start..end);
        }
    }
    output
}

fn walk_nodes(nodes: &[RdNode]) {
    for node in nodes {
        match node {
            RdNode::Text(value)
            | RdNode::RCode(value)
            | RdNode::Verb(value)
            | RdNode::Comment(value) => {
                let _ = value.len();
            }
            RdNode::Tagged(tagged) => {
                let _ = tagged.tag().as_rd_tag().len();
                if let Some(option) = tagged.option() {
                    walk_nodes(option);
                }
                walk_nodes(tagged.children());
            }
            RdNode::Group(group) => walk_nodes(group.children()),
            RdNode::Raw(_) => panic!("Raw node in parser output"),
            _ => {}
        }
    }
}

#[test]
fn controls_are_literal_in_verbatim_and_equation() {
    let parsed = crate::parse(br"\preformatted{\foo} \eqn{\bar}{\baz}").unwrap();
    assert_eq!(parsed.diagnostics(), &[]);
    assert_eq!(
        parsed.document().nodes()[0].as_tagged().unwrap().children(),
        &[RdNode::Verb(r"\foo".into())]
    );
    let equation = parsed.document().nodes()[2].as_tagged().unwrap();
    assert_eq!(
        equation.children()[0],
        RdNode::group(vec![RdNode::Verb(r"\bar".into())])
    );
    assert_eq!(
        equation.children()[1],
        RdNode::group(vec![RdNode::Verb(r"\baz".into())])
    );
}

#[test]
fn conditional_recovery_warns_and_keeps_parsing() {
    let missing = crate::parse(b"\\description{\n#ifdef unix\ninside\n}").unwrap();
    assert!(
        missing
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == &crate::DiagnosticCode::MissingEndIf)
    );

    let stray = crate::parse(b"\\description{\n#endif ignored } garbage\nafter\n}").unwrap();
    assert_eq!(stray.diagnostics().len(), 1);
    assert_eq!(
        stray.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnexpectedEndIf
    );
    assert!(stray.document().nodes().iter().any(|node| {
        matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Description)
    }));
}

#[test]
fn conditional_body_discards_stray_closing_delimiter_in_examples() {
    let parsed = crate::parse(b"\\examples{\n#ifdef unix\n}\nx <- 1\n#endif\ny <- 2\n}").unwrap();
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.severity() == &crate::Severity::Warning)
            .count(),
        1
    );
    let examples = parsed.document().nodes()[0].as_tagged().unwrap();
    let conditional = examples.children()[1].as_tagged().unwrap();
    assert_eq!(
        conditional.children()[1].as_group().unwrap().children(),
        &[RdNode::RCode("\n".into()), RdNode::RCode("x <- 1\n".into())]
    );
    assert!(
        examples
            .children()
            .iter()
            .any(|node| { matches!(node, RdNode::RCode(value) if value == "y <- 2\n") })
    );
}

#[test]
fn conditional_body_discards_stray_closing_delimiter_at_document_level() {
    let parsed = crate::parse(b"#ifdef unix\n}\n\\alias{y}\n#endif\n\\alias{z}\n").unwrap();
    assert_eq!(
        parsed
            .diagnostics()
            .iter()
            .filter(|diagnostic| diagnostic.severity() == &crate::Severity::Warning)
            .count(),
        1
    );
    assert!(
        parsed.document().nodes().iter().any(|node| {
            matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Alias)
        })
    );
    let conditional = parsed.document().nodes()[0].as_tagged().unwrap();
    assert!(
        conditional.children()[1]
            .as_group()
            .unwrap()
            .children()
            .iter()
            .any(|node| matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Alias))
    );
    assert!(parsed.document().nodes().iter().any(|node| {
            matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Alias
                && tagged.children().iter().any(|child| matches!(child, RdNode::Verb(value) if value == "z")))
        }));
}

#[test]
fn conditional_directive_in_option_warns_and_preserves_following_content() {
    let parsed = crate::parse(
            b"\\name{x}\n\\description{\\link[\n#ifdef unix\npkg\n#endif\n]{topic}\n}\n\\alias{after}\n",
        )
        .unwrap();
    assert_eq!(parsed.diagnostics().len(), 1);
    assert_eq!(
        parsed.diagnostics()[0].severity(),
        &crate::Severity::Warning
    );
    assert_eq!(
        parsed.diagnostics()[0].code(),
        &crate::DiagnosticCode::UnexpectedConditional
    );
    assert!(
        parsed.document().nodes().iter().any(|node| {
            matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Alias)
        })
    );
}
