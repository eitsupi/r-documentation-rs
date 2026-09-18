use rd_ast::{RdAstPath, RdAstPathSegment};

#[test]
fn write_error_path_maps_to_a_parsed_source_extent() {
    let input = br"\unknown{x}";
    let parsed = rd_source::parse(input).unwrap();
    let error = rd_writer::write_document(parsed.document()).unwrap_err();
    let path = error
        .ast_path()
        .expect("an unsupported parsed tag has a canonical AST path");
    assert_eq!(path, &RdAstPath::new(vec![RdAstPathSegment::TopLevel(0)]));
    assert_eq!(
        parsed.source_map().span(path).unwrap().bytes(),
        0..input.len()
    );
}
