use flate2::read::GzDecoder;
use rd_rds::{RValue, parse};
use std::io::Read;

#[test]
fn raw_access_uses_stable_constructors_accessors_and_wildcards() {
    let bytes = include_bytes!("fixtures/data/rd_minimal_v3.rds");
    let mut decoder = GzDecoder::new(bytes.as_slice());
    let mut decompressed = Vec::new();
    decoder
        .read_to_end(&mut decompressed)
        .expect("fixture gzip stream");
    let object = parse(&decompressed).expect("decompressed fixture");
    walk(&object);
}

fn walk(object: &rd_rds::RObject) {
    for attribute in object.attributes().iter() {
        walk(attribute.value());
    }
    match object.value() {
        RValue::List(values) => values.iter().for_each(walk),
        RValue::Character(_) | RValue::Null => {}
        RValue::Logical(_) | RValue::Integer(_) | RValue::Real(_) => {}
        RValue::Symbol(_) | RValue::Persisted(_) | RValue::Environment(_) => {}
        _ => {}
    }
}
