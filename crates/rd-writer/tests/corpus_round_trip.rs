use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

use rd_writer::{UnserializableKind, WriteError};

#[derive(Debug)]
struct CorpusFile {
    id: String,
    path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct CorpusExpectation {
    total: usize,
    unparsed: usize,
    unserializable: usize,
    round_tripped: usize,
}

impl CorpusExpectation {
    fn parse(value: &str) -> Self {
        let mut fields = BTreeMap::new();
        let parts: Vec<_> = value.split_whitespace().collect();
        if parts.len() != 4 {
            panic!(
                "RD_WRITER_CORPUS_EXPECT must contain exactly four fields: total=<n> unparsed=<n> unserializable=<n> round_tripped=<n>"
            );
        }
        for part in parts {
            let (name, number) = part
                .split_once('=')
                .filter(|(name, number)| !name.is_empty() && !number.is_empty())
                .unwrap_or_else(|| panic!("malformed RD_WRITER_CORPUS_EXPECT field {part:?}"));
            if !matches!(
                name,
                "total" | "unparsed" | "unserializable" | "round_tripped"
            ) {
                panic!("unknown RD_WRITER_CORPUS_EXPECT field {name:?}");
            }
            let number = number
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("malformed RD_WRITER_CORPUS_EXPECT value {part:?}"));
            if fields.insert(name, number).is_some() {
                panic!("duplicate RD_WRITER_CORPUS_EXPECT field {name:?}");
            }
        }
        Self {
            total: fields
                .remove("total")
                .expect("missing total in RD_WRITER_CORPUS_EXPECT"),
            unparsed: fields
                .remove("unparsed")
                .expect("missing unparsed in RD_WRITER_CORPUS_EXPECT"),
            unserializable: fields
                .remove("unserializable")
                .expect("missing unserializable in RD_WRITER_CORPUS_EXPECT"),
            round_tripped: fields
                .remove("round_tripped")
                .expect("missing round_tripped in RD_WRITER_CORPUS_EXPECT"),
        }
    }
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

fn kind_label(kind: &UnserializableKind) -> String {
    kind.to_string()
}

#[test]
#[ignore = "requires an explicit RD_SOURCE_CORPUS corpus root"]
fn corpus_round_trip() {
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

    let mut ids = BTreeSet::new();
    for file in &files {
        assert!(
            ids.insert(file.id.clone()),
            "duplicate corpus-relative id: {}",
            file.id
        );
    }

    let mut unparsed = 0;
    let mut unserializable = BTreeMap::<String, (usize, Vec<String>)>::new();
    let mut round_tripped = 0;
    let mut mismatches = Vec::new();

    for file in &files {
        let bytes = fs::read(&file.path).unwrap_or_else(|error| {
            mismatches.push(format!("{}: failed to read input: {error}", file.id));
            Vec::new()
        });
        let parsed = match rd_source::parse(&bytes) {
            Ok(parsed) if parsed.diagnostics().is_empty() => parsed,
            Ok(_) | Err(_) => {
                unparsed += 1;
                continue;
            }
        };

        let source = match rd_writer::write_document(parsed.document()) {
            Ok(source) => source,
            Err(WriteError::Unserializable { kind, .. }) => {
                let entry = unserializable.entry(kind_label(&kind)).or_default();
                entry.0 += 1;
                if entry.1.len() < 10 {
                    entry.1.push(file.id.clone());
                }
                continue;
            }
            Err(error) => {
                mismatches.push(format!("{}: writer error: {error}", file.id));
                continue;
            }
        };

        let reparsed = match rd_source::parse(source.as_bytes()) {
            Ok(reparsed) if reparsed.diagnostics().is_empty() => reparsed,
            Ok(reparsed) => {
                mismatches.push(format!(
                    "{}: reparsed output has diagnostics: {:?}",
                    file.id,
                    reparsed.diagnostics()
                ));
                continue;
            }
            Err(error) => {
                mismatches.push(format!("{}: reparsed output failed: {error}", file.id));
                continue;
            }
        };
        if reparsed.document() != parsed.document() {
            mismatches.push(format!("{}: reparsed document differs", file.id));
        } else {
            round_tripped += 1;
        }
    }

    println!("corpus writer round-trip report");
    let observed = CorpusExpectation {
        total: files.len(),
        unparsed,
        unserializable: unserializable.values().map(|(count, _)| count).sum(),
        round_tripped,
    };
    println!(
        "total={} unparsed={} unserializable={} round_tripped={} mismatches={}",
        observed.total,
        observed.unparsed,
        observed.unserializable,
        observed.round_tripped,
        mismatches.len()
    );
    if !unserializable.is_empty() {
        println!("unserializable inventory:");
        for (kind, (count, examples)) in &unserializable {
            println!("  {kind}: occurrences={count} examples={examples:?}");
        }
    }
    if !mismatches.is_empty() {
        println!("mismatch examples:");
        for mismatch in &mismatches {
            println!("  {mismatch}");
        }
    }

    assert!(!files.is_empty(), "corpus contains no .Rd files");
    assert!(
        mismatches.is_empty(),
        "corpus writer round-trip found mismatches: {mismatches:?}"
    );
    if let Ok(value) = env::var("RD_WRITER_CORPUS_EXPECT") {
        assert_eq!(observed, CorpusExpectation::parse(&value));
    }
}
