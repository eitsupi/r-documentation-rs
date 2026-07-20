use super::fixture::{Case, Comparison, ObligationView};
use rd_ast::{RdDocument, RdNode};

fn has(view: ObligationView<'_>, label: &str) -> bool {
    view.labels.iter().any(|x| x == label)
}
fn check_manifest_consistency(case: &Case, view: ObligationView<'_>) -> Result<(), String> {
    match case.comparison {
        Comparison::OracleParity => {
            if case.divergence_reason.is_some()
                || case.oracle_obligations.is_some()
                || case.source_obligations.is_some()
            {
                return Err(format!(
                    "{}: oracle-parity cases cannot define divergence fields",
                    case.name
                ));
            }
        }
        Comparison::IntentionalDivergence => {
            if case
                .divergence_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(format!(
                    "{}: intentional-divergence requires a non-empty divergence_reason",
                    case.name
                ));
            }
            if case.oracle_obligations.is_none() || case.source_obligations.is_none() {
                return Err(format!(
                    "{}: intentional-divergence requires oracle and source obligations",
                    case.name
                ));
            }
            if !case.obligations.is_empty() {
                return Err(format!(
                    "{}: intentional-divergence requires empty legacy obligations",
                    case.name
                ));
            }
        }
        Comparison::SourceOnly => {
            if case.source_obligations.is_none() {
                return Err(format!(
                    "{}: source-only requires source obligations",
                    case.name
                ));
            }
        }
    }
    let mappings = [
        (
            "comment_paths",
            "comment_paths",
            view.comment_paths.is_some(),
            view.comment_paths.is_some_and(|v| !v.is_empty()),
        ),
        (
            "comment_values",
            "comment_paths",
            view.comment_values.is_some(),
            view.comment_values.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_sequence",
            "node_sequence",
            view.expected_sequence.is_some(),
            view.expected_sequence.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_leaves",
            "exact_leaf_values",
            view.expected_leaves.is_some(),
            view.expected_leaves.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_leaf_kinds",
            "leaf_kinds",
            view.expected_leaf_kinds.is_some(),
            view.expected_leaf_kinds.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_decoded",
            "decoded_escapes_by_kind",
            view.expected_decoded.is_some(),
            view.expected_decoded.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_groups",
            "group_shape",
            view.expected_groups.is_some(),
            view.expected_groups.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_root_nodes",
            "document_shape",
            view.expected_root_nodes.is_some(),
            view.expected_root_nodes.is_some(),
        ),
        (
            "expected_arities",
            "tag_arity",
            view.expected_arities.is_some(),
            view.expected_arities.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_options",
            "option_shape",
            view.expected_options.is_some(),
            view.expected_options.is_some_and(|v| !v.is_empty()),
        ),
        (
            "expected_option_nodes",
            "option_nodes",
            view.expected_option_nodes.is_some(),
            view.expected_option_nodes.is_some_and(|v| !v.is_empty()),
        ),
        (
            "required_tags",
            "required_tags",
            view.required_tags.is_some(),
            view.required_tags.is_some_and(|v| !v.is_empty()),
        ),
    ];
    for (field, label, present, nonempty) in mappings {
        if present && !has(view, label) {
            return Err(format!(
                "{}: manifest field '{field}' is present but obligation label '{label}' is absent",
                case.name
            ));
        }
        if has(view, label) && !nonempty {
            return Err(format!(
                "{}: obligation label '{label}' requires non-empty field '{field}'",
                case.name
            ));
        }
    }
    Ok(())
}
fn kind(node: &RdNode) -> Option<String> {
    Some(match node {
        RdNode::Text(_) => "TEXT".into(),
        RdNode::RCode(_) => "RCODE".into(),
        RdNode::Verb(_) => "VERB".into(),
        RdNode::Comment(_) => "COMMENT".into(),
        RdNode::Group(_) => "GROUP".into(),
        RdNode::Tagged(n) => n.tag().as_rd_tag().trim_start_matches('\\').into(),
        RdNode::Raw(raw) => return raw.tag().map(|tag| tag.trim_start_matches('\\').into()),
        _ => return None,
    })
}
fn node_at<'a>(doc: &'a RdDocument, path: &str) -> Option<&'a RdNode> {
    if path == "root" {
        return None;
    }
    let mut parts = path.strip_prefix("root/")?.split('/');
    let mut node = doc.nodes().get(parts.next()?.parse::<usize>().ok()?)?;
    for p in parts {
        let index = p.parse::<usize>().ok()?;
        node = match node {
            RdNode::Group(g) => g.children().get(index)?,
            RdNode::Tagged(t) => t.children().get(index)?,
            _ => return None,
        };
    }
    Some(node)
}
fn walk(
    node: &RdNode,
    path: &str,
    comments: &mut Vec<(String, String)>,
    leaves: &mut Vec<(String, String)>,
    raw: &mut bool,
) {
    match node {
        RdNode::Comment(v) => {
            comments.push((path.into(), v.clone()));
            leaves.push(("COMMENT".into(), v.clone()));
        }
        RdNode::Text(v) => leaves.push(("TEXT".into(), v.clone())),
        RdNode::RCode(v) => leaves.push(("RCODE".into(), v.clone())),
        RdNode::Verb(v) => leaves.push(("VERB".into(), v.clone())),
        RdNode::Raw(_) => *raw = true,
        RdNode::Group(g) => {
            for (i, child) in g.children().iter().enumerate() {
                walk(child, &format!("{path}/{i}"), comments, leaves, raw);
            }
        }
        RdNode::Tagged(t) => {
            for (i, child) in t.children().iter().enumerate() {
                walk(child, &format!("{path}/{i}"), comments, leaves, raw);
            }
        }
        _ => *raw = true,
    }
}

pub fn check(document: &RdDocument, case: &Case, view: ObligationView<'_>) -> Result<(), String> {
    let known = [
        "adjacent_text",
        "whitespace_text",
        "comments",
        "comment_paths",
        "groups",
        "option_absent",
        "option_nonempty",
        "option_present_empty",
        "decoded_escapes",
        "unicode_exact",
        "node_sequence",
        "exact_leaf_values",
        "leaf_kinds",
        "decoded_escapes_by_kind",
        "group_shape",
        "empty_document",
        "document_shape",
        "tag_arity",
        "option_shape",
        "option_nodes",
        "crlf_source",
        "required_tags",
    ];
    for label in view.labels {
        if !known.contains(&label.as_str()) {
            return Err(format!("{}: unknown obligation label: {label}", case.name));
        }
    }
    check_manifest_consistency(case, view)?;
    let mut comments = Vec::new();
    let mut leaves = Vec::new();
    let mut raw = false;
    for (i, n) in document.nodes().iter().enumerate() {
        walk(
            n,
            &format!("root/{i}"),
            &mut comments,
            &mut leaves,
            &mut raw,
        );
    }
    if comments.len() != view.comments {
        return Err(format!(
            "{}: expected {} comments, found {}",
            case.name,
            view.comments,
            comments.len()
        ));
    }
    if comments.iter().any(|(_, v)| !v.starts_with('%')) {
        return Err(format!("{}: comment does not start with %", case.name));
    }
    if has(view, "comment_paths")
        && (comments.iter().map(|x| x.0.clone()).collect::<Vec<_>>()
            != view.comment_paths.unwrap_or_default()
            || comments.iter().map(|x| x.1.clone()).collect::<Vec<_>>()
                != view.comment_values.unwrap_or_default())
    {
        return Err(format!("{}: comment paths/values differ", case.name));
    }
    if has(view, "empty_document") && !document.nodes().is_empty() {
        return Err(format!("{}: document is not empty", case.name));
    }
    if has(view, "document_shape")
        && document.nodes().len() != view.expected_root_nodes.unwrap_or(-1) as usize
    {
        return Err(format!("{}: root node count differs", case.name));
    }
    if has(view, "leaf_kinds") {
        if raw {
            return Err(format!(
                "{}: Raw node encountered while checking leaf_kinds",
                case.name
            ));
        }
        if leaves.iter().map(|x| x.0.clone()).collect::<Vec<_>>()
            != view.expected_leaf_kinds.unwrap_or_default()
        {
            return Err(format!("{}: leaf kinds differ", case.name));
        }
    }
    if has(view, "exact_leaf_values")
        && leaves
            != view
                .expected_leaves
                .unwrap()
                .iter()
                .map(|x| (x.kind.clone(), x.value.clone()))
                .collect::<Vec<_>>()
    {
        return Err(format!(
            "{}: leaf values differ: expected {:?}, actual {:?}",
            case.name, view.expected_leaves, leaves
        ));
    }
    if has(view, "decoded_escapes_by_kind") {
        for entry in view.expected_decoded.unwrap() {
            let k = &entry.kind;
            let v = &entry.value;
            let vals: Vec<_> = leaves.iter().filter(|x| &x.0 == k).map(|x| &x.1).collect();
            if !vals.iter().any(|x| x.contains(v))
                || vals
                    .iter()
                    .any(|x| [r"\%", r"\{", r"\}", r"\\"].iter().any(|e| x.contains(e)))
            {
                return Err(format!("{}: decoded escapes differ for {k}", case.name));
            }
        }
    }
    if has(view, "node_sequence") {
        for entry in view.expected_sequence.unwrap() {
            let path = &entry.path;
            let expected = &entry.kinds;
            let actual: Vec<String> = if path == "root" {
                document
                    .nodes()
                    .iter()
                    .map(|x| {
                        kind(x).ok_or_else(|| {
                            format!("{}: Raw node in node_sequence at root", case.name)
                        })
                    })
                    .collect::<Result<_, _>>()?
            } else {
                let n = node_at(document, path)
                    .ok_or_else(|| format!("{}: invalid node path {path}", case.name))?;
                match n {
                    RdNode::Group(g) => g.children(),
                    RdNode::Tagged(t) => t.children(),
                    _ => {
                        return Err(format!(
                            "{}: node_sequence path is not a container: {path}",
                            case.name
                        ));
                    }
                }
                .iter()
                .map(|x| {
                    kind(x).ok_or_else(|| {
                        format!("{}: Raw node in node_sequence at {path}", case.name)
                    })
                })
                .collect::<Result<_, _>>()?
            };
            if &actual != expected {
                return Err(format!("{}: node sequence differs at {path}", case.name));
            }
        }
    }
    if has(view, "group_shape") {
        for entry in view.expected_groups.unwrap() {
            let path = &entry.path;
            let count = entry.children;
            match node_at(document, path) {
                Some(RdNode::Group(g)) if g.children().len() == count => {}
                _ => return Err(format!("{}: group shape differs at {path}", case.name)),
            }
        }
    }
    if has(view, "tag_arity") {
        let mut actual: std::collections::BTreeMap<String, Vec<usize>> =
            std::collections::BTreeMap::new();
        fn arities(n: &RdNode, actual: &mut std::collections::BTreeMap<String, Vec<usize>>) {
            match n {
                RdNode::Tagged(t) => {
                    let tag = t.tag().as_rd_tag().trim_start_matches('\\').to_string();
                    let count = t
                        .children()
                        .iter()
                        .filter(|x| matches!(x, RdNode::Group(_)))
                        .count();
                    actual.entry(tag).or_default().push(count);
                    for x in t.children() {
                        arities(x, actual);
                    }
                }
                RdNode::Group(g) => {
                    for x in g.children() {
                        arities(x, actual)
                    }
                }
                _ => {}
            }
        }
        for n in document.nodes() {
            arities(n, &mut actual);
        }
        for entry in view.expected_arities.unwrap() {
            if actual.get(&entry.tag) != Some(&entry.counts) {
                return Err(format!(
                    "{}: tag arity differs for {}",
                    case.name, entry.tag
                ));
            }
        }
    }
    if has(view, "option_shape") {
        let expected = view
            .expected_options
            .unwrap()
            .iter()
            .map(|x| format!("{}:{}", x.tag, x.presence))
            .collect::<Vec<_>>();
        let wanted: std::collections::BTreeSet<_> = expected
            .iter()
            .filter_map(|x| x.split_once(':').map(|(t, _)| t))
            .collect();
        let mut actual = Vec::new();
        fn options(
            n: &RdNode,
            wanted: &std::collections::BTreeSet<&str>,
            actual: &mut Vec<String>,
        ) {
            match n {
                RdNode::Tagged(t) => {
                    let tag = t.tag().as_rd_tag().trim_start_matches('\\');
                    if wanted.contains(tag) {
                        let presence = match t.option() {
                            None => "absent",
                            Some([]) => "empty",
                            Some(_) => "nonempty",
                        };
                        actual.push(format!("{tag}:{presence}"));
                    }
                    for x in t.children() {
                        options(x, wanted, actual);
                    }
                }
                RdNode::Group(g) => {
                    for x in g.children() {
                        options(x, wanted, actual)
                    }
                }
                _ => {}
            }
        }
        for n in document.nodes() {
            options(n, &wanted, &mut actual);
        }
        if actual != expected {
            return Err(format!("{}: option shape differs", case.name));
        }
    }
    if has(view, "option_nodes") {
        fn descriptor(node: &RdNode) -> Option<String> {
            let kind = kind(node)?;
            let leaf = match node {
                RdNode::Text(v) | RdNode::RCode(v) | RdNode::Verb(v) | RdNode::Comment(v) => {
                    Some(v.as_str())
                }
                RdNode::Tagged(t) if t.children().len() == 1 => match &t.children()[0] {
                    RdNode::Text(v) | RdNode::RCode(v) | RdNode::Verb(v) | RdNode::Comment(v) => {
                        Some(v.as_str())
                    }
                    _ => None,
                },
                _ => None,
            };
            Some(leaf.map_or(kind.clone(), |value| format!("{kind}:{value}")))
        }
        fn options(n: &RdNode, actual: &mut Vec<String>) -> Result<(), String> {
            match n {
                RdNode::Tagged(t) => {
                    if let Some(option) = t.option() {
                        let tag = t.tag().as_rd_tag().trim_start_matches('\\');
                        let nodes = option
                            .iter()
                            .map(|node| {
                                descriptor(node).ok_or_else(|| {
                                    format!("unsupported Raw node in option_nodes for {tag}")
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        actual.push(format!("{tag}:{}", nodes.join(",")));
                    }
                    for child in t.children() {
                        options(child, actual)?;
                    }
                }
                RdNode::Group(g) => {
                    for child in g.children() {
                        options(child, actual)?;
                    }
                }
                _ => {}
            }
            Ok(())
        }
        let mut actual = Vec::new();
        for node in document.nodes() {
            options(node, &mut actual)?;
        }
        let expected = view
            .expected_option_nodes
            .unwrap()
            .iter()
            .map(|entry| {
                format!(
                    "{}:{}",
                    entry.tag,
                    entry
                        .nodes
                        .iter()
                        .map(|n| n
                            .value
                            .as_ref()
                            .map_or(n.kind.clone(), |v| format!("{}:{}", n.kind, v)))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(format!(
                "{}: option nodes differ: expected {:?}, actual {:?}",
                case.name, expected, actual
            ));
        }
    }
    if has(view, "required_tags") {
        fn tags(n: &RdNode, found: &mut std::collections::BTreeSet<String>) {
            match n {
                RdNode::Tagged(t) => {
                    found.insert(t.tag().as_rd_tag().trim_start_matches('\\').to_string());
                    for child in t.children() {
                        tags(child, found);
                    }
                }
                RdNode::Group(g) => {
                    for child in g.children() {
                        tags(child, found);
                    }
                }
                RdNode::Raw(raw) => {
                    if let Some(tag) = raw.tag() {
                        found.insert(tag.trim_start_matches('\\').to_string());
                    }
                }
                _ => {}
            }
        }
        let mut found = std::collections::BTreeSet::new();
        for node in document.nodes() {
            tags(node, &mut found);
        }
        for tag in view.required_tags.unwrap() {
            if !found.contains(tag) {
                return Err(format!("{}: required tag missing: {tag}", case.name));
            }
        }
    }
    if has(view, "crlf_source") {
        let bytes = std::fs::read(crate::support::fixture::fixture_root().join(&case.rd))
            .map_err(|e| format!("{}: {e}", case.name))?;
        let lf: Vec<_> = bytes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| (*b == b'\n').then_some(i))
            .collect();
        if lf.is_empty()
            || !lf.iter().all(|&i| i > 0 && bytes[i - 1] == b'\r')
            || !bytes.windows(2).any(|p| p == b"\r\n")
        {
            return Err(format!("{}: source is not CRLF-only", case.name));
        }
    }
    let mut adjacent = false;
    let mut groups = 0;
    let mut whitespace = false;
    let mut absent = 0;
    let mut nonempty = 0;
    let mut empty = 0;
    fn scan(
        n: &RdNode,
        adjacent: &mut bool,
        groups: &mut usize,
        whitespace: &mut bool,
        absent: &mut usize,
        nonempty: &mut usize,
        empty: &mut usize,
    ) {
        match n {
            RdNode::Text(v) => {
                if !v.is_empty() && v.chars().all(char::is_whitespace) {
                    *whitespace = true
                }
            }
            RdNode::Group(g) => {
                *groups += 1;
                for p in g.children().windows(2) {
                    if matches!((&p[0], &p[1]), (RdNode::Text(_), RdNode::Text(_))) {
                        *adjacent = true
                    }
                }
                for x in g.children() {
                    scan(x, adjacent, groups, whitespace, absent, nonempty, empty)
                }
            }
            RdNode::Tagged(t) => {
                if t.tag() == &rd_ast::RdTag::Link {
                    match t.option() {
                        None => *absent += 1,
                        Some([]) => *empty += 1,
                        Some(_) => *nonempty += 1,
                    }
                }
                for p in t.children().windows(2) {
                    if matches!((&p[0], &p[1]), (RdNode::Text(_), RdNode::Text(_))) {
                        *adjacent = true
                    }
                }
                for x in t.children() {
                    scan(x, adjacent, groups, whitespace, absent, nonempty, empty)
                }
            }
            _ => {}
        }
    }
    for n in document.nodes() {
        scan(
            n,
            &mut adjacent,
            &mut groups,
            &mut whitespace,
            &mut absent,
            &mut nonempty,
            &mut empty,
        )
    }
    if has(view, "adjacent_text") && !adjacent {
        return Err(format!("{}: no adjacent text boundary", case.name));
    }
    if has(view, "whitespace_text") && !whitespace {
        return Err(format!("{}: no whitespace-only text", case.name));
    }
    if has(view, "groups") && groups == 0 {
        return Err(format!("{}: no group", case.name));
    }
    if has(view, "option_absent") && absent == 0 {
        return Err(format!("{}: no absent link option", case.name));
    }
    if has(view, "option_nonempty") && nonempty == 0 {
        return Err(format!("{}: no non-empty link option", case.name));
    }
    if has(view, "option_present_empty") && empty == 0 {
        return Err(format!("{}: no present-empty link option", case.name));
    }
    if has(view, "decoded_escapes")
        && leaves
            .iter()
            .any(|(k, v)| k == "TEXT" && [r"\%", r"\{", r"\}", r"\\"].iter().any(|e| v.contains(e)))
    {
        return Err(format!("{}: source escape spelling remained", case.name));
    }
    if has(view, "unicode_exact")
        && !leaves
            .iter()
            .any(|(_, v)| v.contains("e\u{301}") && v.contains("日本語"))
    {
        return Err(format!("{}: expected Unicode not found", case.name));
    }
    Ok(())
}
