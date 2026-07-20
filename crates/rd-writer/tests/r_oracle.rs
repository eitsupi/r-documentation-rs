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
