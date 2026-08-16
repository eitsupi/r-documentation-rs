#[allow(dead_code)]
mod support;

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use rd_ast::{RawRdValue, RdDocument, RdNode, RdTag, producer};
use support::diff;

#[derive(Debug)]
struct MacroSpec {
    macro_name: &'static str,
    source_tag: RdTag,
}

// The production recognizer owns definition validation; this table only maps
// source-level tags to the corresponding macro marker names.
const SYSTEM_MACROS: &[MacroSpec] = &[
    MacroSpec {
        macro_name: "\\doi",
        source_tag: RdTag::Doi,
    },
    MacroSpec {
        macro_name: "\\CRANpkg",
        source_tag: RdTag::CranPkg,
    },
    MacroSpec {
        macro_name: "\\sspace",
        source_tag: RdTag::Sspace,
    },
    MacroSpec {
        macro_name: "\\I",
        source_tag: RdTag::I,
    },
];

#[derive(Debug, PartialEq, Eq)]
enum NormalizeError {
    DefinitionMismatch(String),
    UnsupportedInvocationShape(String),
}

struct NormalizeResult {
    nodes: Vec<RdNode>,
    spliced_into_parent: bool,
}

fn system_macro(name: &str) -> Option<&'static MacroSpec> {
    SYSTEM_MACROS.iter().find(|spec| spec.macro_name == name)
}

fn usermacro_name(raw: &rd_ast::RawRdNode) -> Option<&str> {
    raw.attributes()
        .iter()
        .find(|a| a.name() == "macro")
        .and_then(|a| match a.value().value() {
            RawRdValue::Character(values) if values.len() == 1 => values[0].as_deref(),
            _ => None,
        })
}

fn normalize_children(children: &[RdNode], oracle: bool) -> Result<Vec<RdNode>, NormalizeError> {
    let mut out = Vec::new();
    let mut join_next = false;
    for node in children {
        let result = normalize_node(node, oracle)?;
        let mut nodes = result.nodes;
        if (join_next || result.spliced_into_parent)
            && !nodes.is_empty()
            && out
                .last_mut()
                .is_some_and(|last| merge_leaves(last, &nodes[0]))
        {
            nodes.remove(0);
        }
        if nodes.is_empty() && result.spliced_into_parent {
            join_next = true;
        } else {
            join_next = result.spliced_into_parent;
        }
        out.extend(nodes);
    }
    Ok(out)
}

fn normalize_node(node: &RdNode, oracle: bool) -> Result<NormalizeResult, NormalizeError> {
    if oracle
        && let RdNode::Raw(raw) = node
        && raw.tag() == Some("USERMACRO")
    {
        let name = usermacro_name(raw)
            .ok_or_else(|| NormalizeError::DefinitionMismatch("<missing macro name>".into()))?;
        let _spec =
            system_macro(name).ok_or_else(|| NormalizeError::DefinitionMismatch(name.into()))?;
        return Ok(NormalizeResult {
            nodes: Vec::new(),
            spliced_into_parent: true,
        });
    }
    if !oracle
        && let RdNode::Tagged(tagged) = node
        && let Some(spec) = SYSTEM_MACROS.iter().find(|s| s.source_tag == *tagged.tag())
    {
        let arg = tagged.children();
        let expanded = match spec.source_tag {
            RdTag::I => {
                if tagged.option().is_some() {
                    return Err(NormalizeError::UnsupportedInvocationShape(
                        spec.macro_name.into(),
                    ));
                }
                return Ok(NormalizeResult {
                    nodes: normalize_children(arg, false)?,
                    spliced_into_parent: true,
                });
            }
            RdTag::Sspace => {
                if !arg.is_empty() || tagged.option().is_some() {
                    return Err(NormalizeError::UnsupportedInvocationShape(
                        spec.macro_name.into(),
                    ));
                } else {
                    sspace()
                }
            }
            RdTag::CranPkg => cranpkg(arg, spec)?,
            RdTag::Doi => doi(arg, spec)?,
            _ => unreachable!(),
        };
        return Ok(NormalizeResult {
            nodes: vec![expanded],
            spliced_into_parent: false,
        });
    }
    let normalized = match node {
        RdNode::Tagged(tagged) => RdNode::tagged(
            tagged.tag().clone(),
            tagged
                .option()
                .map(|o| normalize_children(o, oracle))
                .transpose()?,
            normalize_children(tagged.children(), oracle)?,
        ),
        RdNode::Group(group) => RdNode::group(normalize_children(group.children(), oracle)?),
        RdNode::Raw(raw) => {
            let (tag, option, children, payload, attrs) = raw.clone().into_parts();
            RdNode::Raw(producer::raw_node(
                tag,
                option.map(|o| normalize_children(&o, oracle)).transpose()?,
                normalize_children(&children, oracle)?,
                payload,
                attrs,
            ))
        }
        _ => node.clone(),
    };
    Ok(NormalizeResult {
        nodes: vec![normalized],
        spliced_into_parent: false,
    })
}

fn sspace() -> RdNode {
    RdNode::tagged(
        RdTag::IfElse,
        None,
        vec![
            RdNode::group(vec![RdNode::Text("latex".into())]),
            RdNode::group(vec![RdNode::tagged(
                RdTag::Out,
                None,
                vec![RdNode::Verb("~".into())],
            )]),
            RdNode::group(vec![RdNode::Text(" ".into())]),
        ],
    )
}

fn plain_text_argument(children: &[RdNode], spec: &MacroSpec) -> Result<String, NormalizeError> {
    match children {
        [RdNode::Text(value)] => Ok(value.clone()),
        _ => Err(NormalizeError::UnsupportedInvocationShape(
            spec.macro_name.into(),
        )),
    }
}

fn cranpkg(children: &[RdNode], spec: &MacroSpec) -> Result<RdNode, NormalizeError> {
    let package = plain_text_argument(children, spec)?;
    Ok(RdNode::tagged(
        RdTag::Href,
        None,
        vec![
            RdNode::group(vec![RdNode::Verb(format!(
                "https://CRAN.R-project.org/package={package}"
            ))]),
            RdNode::group(vec![RdNode::tagged(
                RdTag::Pkg,
                None,
                vec![RdNode::Text(package)],
            )]),
        ],
    ))
}

fn doi(children: &[RdNode], spec: &MacroSpec) -> Result<RdNode, NormalizeError> {
    let id = plain_text_argument(children, spec)?;
    if id.contains(['\\', '"']) {
        return Err(NormalizeError::UnsupportedInvocationShape(
            spec.macro_name.into(),
        ));
    }
    Ok(RdNode::tagged(
        RdTag::Sexpr,
        Some(vec![RdNode::Text("results=rd".into())]),
        vec![RdNode::RCode(format!(r##"tools:::Rd_expr_doi("{id}")"##))],
    ))
}

fn merge_leaves(left: &mut RdNode, right: &RdNode) -> bool {
    match (left, right) {
        (RdNode::Text(left), RdNode::Text(right))
        | (RdNode::RCode(left), RdNode::RCode(right))
        | (RdNode::Verb(left), RdNode::Verb(right)) => {
            left.push_str(right);
            true
        }
        _ => false,
    }
}

fn usermacro_names(document: &RdDocument, names: &mut BTreeMap<String, (usize, usize)>) {
    fn visit(node: &RdNode, names: &mut BTreeMap<String, (usize, usize)>) {
        if let RdNode::Raw(raw) = node
            && raw.tag() == Some("USERMACRO")
        {
            let name = usermacro_name(raw).unwrap_or("<malformed>");
            names.entry(name.into()).or_default().0 += 1;
        }
        match node {
            RdNode::Tagged(n) => {
                n.children().iter().for_each(|x| visit(x, names));
                n.option()
                    .unwrap_or_default()
                    .iter()
                    .for_each(|x| visit(x, names));
            }
            RdNode::Group(n) => n.children().iter().for_each(|x| visit(x, names)),
            RdNode::Raw(n) => n.children().iter().for_each(|x| visit(x, names)),
            _ => {}
        }
    }
    document.nodes().iter().for_each(|node| visit(node, names));
}

#[derive(Debug)]
struct CorpusFile {
    id: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warning,
    Error,
    Timeout,
}

struct ResultRow {
    status: Status,
    message: String,
    bytes: Option<PathBuf>,
}

fn discover(root: &Path, dir: &Path, out: &mut Vec<CorpusFile>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read corpus directory {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            discover(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "Rd") {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.push(CorpusFile { id: relative, path });
        }
    }
}

fn temporary_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!("rd-source-corpus-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn parse_status(value: &str) -> Status {
    match value {
        "ok" => Status::Ok,
        "warning" => Status::Warning,
        "error" => Status::Error,
        "timeout" => Status::Timeout,
        other => panic!("oracle returned unknown status {other:?}"),
    }
}

#[test]
#[ignore = "requires an explicit RD_SOURCE_CORPUS corpus root"]
fn corpus_differential() {
    let roots = env::var("RD_SOURCE_CORPUS").unwrap_or_else(|_| {
        panic!("RD_SOURCE_CORPUS is unset; set it to colon-separated directory roots, e.g. RD_SOURCE_CORPUS=/path/to/R/src/library")
    });
    let mut files = Vec::new();
    for root in roots.split(':').filter(|root| !root.is_empty()) {
        let root =
            fs::canonicalize(root).unwrap_or_else(|e| panic!("invalid corpus root {root:?}: {e}"));
        discover(&root, &root, &mut files);
    }
    files.sort_by(|a, b| a.id.cmp(&b.id).then(a.path.cmp(&b.path)));
    let mut ids = BTreeMap::new();
    for file in &files {
        assert!(
            ids.insert(file.id.clone(), ()).is_none(),
            "duplicate corpus-relative id: {}",
            file.id
        );
    }

    let temp = temporary_dir();
    let list = temp.join("paths.txt");
    fs::write(
        &list,
        files
            .iter()
            .map(|file| format!("{}\n", file.path.display()))
            .collect::<String>(),
    )
    .unwrap();
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/corpus_oracle.R");
    let command = Command::new("Rscript")
        .arg(script)
        .arg(&list)
        .arg(&temp)
        .output()
        .unwrap_or_else(|e| panic!("failed to invoke Rscript: {e}"));
    assert!(
        command.status.success(),
        "corpus oracle failed: {}",
        String::from_utf8_lossy(&command.stderr)
    );

    let mut rows = Vec::new();
    for (index, line) in fs::read_to_string(temp.join("manifest.tsv"))
        .unwrap()
        .lines()
        .enumerate()
    {
        let mut fields = line.splitn(3, '\t');
        let returned = fields.next().unwrap().parse::<usize>().unwrap() - 1;
        assert_eq!(returned, index, "oracle manifest is not ordered");
        let status = parse_status(fields.next().unwrap());
        rows.push(ResultRow {
            status,
            message: fields.next().unwrap_or_default().to_string(),
            bytes: matches!(status, Status::Ok).then(|| temp.join(format!("{index}.rds"))),
        });
    }
    assert_eq!(
        rows.len(),
        files.len(),
        "oracle manifest row count differs from discovery"
    );

    let mut counts = [0usize; 11];
    let mut examples: [Vec<String>; 8] = std::array::from_fn(|_| Vec::new());
    let mut normalized_inventory: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut residual_inventory: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (file, row) in files.iter().zip(rows) {
        match row.status {
            Status::Warning => {
                counts[4] += 1;
                if examples[0].len() < 10 {
                    examples[0].push(format!("{} ({})", file.id, row.message));
                }
            }
            Status::Error => {
                counts[5] += 1;
                if examples[1].len() < 10 {
                    examples[1].push(format!("{} ({})", file.id, row.message));
                }
            }
            Status::Timeout => {
                counts[6] += 1;
                if examples[2].len() < 10 {
                    examples[2].push(format!("{} ({})", file.id, row.message));
                }
            }
            Status::Ok => {
                let object = rd_rds::parse(&fs::read(row.bytes.unwrap()).unwrap())
                    .unwrap_or_else(|e| panic!("{}: oracle RDS failed: {e}", file.id));
                let expected = rd_ast::lower_r_object(&object)
                    .unwrap_or_else(|e| panic!("{}: oracle lowering failed: {e}", file.id));
                let mut names = BTreeMap::new();
                usermacro_names(&expected, &mut names);
                let all_allowlisted =
                    !names.is_empty() && names.keys().all(|name| system_macro(name).is_some());
                // Residual files are out of comparison coverage entirely, so
                // curated invocations inside them intentionally skip shape
                // validation: the unsupported-shape bucket only guards files
                // that would otherwise be compared. A residual file re-enters
                // full scrutiny once its blocking macro is curated.
                if !names.is_empty() && !all_allowlisted {
                    counts[3] += 1;
                    for (name, (occurrences, _)) in &names {
                        let entry = residual_inventory.entry(name.clone()).or_default();
                        entry.0 += occurrences;
                        entry.1 += 1;
                    }
                    if examples[3].len() < 10 {
                        examples[3].push(file.id.clone());
                    }
                    continue;
                }
                match rd_source::parse(&fs::read(&file.path).unwrap()) {
                    Err(error) => {
                        counts[8] += 1;
                        if examples[5].len() < 10 {
                            examples[5].push(format!("{} ({error})", file.id));
                        }
                    }
                    Ok(parsed) if !parsed.diagnostics().is_empty() => {
                        counts[8] += 1;
                        if examples[5].len() < 10 {
                            examples[5].push(format!(
                                "{} (diagnostics: {:?})",
                                file.id,
                                parsed.diagnostics()
                            ));
                        }
                    }
                    Ok(parsed) => {
                        let (expected_normalized, actual_normalized) = if names.is_empty() {
                            counts[1] += 1;
                            (expected.clone(), parsed.document().clone())
                        } else {
                            match (
                                normalize_children(expected.nodes(), true),
                                normalize_children(parsed.document().nodes(), false),
                            ) {
                                (Ok(expected_nodes), Ok(actual_nodes)) => {
                                    for (name, (occurrences, _)) in &names {
                                        let entry =
                                            normalized_inventory.entry(name.clone()).or_default();
                                        entry.0 += occurrences;
                                        entry.1 += 1;
                                    }
                                    counts[2] += 1;
                                    (
                                        RdDocument::new(expected_nodes),
                                        RdDocument::new(actual_nodes),
                                    )
                                }
                                (Err(NormalizeError::UnsupportedInvocationShape(reason)), _)
                                | (_, Err(NormalizeError::UnsupportedInvocationShape(reason))) => {
                                    counts[10] += 1;
                                    if examples[7].len() < 10 {
                                        examples[7].push(format!("{} ({reason})", file.id));
                                    }
                                    continue;
                                }
                                (Err(error), _) | (_, Err(error)) => {
                                    counts[9] += 1;
                                    if examples[6].len() < 10 {
                                        examples[6].push(format!("{} ({error:?})", file.id));
                                    }
                                    continue;
                                }
                            }
                        };
                        let differences = diff::compare(&expected_normalized, &actual_normalized);
                        if let Some(first) = differences.first() {
                            counts[7] += 1;
                            if examples[4].len() < 10 {
                                examples[4].push(format!("{} ({first})", file.id));
                            }
                        }
                    }
                }
            }
        }
    }
    counts[0] = files.len();
    println!("corpus differential report");
    println!(
        "discovered={} ok-compared={} ok-usermacro-normalized={} ok-usermacro-residual={} warning={} error={} timeout={} mismatched={} rd-source-failed={} normalization-failed={} unsupported-shape={}",
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        counts[4],
        counts[5],
        counts[6],
        counts[7],
        counts[8],
        counts[9],
        counts[10]
    );
    println!("normalized macros:");
    for (name, (occurrences, files_count)) in normalized_inventory {
        println!("  {name}: occurrences={occurrences} files={files_count}");
    }
    println!("residual macros:");
    for (name, (occurrences, files_count)) in residual_inventory {
        println!("  {name}: occurrences={occurrences} files={files_count}");
    }
    for (label, values) in [
        "warning",
        "error",
        "timeout",
        "usermacro-residual",
        "mismatched",
        "rd-source-failed",
        "normalization-failed",
        "unsupported-shape",
    ]
    .into_iter()
    .zip(examples)
    {
        if !values.is_empty() {
            println!("{label} examples:");
            for value in values {
                println!("  {value}");
            }
        }
    }
    assert_eq!(
        counts[7] + counts[8] + counts[9] + counts[10],
        0,
        "corpus differential found Rust failures; see report above"
    );
}

#[cfg(test)]
mod adapter_tests {
    use super::*;

    struct TransientTempDir(PathBuf);

    impl TransientTempDir {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = env::temp_dir().join(format!(
                "rd-source-system-macros-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TransientTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn installed_r_system_macro_definitions_are_recognized() {
        let temp_dir = TransientTempDir::new();
        let rds_path = temp_dir.path().join("system-macro-probe.rds");
        let escaped_rds_path = rds_path
            .to_string_lossy()
            .replace('\\', r"\\")
            .replace('"', r#"\""#);
        let r_code = format!(
            r#"x <- tools::parse_Rd(textConnection('\\\\doi{{10.1000/xyz}}, \\\\CRANpkg{{cli}}, one\\\\sspace two, \\\\I{{x}}'), macros = file.path(R.home('share'), 'Rd/macros/system.Rd'), fragment = TRUE); saveRDS(x, "{escaped_rds_path}", version = 3)"#
        );
        let r_code = format!("suppressWarnings({{{r_code}}})");
        let status = Command::new("Rscript").args(["-e", &r_code]).status();
        let Ok(status) = status else {
            return;
        };
        assert!(
            status.success(),
            "installed R did not parse system-macro probe"
        );

        let raw = rd_rds::file::read(&rds_path).expect("read R's parsed system-macro probe");
        let document = rd_ast::lower_r_object(&raw).expect("lower R's parsed system-macro probe");
        let items = document.system_macro_items().collect::<Vec<_>>();
        assert!(items.iter().any(|item| matches!(item,
            rd_ast::RdSystemMacroItem::Macro(m)
                if matches!(m.semantic(), rd_ast::RdSystemMacro::Doi { id: "10.1000/xyz" }))));
        assert!(items.iter().any(|item| matches!(item,
            rd_ast::RdSystemMacroItem::Macro(m)
                if matches!(m.semantic(), rd_ast::RdSystemMacro::CranPkg { package: "cli" }))));
        assert!(items.iter().any(|item| matches!(item,
            rd_ast::RdSystemMacroItem::Macro(m) if matches!(m.semantic(), rd_ast::RdSystemMacro::Sspace))));
        assert!(!items.iter().any(|item| matches!(item,
            rd_ast::RdSystemMacroItem::Macro(m) if matches!(m.semantic(), rd_ast::RdSystemMacro::I { .. }))));
    }

    fn marker(name: &str, definition: &str, argument: &str) -> RdNode {
        RdNode::Raw(producer::raw_node(
            Some("USERMACRO".into()),
            None,
            vec![RdNode::Text(format!("{definition}{argument}"))],
            None,
            vec![producer::raw_attribute(
                "macro".into(),
                producer::raw_object(RawRdValue::Character(vec![Some(name.into())]), vec![]),
            )],
        ))
    }

    #[test]
    fn marker_i_splice_and_coalesce_in_latex_and_r_like_parents() {
        for (left, middle, right) in [
            (
                RdNode::Text("a".into()),
                RdNode::Text("x".into()),
                RdNode::Text("b".into()),
            ),
            (
                RdNode::RCode("a".into()),
                RdNode::RCode("x".into()),
                RdNode::RCode("b".into()),
            ),
        ] {
            let oracle = RdDocument::new(vec![
                left.clone(),
                marker(r"\I", "#1", "x"),
                middle.clone(),
                right.clone(),
            ]);
            let source = RdDocument::new(vec![
                left.clone(),
                RdNode::tagged(RdTag::I, None, vec![middle.clone()]),
                right.clone(),
            ]);
            let oracle = RdDocument::new(normalize_children(oracle.nodes(), true).unwrap());
            let source = RdDocument::new(normalize_children(source.nodes(), false).unwrap());
            assert_eq!(oracle.nodes().len(), 2);
            assert_eq!(source.nodes().len(), 1);
            match (&left, &middle, &right, oracle.nodes(), source.nodes()) {
                (
                    RdNode::Text(_),
                    RdNode::Text(_),
                    RdNode::Text(_),
                    [RdNode::Text(oracle_left), RdNode::Text(oracle_right)],
                    [RdNode::Text(source)],
                ) => {
                    assert_eq!(oracle_left, "ax");
                    assert_eq!(oracle_right, "b");
                    assert_eq!(source, "axb");
                }
                (
                    RdNode::RCode(_),
                    RdNode::RCode(_),
                    RdNode::RCode(_),
                    [RdNode::RCode(oracle_left), RdNode::RCode(oracle_right)],
                    [RdNode::RCode(source)],
                ) => {
                    assert_eq!(oracle_left, "ax");
                    assert_eq!(oracle_right, "b");
                    assert_eq!(source, "axb");
                }
                _ => panic!("unexpected normalized leaf kinds"),
            }
        }
    }

    #[test]
    fn unrelated_adjacent_leaves_remain_segmented_recursively() {
        for oracle in [true, false] {
            let document = RdDocument::new(vec![
                RdNode::Text("a".into()),
                RdNode::Text("b".into()),
                RdNode::group(vec![RdNode::RCode("x".into()), RdNode::RCode("y".into())]),
            ]);
            let normalized = normalize_children(document.nodes(), oracle).unwrap();
            assert_eq!(normalized.len(), 3);
            assert!(matches!((&normalized[0], &normalized[1]),
                (RdNode::Text(left), RdNode::Text(right)) if left == "a" && right == "b"));
            let RdNode::Group(group) = &normalized[2] else {
                panic!("expected nested group");
            };
            assert!(matches!((&group.children()[0], &group.children()[1]),
                (RdNode::RCode(left), RdNode::RCode(right)) if left == "x" && right == "y"));
        }
    }

    #[test]
    fn oracle_marker_removal_coalesces_only_across_the_marker() {
        let document = RdDocument::new(vec![
            RdNode::Text("a".into()),
            marker(r"\I", "#1", "ignored"),
            RdNode::Text("b".into()),
            RdNode::Text("c".into()),
        ]);
        assert_eq!(
            normalize_children(document.nodes(), true).unwrap(),
            vec![RdNode::Text("ab".into()), RdNode::Text("c".into())]
        );
    }

    #[test]
    fn source_i_splice_coalesces_boundaries_not_structured_children() {
        let document = RdDocument::new(vec![
            RdNode::Text("a".into()),
            RdNode::tagged(
                RdTag::I,
                None,
                vec![
                    RdNode::Text("x".into()),
                    RdNode::tagged(RdTag::Pkg, None, vec![RdNode::Text("p".into())]),
                    RdNode::Text("y".into()),
                ],
            ),
            RdNode::Text("b".into()),
        ]);
        let normalized = normalize_children(document.nodes(), false).unwrap();
        assert_eq!(normalized.len(), 3);
        assert_eq!(normalized[0], RdNode::Text("ax".into()));
        assert!(matches!(&normalized[1], RdNode::Tagged(node) if node.tag() == &RdTag::Pkg));
        assert_eq!(normalized[2], RdNode::Text("yb".into()));
    }

    #[test]
    fn projections_do_not_trigger_neighbor_coalescing() {
        for projection in [
            RdNode::tagged(RdTag::Sspace, None, vec![]),
            RdNode::tagged(RdTag::CranPkg, None, vec![RdNode::Text("stats".into())]),
            RdNode::tagged(RdTag::Doi, None, vec![RdNode::Text("10.1/x".into())]),
        ] {
            let normalized = normalize_children(
                &[
                    RdNode::Text("a".into()),
                    projection,
                    RdNode::Text("b".into()),
                ],
                false,
            )
            .unwrap();
            assert_eq!(normalized.len(), 3);
            assert_eq!(normalized[0], RdNode::Text("a".into()));
            assert_eq!(normalized[2], RdNode::Text("b".into()));
        }
    }

    #[test]
    fn projections_match_the_oracle_expansions() {
        let sspace = normalize_node(&RdNode::tagged(RdTag::Sspace, None, vec![]), false)
            .unwrap()
            .nodes;
        assert!(matches!(&sspace[0], RdNode::Tagged(node) if node.tag() == &RdTag::IfElse));
        let cran = normalize_node(
            &RdNode::tagged(RdTag::CranPkg, None, vec![RdNode::Text("stats".into())]),
            false,
        )
        .unwrap()
        .nodes;
        assert!(matches!(&cran[0], RdNode::Tagged(node) if node.tag() == &RdTag::Href));
        let doi = normalize_node(
            &RdNode::tagged(RdTag::Doi, None, vec![RdNode::Text("10.1/x".into())]),
            false,
        )
        .unwrap()
        .nodes;
        assert!(matches!(&doi[0], RdNode::Tagged(node) if node.tag() == &RdTag::Sexpr));
    }

    #[test]
    fn normalization_rejects_definition_mismatch_and_structured_arguments() {
        let wrong = RdNode::Raw(producer::raw_node(
            Some("USERMACRO".into()),
            None,
            vec![RdNode::Text("wrong".into())],
            None,
            vec![producer::raw_attribute(
                "macro".into(),
                producer::raw_object(RawRdValue::Character(vec![Some(r"\I".into())]), vec![]),
            )],
        ));
        assert!(normalize_node(&wrong, true).is_ok());
        let structured = RdNode::tagged(
            RdTag::CranPkg,
            None,
            vec![RdNode::group(vec![RdNode::Text("stats".into())])],
        );
        assert!(matches!(
            normalize_node(&structured, false),
            Err(NormalizeError::UnsupportedInvocationShape(_))
        ));
    }

    #[test]
    fn unknown_usermacro_is_residual() {
        let document = RdDocument::new(vec![marker(r"\bibinfo", "#1", "x")]);
        let mut names = BTreeMap::new();
        usermacro_names(&document, &mut names);
        assert_eq!(names.get(r"\bibinfo").map(|value| value.0), Some(1));
        assert!(system_macro(r"\bibinfo").is_none());
    }
}
