use rd_ast::{RdDocument, RdNode, RdTag};

struct Case {
    input_rd: &'static str,
    expected_rcode: &'static str,
    expected_written_rd: &'static str,
}

const CASES: &[Case] = &[
    Case {
        input_rd: r##"\examples{r"(a)"}"##,
        expected_rcode: r##"r"(a)""##,
        expected_written_rd: r##"\examples{r"(a)"}"##,
    },
    Case {
        input_rd: r##"\examples{r'(a)'}"##,
        expected_rcode: r##"r'(a)'"##,
        expected_written_rd: r##"\examples{r'(a)'}"##,
    },
    Case {
        input_rd: r##"\examples{R"[a]"}"##,
        expected_rcode: r##"R"[a]""##,
        expected_written_rd: r##"\examples{R"[a]"}"##,
    },
    Case {
        input_rd: r##"\examples{r"{a}"}"##,
        expected_rcode: r##"r"{a}""##,
        expected_written_rd: r##"\examples{r"{a}"}"##,
    },
    Case {
        input_rd: r###"\examples{r'---(a%{\q}"#)---'}"###,
        expected_rcode: r###"r'---(a%{\q}"#)---'"###,
        expected_written_rd: r###"\examples{r'---(a%{\q}"#)---'}"###,
    },
    Case {
        input_rd: r##"\examples{r"(a)q)"}"##,
        expected_rcode: r##"r"(a)q)""##,
        expected_written_rd: r##"\examples{r"(a)q)"}"##,
    },
    Case {
        input_rd: r##"\examples{r"(a))"}"##,
        expected_rcode: r##"r"(a))""##,
        expected_written_rd: r##"\examples{r"(a))"}"##,
    },
    Case {
        input_rd: r##"\examples{r"x"}"##,
        expected_rcode: r##"r"x""##,
        expected_written_rd: r##"\examples{r"x"}"##,
    },
    Case {
        input_rd: "\\examples{r(\x60foo\x60)}",
        expected_rcode: "r(\x60foo\x60)",
        expected_written_rd: "\\examples{r(\x60foo\x60)}",
    },
    Case {
        input_rd: "\\examples{r\n\"(a)\"}",
        expected_rcode: "r\n\"(a)\"",
        expected_written_rd: "\\examples{r\n\"(a)\"}",
    },
    Case {
        input_rd: "\\examples{r\"(a\nb)\"}",
        expected_rcode: "r\"(a\nb)\"",
        expected_written_rd: "\\examples{r\"(a\nb)\"}",
    },
    Case {
        input_rd: "\\examples{\"a\nb\"}",
        expected_rcode: "\"a\nb\"",
        expected_written_rd: "\\examples{\"a\nb\"}",
    },
    Case {
        input_rd: "\\examples{x # 100%\ny}",
        expected_rcode: "x # 100%\ny",
        expected_written_rd: "\\examples{x # 100\\%\ny}",
    },
    Case {
        input_rd: r##"\examples{x \% \{ \}}"##,
        expected_rcode: "x % { }",
        expected_written_rd: r##"\examples{x \% \{ \}}"##,
    },
];

fn rcode(document: &RdDocument) -> String {
    fn visit(node: &RdNode, output: &mut String) {
        match node {
            RdNode::RCode(value) => output.push_str(value),
            RdNode::Tagged(tagged) => {
                for child in tagged.children() {
                    visit(child, output);
                }
            }
            RdNode::Group(group) => {
                for child in group.children() {
                    visit(child, output);
                }
            }
            RdNode::Text(_) | RdNode::Verb(_) | RdNode::Comment(_) | RdNode::Raw(_) | _ => {}
        }
    }

    let mut output = String::new();
    for node in document.nodes() {
        visit(node, &mut output);
    }
    output
}

fn document_for_rcode(value: &str) -> RdDocument {
    let children = if value.contains('\n')
        && (value.starts_with("r\"")
            || value.starts_with("r'")
            || value.starts_with("R\"")
            || value.starts_with("R'"))
    {
        vec![RdNode::RCode(value.to_owned())]
    } else {
        value
            .split_inclusive('\n')
            .filter(|part| !part.is_empty())
            .map(|part| RdNode::RCode(part.to_owned()))
            .collect()
    };
    RdDocument::from(vec![RdNode::tagged(RdTag::Examples, None, children)])
}

#[test]
fn independent_literal_expectations_cover_shared_rlike_behaviour() {
    for (index, case) in CASES.iter().enumerate() {
        let parsed = rd_source::parse(case.input_rd.as_bytes()).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "input {:?}: {:?}",
            case.input_rd,
            parsed.diagnostics()
        );
        assert_eq!(rcode(parsed.document()), case.expected_rcode, "input");

        let expected_document = document_for_rcode(case.expected_rcode);
        let written = rd_writer::write_document(&expected_document)
            .unwrap_or_else(|error| panic!("case {index}: {error}"));
        assert_eq!(written, case.expected_written_rd, "expected writer output");

        let reparsed = rd_source::parse(case.expected_written_rd.as_bytes()).unwrap();
        assert!(
            reparsed.diagnostics().is_empty(),
            "written {:?}: {:?}",
            case.expected_written_rd,
            reparsed.diagnostics()
        );
        assert_eq!(reparsed.document(), &expected_document, "parse-back");
    }
}

#[test]
fn adjacent_leaves_and_conditional_frame_transitions_remain_round_trippable() {
    let source = "\\examples{x\n#ifdef unix\ny\n#endif\nz}";
    let parsed = rd_source::parse(source.as_bytes()).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let written = rd_writer::write_document(parsed.document()).unwrap();
    let reparsed = rd_source::parse(written.as_bytes()).unwrap();
    assert_eq!(reparsed.document(), parsed.document());
}

#[test]
#[ignore = "requires Rscript"]
fn r_oracle_independent_literal_expectations() {
    let temp = std::env::temp_dir().join(format!(
        "rd-writer-rlike-conformance-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp).unwrap();
    let mut command = std::process::Command::new("Rscript");
    command.arg("-");
    for (index, case) in CASES.iter().enumerate() {
        let input = temp.join(format!("{}-input.Rd", index));
        let written = temp.join(format!("{}-written.Rd", index));
        std::fs::write(&input, case.input_rd).unwrap();
        std::fs::write(&written, case.expected_written_rd).unwrap();
        command.arg(input).arg(written).arg(case.expected_rcode);
    }
    let script = r#"
args <- commandArgs(trailingOnly = TRUE)
rcode <- function(document) {
  values <- character()
  visit <- function(node) {
    if (identical(attr(node, "Rd_tag"), "RCODE")) {
      values <<- c(values, node[[1L]])
      return(invisible(NULL))
    }
    if (is.list(node)) lapply(node, visit)
    invisible(NULL)
  }
  visit(document)
  paste0(values, collapse = "")
}
stopifnot(length(args) %% 3L == 0L)
for (i in seq.int(1L, length(args), by = 3L)) {
  expected <- args[[i + 2L]]
  original <- rcode(tools::parse_Rd(args[[i]]))
  written <- rcode(tools::parse_Rd(args[[i + 1L]]))
  if (!identical(original, expected) || !identical(written, expected)) {
    cat("case", (i - 1L) / 3L, "\\n")
    dput(list(expected = expected, original = original, written = written))
    quit(status = 1L)
  }
}
"#;
    let mut child = command.stdin(std::process::Stdio::piped()).spawn().unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), script.as_bytes()).unwrap();
    let status = child.wait().unwrap();
    let _ = std::fs::remove_dir_all(temp);
    assert!(status.success(), "R oracle failed");
}
