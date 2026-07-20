use std::fs;

use rd_writer::{LineEnding, Writer, WriterOptions};

fn fixtures() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rd");
    assert!(
        root.is_dir(),
        "fixture directory is missing: {}",
        root.display()
    );
    let mut files = fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "Rd"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[test]
fn local_fixtures_match_sibling_corpus_when_available() {
    let local = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rd");
    let sibling = local.join("../../../../rd-source/tests/fixtures/rd");
    if !sibling.is_dir() {
        return;
    }
    let names = |root: &std::path::Path| {
        let mut names = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(names(&local), names(&sibling));
    for name in names(&local) {
        assert_eq!(
            fs::read(local.join(&name)).unwrap(),
            fs::read(sibling.join(name)).unwrap()
        );
    }
}

#[test]
fn all_diagnostic_free_fixtures_round_trip() {
    let mut skipped = 0;
    for path in fixtures() {
        let bytes = fs::read(&path).unwrap();
        let parsed = rd_source::parse(&bytes).unwrap();
        if !parsed.diagnostics().is_empty() {
            skipped += 1;
            eprintln!("skipped {}", path.display());
            continue;
        }
        let source = rd_writer::write_document(parsed.document())
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let reparsed = rd_source::parse(source.as_bytes()).unwrap();
        assert!(
            reparsed.diagnostics().is_empty(),
            "{}: {:?}",
            path.display(),
            reparsed.diagnostics()
        );
        assert_eq!(reparsed.document(), parsed.document(), "{}", path.display());
        let crlf = Writer::new(WriterOptions::new().with_line_ending(LineEnding::CrLf))
            .write_document(parsed.document())
            .unwrap_or_else(|error| panic!("{} (CRLF): {error}", path.display()));
        let reparsed = rd_source::parse(crlf.as_bytes()).unwrap();
        assert!(
            reparsed.diagnostics().is_empty(),
            "{} (CRLF): {:?}",
            path.display(),
            reparsed.diagnostics()
        );
        assert_eq!(
            reparsed.document(),
            parsed.document(),
            "{} (CRLF)",
            path.display()
        );
    }
    eprintln!("skipped {skipped} fixture(s) with parser diagnostics");
}

#[test]
fn programmatic_documents_round_trip() {
    let document = rd_ast::RdDocument::from(vec![
        rd_ast::RdNode::tagged(
            rd_ast::RdTag::Name,
            None,
            vec![rd_ast::RdNode::Verb("example".into())],
        ),
        rd_ast::RdNode::tagged(
            rd_ast::RdTag::Title,
            None,
            vec![rd_ast::RdNode::Text("A title".into())],
        ),
        rd_ast::RdNode::tagged(
            rd_ast::RdTag::Description,
            None,
            vec![rd_ast::RdNode::tagged(
                rd_ast::RdTag::Link,
                Some(vec![rd_ast::RdNode::Text("base".into())]),
                vec![rd_ast::RdNode::Text("print".into())],
            )],
        ),
    ]);
    let source = rd_writer::write_document(&document).unwrap();
    let parsed = rd_source::parse(source.as_bytes()).unwrap();
    assert!(parsed.diagnostics().is_empty());
    assert_eq!(parsed.document(), &document);
}

#[test]
fn confirmed_lexical_regressions_round_trip() {
    for source in [
        "\\name{x}\n% tail",
        "#ifdef %target\nvalue\n#endif\n",
        "\\examples{# \\}\n}\n",
        "\\examples{x <- r\"(a)\" %% 2\n}\n",
        "\\examples{x <- r\"(a)\" \\%\\% 2\n}\n",
        "\\examples{x <- r\"---(a\\%{)---\" \\% y\n}\n",
        "\\usage{f(\"a\\\"b\\{\")\n}\n",
        "\\examples{\"\\value{x}\"\n}",
        "\\examples{\"\\ldots{} \\link[pkg]{x} \\linkS4class{x} \\value{x} \\var{x} \\verb{x}\"\n}",
        r"\examples{r}",
        r"\examples{R}",
        r#"\examples{r"}"#,
        r#"\examples{r"-}"#,
        r#"\examples{r"---}"#,
        "\\examples{r\"---(x % { y })---\"\n}",
        "\\examples{r\"---\n#ifdef unix\nx\"\n#endif\n}\n",
    ] {
        let parsed = rd_source::parse(source.as_bytes()).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{source:?}: {:?}",
            parsed.diagnostics()
        );
        let written = rd_writer::write_document(parsed.document()).unwrap();
        let reparsed = rd_source::parse(written.as_bytes()).unwrap();
        assert!(
            reparsed.diagnostics().is_empty(),
            "{written:?}: {:?}",
            reparsed.diagnostics()
        );
        assert_eq!(reparsed.document(), parsed.document(), "{source:?}");
    }
}

fn round_trip_source(source: &str) -> (String, rd_ast::RdDocument) {
    let parsed = rd_source::parse(source.as_bytes()).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{source:?}: {:?}",
        parsed.diagnostics()
    );
    let written = rd_writer::write_document(parsed.document()).unwrap();
    let reparsed = rd_source::parse(written.as_bytes()).unwrap();
    assert!(
        reparsed.diagnostics().is_empty(),
        "{written:?}: {:?}",
        reparsed.diagnostics()
    );
    assert_eq!(reparsed.document(), parsed.document(), "{source:?}");
    (written, reparsed.document().clone())
}

fn contains_tag(node: &rd_ast::RdNode, tag: rd_ast::RdTag) -> bool {
    match node {
        rd_ast::RdNode::Tagged(tagged) => {
            tagged.tag() == &tag
                || tagged
                    .children()
                    .iter()
                    .any(|child| contains_tag(child, tag.clone()))
        }
        rd_ast::RdNode::Group(group) => group
            .children()
            .iter()
            .any(|child| contains_tag(child, tag.clone())),
        _ => false,
    }
}

#[test]
fn ordinary_quote_delimiter_match_and_percent_regressions_round_trip() {
    let source = r#"\examples{x <- c("\\\\value{foo}", "function(bar)")
}
"#;
    let (_, document) = round_trip_source(source);
    assert!(
        !document
            .nodes()
            .iter()
            .any(|node| contains_tag(node, rd_ast::RdTag::Value))
    );

    for source in [
        r#"\examples{"100\%"
}
"#,
        r#"\examples{"100%"
}
"#,
    ] {
        round_trip_source(source);
    }
}

#[test]
fn conditional_bodies_preserve_item_context() {
    for (section, item) in [
        (r"\arguments", r"\item{arg}{description}"),
        (r"\value", r"\item{value}{description}"),
        (r"\describe", r"\item{term}{description}"),
        (r"\itemize", r"\item"),
        (r"\enumerate", r"\item"),
    ] {
        let source =
            format!("{section}{{\n% before\n#ifdef FEATURE\n{item}\n% after\n#endif\n}}\n");
        round_trip_source(&source);
    }

    round_trip_source("\\value{\n\\item{name}{\n#ifdef FEATURE\n\\emph{nested}\n#endif\n}\n}\n");
}

#[test]
fn usage_escape_regression_preserves_exact_source() {
    let source = "\\usage{f(\"a\\\"b\\{\")\n}\n";
    let parsed = rd_source::parse(source.as_bytes()).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(
        rd_writer::write_document(parsed.document()).unwrap(),
        source
    );
}
