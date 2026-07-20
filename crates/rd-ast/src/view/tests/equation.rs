use super::*;

#[test]
fn equations_borrow_arguments_and_record_display_mode() {
    let one = RdNode::tagged(
        RdTag::Eqn,
        None,
        vec![raw_group(vec![RdNode::Verb("x^2".into())])],
    );
    let two = RdNode::tagged(
        RdTag::Eqn,
        None,
        vec![
            raw_group(vec![RdNode::Verb("x^2".into())]),
            raw_group(vec![RdNode::Text("x squared".into())]),
        ],
    );
    let block = RdNode::tagged(
        RdTag::Deqn,
        None,
        vec![raw_group(vec![RdNode::Text("sum".into())])],
    );
    let base = RdPath::new(vec![RdPathSegment::TopLevel(4)]);
    let one = one.as_tagged().unwrap().inspect_equation(&base).unwrap();
    assert_eq!(one.display(), RdEquationDisplay::Inline);
    assert_eq!(one.ascii(), None);
    assert_eq!(one.path(), &base);
    let two = two.as_tagged().unwrap().inspect_equation(&base).unwrap();
    assert_eq!(two.ascii().unwrap().len(), 1);
    let block = block.as_tagged().unwrap().inspect_equation(&base).unwrap();
    assert_eq!(block.display(), RdEquationDisplay::Block);
}

#[test]
fn equations_report_shape_errors_and_propagate_paths() {
    let base = RdPath::new(vec![RdPathSegment::TopLevel(2)]);
    let option = RdNode::tagged(RdTag::Eqn, Some(vec![]), vec![]);
    assert!(matches!(
        option
            .as_tagged()
            .unwrap()
            .inspect_equation(&base)
            .unwrap_err()
            .kind(),
        RdShapeErrorKind::UnexpectedOption
    ));
    for count in [0, 3] {
        let node = RdNode::tagged(
            RdTag::Eqn,
            None,
            (0..count).map(|_| raw_group(vec![])).collect(),
        );
        assert!(matches!(
            node.as_tagged().unwrap().inspect_equation(&base).unwrap_err().kind(),
            RdShapeErrorKind::WrongArity {
                expected: RdArity::Range { min: 1, max: 2 },
                actual
            } if *actual == count
        ));
    }
    let child = RdNode::tagged(RdTag::Eqn, None, vec![RdNode::Text("bad".into())]);
    let error = child
        .as_tagged()
        .unwrap()
        .inspect_equation(&base)
        .unwrap_err();
    assert_eq!(
        error.path().segments(),
        &[RdPathSegment::TopLevel(2), RdPathSegment::Child(0)]
    );
    assert!(matches!(
        error.kind(),
        RdShapeErrorKind::UnexpectedNode {
            expected: RdExpectedNode::Group,
            actual: RdNodeKind::Text
        }
    ));
    let wrong = RdNode::tagged(RdTag::Href, None, vec![]);
    let error = wrong
        .as_tagged()
        .unwrap()
        .inspect_equation(&base)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        r"expected equation node, found tagged node for \href at top-level[2]"
    );
}
/// Tests that lower a real fixture (via the `rds` feature) and exercise
/// [`RdDocument::arguments`] against it.
#[cfg(feature = "rds")]
mod rds_tests {
    use std::{fs, io::Read, path::PathBuf};

    use flate2::read::GzDecoder;
    use rd_rds::parse;

    use crate::lower_r_object;

    fn fixture(name: &str) -> rd_rds::RObject {
        // These crate-local copies originate from crates/rd-rds/tests/fixtures/data/ and committed
        // R generator scripts; keep them byte-identical so packaged rd-ast tests run standalone.
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
        let bytes = fs::read(dir.join(name)).unwrap_or_else(|err| panic!("read {name}: {err}"));
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .unwrap_or_else(|err| panic!("gunzip {name}: {err}"));
        parse(&decompressed).unwrap_or_else(|err| panic!("parse {name}: {err}"))
    }

    /// `tests/fixtures/rd-src/arguments.Rd` (source for the
    /// `rd_arguments_v*.rds` fixtures) declares an `\arguments` block with
    /// five `\item{name}{description}` entries (`x`, `y`, `z`, `verbose`,
    /// `\dots`).
    ///
    /// The fixture was parsed by `tools::parse_Rd()`, which attaches a
    /// `srcref` attribute to every node; the lowering discards `srcref` as
    /// known producer metadata (see the `rds` module documentation), so
    /// `\arguments` and its `\item`s lower structurally to
    /// [`crate::RdNode::Tagged`] and [`RdDocument::arguments`] pairs all
    /// five entries. The `\dots` item's name group contains only the
    /// (childless) `\dots` macro node, so its [`text_contents`] is empty.
    #[test]
    fn arguments_fixture_yields_five_expected_arguments() {
        use super::super::text_contents;
        use crate::RdDocument;

        for version in [2, 3] {
            let root = fixture(&format!("rd_arguments_v{version}.rds"));
            let doc: RdDocument = lower_r_object(&root).expect("lower document");

            let arguments: Vec<(String, String)> = doc
                .arguments()
                .map(|argument| {
                    (
                        text_contents(argument.name),
                        text_contents(argument.description),
                    )
                })
                .collect();
            assert_eq!(
                arguments,
                vec![
                    ("x".to_string(), "the first argument".to_string()),
                    ("y".to_string(), "the second argument".to_string()),
                    ("z".to_string(), "the third argument".to_string()),
                    ("verbose".to_string(), "logical; print progress".to_string()),
                    (
                        String::new(),
                        "further arguments passed to methods".to_string()
                    ),
                ]
            );

            let inspected: Vec<(String, String)> = doc
                .inspect_arguments()
                .expect("fixture has one well-formed arguments container")
                .map(|result| {
                    let argument = result.expect("fixture arguments are structurally valid");
                    (
                        text_contents(argument.name),
                        text_contents(argument.description),
                    )
                })
                .collect();
            assert_eq!(inspected, arguments);

            let title = doc.title().expect(r"\title lowers to Tagged");
            assert_eq!(
                text_contents(title),
                "A Topic With Several Arguments".to_string()
            );
        }
    }
}
