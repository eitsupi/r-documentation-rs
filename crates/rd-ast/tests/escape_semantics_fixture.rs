#![cfg(feature = "rds")]

use std::{fs, io::Read, path::PathBuf};

use flate2::read::GzDecoder;
use rd_ast::{RdDocument, RdNode, RdTag, lower_r_object};
use rd_rds::parse;

fn fixture(version: u8) -> rd_rds::RObject {
    // These crate-local copies originate from crates/rd-rds/tests/fixtures/data/ and committed
    // R generator scripts; keep them byte-identical so packaged rd-ast tests run standalone.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
    let bytes = fs::read(dir.join(format!("rd_escapes_v{version}.rds"))).unwrap();
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();
    parse(&decompressed).unwrap()
}

fn description(document: &RdDocument) -> &[RdNode] {
    document
        .nodes()
        .iter()
        .find_map(|node| {
            let tagged = node.as_tagged()?;
            (tagged.tag() == &RdTag::Description).then_some(tagged.children())
        })
        .expect("description")
}

#[test]
fn parse_rd_escapes_are_decoded_in_lowered_text() {
    for version in [2, 3] {
        let document = lower_r_object(&fixture(version)).unwrap();
        let text = description(&document)
            .iter()
            .find_map(|node| match node {
                RdNode::Text(text) if text.contains("Literal percent") => Some(text.as_str()),
                _ => None,
            })
            .expect("description text");
        assert_eq!(
            text,
            "Literal percent: %, braces: { and }, and slash: \\.\n"
        );
    }
}
