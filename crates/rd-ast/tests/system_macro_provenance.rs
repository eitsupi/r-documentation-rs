#![cfg(feature = "rds")]

use std::{collections::BTreeMap, fs, path::PathBuf};

use flate2::read::GzDecoder;
use rd_ast::{
    RdDocument, RdNode, RdPath, RdPathSegment, RdSystemMacroItem, RdSystemMacroItems,
    RdSystemMacroItemsStrict, RdTag, lower_r_object,
};
use rd_rds::{RObject, RValue, parse};

fn fixture(name: &str, compressed: bool) -> RObject {
    // These crate-local copies originate from crates/rd-rds/tests/fixtures/data/ and committed
    // R generator scripts; keep them byte-identical so packaged rd-ast tests run standalone.
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/data");
    let bytes = fs::read(dir.join(name)).unwrap_or_else(|err| panic!("read {name}: {err}"));
    if compressed {
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut decompressed = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut decompressed).unwrap();
        parse(&decompressed).unwrap()
    } else {
        parse(&bytes).unwrap()
    }
}

fn text(value: &RObject) -> Option<String> {
    let RValue::Character(values) = value.value() else {
        return None;
    };
    (values.len() == 1)
        .then(|| values[0].as_str())
        .flatten()?
        .ok()
        .map(Into::into)
}

fn usermacro_srcfile_provenances(object: &RObject, found: &mut Vec<&'static str>) {
    let is_usermacro =
        object.attributes().get("Rd_tag").and_then(text).as_deref() == Some("USERMACRO");
    if is_usermacro {
        let srcfile = object
            .attributes()
            .get("srcref")
            .and_then(|srcref| srcref.attributes().get("srcfile"));
        found.push(match srcfile.map(RObject::value) {
            Some(RValue::Environment(_)) => "environment",
            Some(RValue::Persisted(_)) => "persisted",
            _ => "other",
        });
    }

    if let RValue::List(items) = object.value() {
        for item in items {
            usermacro_srcfile_provenances(item, found);
        }
    }
    for attribute in object.attributes().iter() {
        usermacro_srcfile_provenances(attribute.value(), found);
    }
}

/// Collects the srcfile provenance of every USERMACRO in the tree, so a
/// mixed-provenance fixture cannot pass on the strength of its first macro.
fn usermacro_srcfile_provenance(object: &RObject) -> Vec<&'static str> {
    let mut found = Vec::new();
    usermacro_srcfile_provenances(object, &mut found);
    found
}

fn tagged<'a>(nodes: &'a [RdNode], wanted: &RdTag, path: &RdPath) -> Vec<(&'a RdNode, RdPath)> {
    let mut found = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let child_path = if path.segments().is_empty() {
            RdPath::new(vec![RdPathSegment::TopLevel(index)])
        } else {
            path.with_child(index)
        };
        if let Some(tagged_node) = node.as_tagged() {
            if tagged_node.tag() == wanted {
                found.push((node, child_path.clone()));
            }
            found.extend(tagged(tagged_node.children(), wanted, &child_path));
        } else if let Some(group) = node.as_group() {
            found.extend(tagged(group.children(), wanted, &child_path));
        }
    }
    found
}

fn macro_summary(document: &RdDocument) -> (usize, BTreeMap<String, usize>) {
    let root = RdPath::new(vec![]);
    let (description, path) = tagged(document.nodes(), &RdTag::Description, &root)
        .into_iter()
        .next()
        .expect("description section");
    let items = RdSystemMacroItems::children(
        description
            .as_tagged()
            .expect("tagged description")
            .children(),
        &path,
    )
    .filter_map(|item| match item {
        RdSystemMacroItem::Macro(item) => Some((item.semantic(), item.origin(), item.consumed())),
        RdSystemMacroItem::Node { .. } => None,
        _ => None,
    })
    .collect::<Vec<_>>();
    fn visit(node: &RdNode, names: &mut BTreeMap<String, usize>) {
        if let RdNode::Raw(raw) = node
            && raw.tag() == Some("USERMACRO")
        {
            if let Some(name) = raw
                .attributes()
                .iter()
                .find_map(|a| {
                    (a.name() == "macro").then(|| match a.value().value() {
                        rd_ast::RawRdValue::Character(v) if v.len() == 1 => v[0].clone(),
                        _ => None,
                    })
                })
                .flatten()
            {
                *names.entry(name).or_default() += 1;
            }
        }
        match node {
            RdNode::Tagged(n) => n.children().iter().for_each(|x| visit(x, names)),
            RdNode::Group(n) => n.children().iter().for_each(|x| visit(x, names)),
            RdNode::Raw(n) => n.children().iter().for_each(|x| visit(x, names)),
            _ => {}
        }
    }
    let mut names = BTreeMap::new();
    document
        .nodes()
        .iter()
        .for_each(|node| visit(node, &mut names));
    (items.len(), names)
}

fn assert_fixture(name: &str, compressed: bool) -> RObject {
    let raw = fixture(name, compressed);
    let document = lower_r_object(&raw).unwrap_or_else(|err| panic!("lower {name}: {err:?}"));
    let (item_count, names) = macro_summary(&document);
    assert_eq!(item_count, 0);
    assert_eq!(names.get("\\fixtureWrap"), Some(&2));
    assert_eq!(names.get("\\fixtureInsert"), Some(&1));

    let root = RdPath::new(vec![]);
    let (description, path) = tagged(document.nodes(), &RdTag::Description, &root)
        .into_iter()
        .next()
        .expect("description section");
    let children = description.as_tagged().unwrap().children();
    for pair in children.windows(2) {
        let RdNode::Raw(raw) = &pair[0] else { continue };
        let Some(name) = raw
            .attributes()
            .iter()
            .find_map(|a| {
                (a.name() == "macro").then(|| match a.value().value() {
                    rd_ast::RawRdValue::Character(v) if v.len() == 1 => v[0].as_deref(),
                    _ => None,
                })
            })
            .flatten()
        else {
            continue;
        };
        let expected = if name == "\\fixtureWrap" {
            RdTag::Strong
        } else {
            RdTag::Emph
        };
        assert!(matches!(&pair[1], RdNode::Tagged(tagged) if tagged.tag() == &expected));
    }
    assert!(
        RdSystemMacroItems::children(children, &path)
            .all(|item| matches!(item, RdSystemMacroItem::Node { .. }))
    );
    let strict = RdSystemMacroItemsStrict::children(
        description
            .as_tagged()
            .expect("tagged description")
            .children(),
        &path,
    )
    .collect::<Result<Vec<_>, _>>();
    assert!(
        strict.is_ok(),
        "strict traversal failed for {name}: {strict:?}"
    );
    raw
}

#[test]
fn system_macro_views_match_across_direct_and_refhook_provenance() {
    let direct_raw = assert_fixture("rd_user_macros_v3.rds", true);
    let refhook_raw = assert_fixture("rd_user_macros_refhook_v3.rds", false);

    assert_eq!(
        usermacro_srcfile_provenance(&direct_raw),
        vec!["environment"; 3]
    );
    assert_eq!(
        usermacro_srcfile_provenance(&refhook_raw),
        vec!["persisted"; 3]
    );
}
