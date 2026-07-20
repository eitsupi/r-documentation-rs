//! Compares `rd-rds`'s decoding of REAL installed-package help-DB entries
//! against R's own `tools:::fetchRdDB()`, at the attribute-structure level.
//!
//! This is the regression net for reference-table misalignment: a
//! misaligned reference table famously manifests as the root `class`
//! attribute silently turning into `Rd_option` (or some other stray
//! attribute name) instead of `Rd`, so this test asserts the full ordered
//! attribute-name list, not just `class`'s value.
//!
//! The test drives a small R helper script (written to a tempdir at test
//! time, never committed) that, for each `(pkg, topic)` pair: emits the
//! oracle facts about `tools:::fetchRdDB()`'s result, and extracts the raw
//! `.rdb` record bytes for that topic so this test can decode them directly
//! with `rd_rds::parse` and compare.
//!
//! If `Rscript` isn't available in the environment, the test prints a clear
//! message and passes trivially rather than failing.

use std::{
    fs,
    io::Read,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use flate2::read::ZlibDecoder;
use rd_rds::RValue;

/// `(package, topic)` pairs to compare. The topic name is the Rd DB key,
/// which for these three happens to equal the file basename.
const TOPICS: &[(&str, &str)] = &[
    ("utils", "flush.console"),
    ("utils", "person"),
    ("base", "AsIs"),
];

const ORACLE_SCRIPT_BODY: &str = r#"
for (spec in topics) {
  pkg <- spec$pkg
  topic <- spec$topic
  cat("topic\t", topic, "\n", sep = "")

  pkg_dir <- tryCatch(find.package(pkg), error = function(e) NULL)
  if (is.null(pkg_dir)) {
    cat("skip\tpackage not found: ", pkg, "\n", sep = "")
    cat("end\n")
    next
  }

  db <- file.path(pkg_dir, "help", pkg)
  rd <- tryCatch(tools:::fetchRdDB(db, topic), error = function(e) NULL)
  if (is.null(rd)) {
    cat("skip\tfetchRdDB failed for ", pkg, "::", topic, "\n", sep = "")
    cat("end\n")
    next
  }

  for (nm in names(attributes(rd))) {
    cat("attr\t", nm, "\n", sep = "")
  }
  for (cls in class(rd)) {
    cat("class\t", cls, "\n", sep = "")
  }
  cat("n_children\t", length(rd), "\n", sep = "")

  rdx <- tryCatch(readRDS(paste0(db, ".rdx")), error = function(e) NULL)
  var <- if (!is.null(rdx)) rdx$variables[[topic]] else NULL
  if (is.null(var)) {
    cat("skip\tno .rdx entry for ", topic, "\n", sep = "")
    cat("end\n")
    next
  }

  con <- file(paste0(db, ".rdb"), "rb")
  seek(con, where = var[1])
  raw <- readBin(con, "raw", n = var[2])
  close(con)

  record_path <- file.path(out_dir, paste0(topic, ".rdbrecord"))
  out_con <- file(record_path, "wb")
  writeBin(raw, out_con)
  close(out_con)
  cat("record\t", record_path, "\n", sep = "")

  cat("end\n")
}
"#;

fn build_oracle_script(topics: &[(&str, &str)]) -> String {
    let mut script = String::new();
    script.push_str("args <- commandArgs(trailingOnly = TRUE)\n");
    script.push_str("out_dir <- args[[1]]\n");
    script.push_str("topics <- list(\n");
    for (index, (pkg, topic)) in topics.iter().enumerate() {
        if index > 0 {
            script.push_str(",\n");
        }
        script.push_str(&format!("  list(pkg = {pkg:?}, topic = {topic:?})"));
    }
    script.push_str("\n)\n");
    script.push_str(ORACLE_SCRIPT_BODY);
    script
}

fn rscript_available() -> bool {
    Command::new("Rscript")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[derive(Debug, Default)]
struct TopicBlock {
    topic: String,
    skip: Option<String>,
    attrs: Vec<String>,
    class: Vec<String>,
    n_children: usize,
    record: Option<PathBuf>,
}

/// Parses the oracle script's line-oriented `tag\tvalue` stdout into one
/// block per `topic\t.../end` section. Stray lines without a tab (e.g.
/// unrelated R startup messages) are ignored rather than causing a panic.
fn parse_oracle_output(stdout: &str) -> Vec<TopicBlock> {
    let mut blocks = Vec::new();
    let mut current: Option<TopicBlock> = None;

    for line in stdout.lines() {
        // "end" is a bare untagged sentinel line (no value, so no tab).
        if line == "end" {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            continue;
        }

        let Some((tag, rest)) = line.split_once('\t') else {
            continue;
        };
        match tag {
            "topic" => {
                current = Some(TopicBlock {
                    topic: rest.to_string(),
                    ..Default::default()
                });
            }
            "skip" => {
                if let Some(block) = current.as_mut() {
                    block.skip = Some(rest.to_string());
                }
            }
            "attr" => {
                if let Some(block) = current.as_mut() {
                    block.attrs.push(rest.to_string());
                }
            }
            "class" => {
                if let Some(block) = current.as_mut() {
                    block.class.push(rest.to_string());
                }
            }
            "n_children" => {
                if let Some(block) = current.as_mut() {
                    block.n_children = rest.trim().parse().expect("n_children is a number");
                }
            }
            "record" => {
                if let Some(block) = current.as_mut() {
                    block.record = Some(PathBuf::from(rest));
                }
            }
            _ => {}
        }
    }

    blocks
}

/// Checks `s` matches `^env::[0-9]+$` without pulling in a regex dependency:
/// real help DBs persist srcfile environments via refhook as `"env::N"`
/// strings, and a wrong reference-table classification here (e.g. decoding
/// as a plain environment instead of `Persisted`) is exactly the kind of
/// drift this test exists to catch.
fn is_env_ref_string(s: &str) -> bool {
    match s.strip_prefix("env::") {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()),
        None => false,
    }
}

#[test]
fn oracle_matches_fetchrddb() {
    if !rscript_available() {
        println!("skipping: Rscript not found");
        return;
    }

    let work_dir = std::env::temp_dir().join(format!(
        "rd-rds-oracle-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    fs::create_dir_all(&work_dir).expect("create oracle work dir");

    let script_path = work_dir.join("oracle.R");
    fs::write(&script_path, build_oracle_script(TOPICS)).expect("write oracle R script");

    let output = Command::new("Rscript")
        .arg(&script_path)
        .arg(&work_dir)
        .output()
        .expect("run oracle R script");
    assert!(
        output.status.success(),
        "oracle R script failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("oracle stdout is UTF-8");
    let blocks = parse_oracle_output(&stdout);
    assert_eq!(
        blocks.len(),
        TOPICS.len(),
        "oracle script produced {} topic blocks, expected {}:\n{stdout}",
        blocks.len(),
        TOPICS.len()
    );

    let mut compared = 0usize;
    for block in &blocks {
        if let Some(reason) = &block.skip {
            println!("skipping topic {}: {reason}", block.topic);
            continue;
        }

        println!(
            "oracle topic {}: attrs={:?} class={:?} n_children={}",
            block.topic, block.attrs, block.class, block.n_children
        );

        let record_path = block
            .record
            .as_ref()
            .unwrap_or_else(|| panic!("{}: missing record path in oracle output", block.topic));
        let record_bytes = fs::read(record_path)
            .unwrap_or_else(|err| panic!("{}: read record file: {err}", block.topic));
        assert!(
            record_bytes.len() >= 4,
            "{}: record file too short for a length prefix",
            block.topic
        );

        let declared_len =
            u32::from_be_bytes(record_bytes[..4].try_into().expect("4 bytes")) as usize;
        let mut decoder = ZlibDecoder::new(&record_bytes[4..]);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .unwrap_or_else(|err| panic!("{}: zlib decompress record: {err}", block.topic));
        assert_eq!(
            decompressed.len(),
            declared_len,
            "{}: decompressed length doesn't match the 4-byte BE prefix",
            block.topic
        );

        let root = rd_rds::parse(&decompressed)
            .unwrap_or_else(|err| panic!("{}: rd_rds::parse failed: {err}", block.topic));

        let root_attr_names: Vec<String> = root
            .attributes()
            .iter()
            .map(|attribute| attribute.name().as_str().to_string())
            .collect();
        assert_eq!(
            root_attr_names, block.attrs,
            "{}: root attribute names/order don't match the oracle",
            block.topic
        );
        assert!(
            !root_attr_names.iter().any(|name| name == "Rd_option"),
            "{}: root has a stray Rd_option attribute -- likely reference-table misalignment",
            block.topic
        );

        let class: Vec<String> = root
            .class()
            .expect("root has a class attribute")
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .expect("class value is not NA")
                    .expect("class value decodes")
                    .into_owned()
            })
            .collect();
        assert_eq!(class, vec!["Rd".to_string()], "{}: root class", block.topic);
        assert_eq!(
            class, block.class,
            "{}: root class doesn't match the oracle",
            block.topic
        );

        let RValue::List(children) = &root.value() else {
            panic!("{}: root did not decode as a List", block.topic);
        };
        assert_eq!(
            children.len(),
            block.n_children,
            "{}: root child count doesn't match the oracle's length(rd)",
            block.topic
        );

        if let Some(srcref) = root.attributes().get("srcref")
            && let Some(srcfile) = srcref.attributes().get("srcfile")
        {
            let RValue::Persisted(persisted) = &srcfile.value() else {
                panic!(
                    "{}: srcref's srcfile attribute did not decode as Persisted \
                     (reference-table drift)",
                    block.topic
                );
            };
            for value in persisted.as_slice() {
                let text = value
                    .as_str()
                    .unwrap_or_else(|| panic!("{}: srcfile persisted string is NA", block.topic))
                    .unwrap_or_else(|err| {
                        panic!(
                            "{}: srcfile persisted string decode error: {err}",
                            block.topic
                        )
                    });
                assert!(
                    is_env_ref_string(&text),
                    "{}: srcfile persisted string {text:?} doesn't match ^env::[0-9]+$",
                    block.topic
                );
            }
        }

        compared += 1;
    }

    assert!(
        compared > 0,
        "no topics were actually compared (all skipped):\n{stdout}"
    );

    let _ = fs::remove_dir_all(&work_dir);
}
