use std::io::Write;

#[test]
#[ignore = "requires Rscript"]
fn r_oracle_fixture_comparison() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rd");
    assert!(root.is_dir());
    let temp = std::env::temp_dir().join(format!("rd-writer-oracle-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    for entry in std::fs::read_dir(root).unwrap() {
        let input = entry.unwrap().path();
        if input.extension().is_none_or(|ext| ext != "Rd") {
            continue;
        }
        let parsed = rd_source::parse(&std::fs::read(&input).unwrap()).unwrap();
        if !parsed.diagnostics().is_empty() {
            continue;
        }
        let output = temp.join(input.file_name().unwrap());
        std::fs::write(
            &output,
            rd_writer::write_document(parsed.document()).unwrap(),
        )
        .unwrap();
        let status = std::process::Command::new("Rscript")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/support/compare_parse_rd.R"
            ))
            .arg(&input)
            .arg(&output)
            .status()
            .unwrap();
        assert!(status.success(), "R oracle failed for {}", input.display());
    }
    let _ = std::fs::remove_dir_all(temp);
}

#[test]
#[ignore = "requires Rscript"]
fn r_oracle_reported_single_quote_raw_string_round_trip() {
    let temp = std::env::temp_dir().join(format!("rd-writer-single-quote-{}", std::process::id()));
    std::fs::create_dir_all(&temp).unwrap();
    let input = temp.join("input.Rd");
    let output = temp.join("output.Rd");
    let source = r#"\examples{f(x = r'(100%\q)')
}
"#;
    std::fs::write(&input, source).unwrap();

    let parsed = rd_source::parse(source.as_bytes()).unwrap();
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    std::fs::write(
        &output,
        rd_writer::write_document(parsed.document()).unwrap(),
    )
    .unwrap();

    let script = r#"
args <- commandArgs(trailingOnly = TRUE)
rcode <- function(node) {
  if (identical(attr(node, "Rd_tag"), "RCODE"))
    return(node[[1L]])
  if (is.list(node))
    for (child in node) {
      value <- rcode(child)
      if (!is.null(value)) return(value)
    }
  NULL
}
original <- rcode(tools::parse_Rd(args[[1L]]))
written <- rcode(tools::parse_Rd(args[[2L]]))
expected <- "f(x = r'(100%\\q)')\n"
stopifnot(identical(original, expected), identical(written, expected))
"#;
    let mut child = std::process::Command::new("Rscript")
        .arg("-")
        .arg(&input)
        .arg(&output)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(script.as_bytes())
        .unwrap();
    let status = child.wait().unwrap();
    let _ = std::fs::remove_dir_all(temp);
    assert!(status.success(), "R oracle failed");
}
