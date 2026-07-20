//! Compares `rd-helpdb`'s *end-to-end* topic lookup -- `.rdx` offset
//! lookup, `.rdb` seek, zlib decompression, and `rd_rds::parse`, all done
//! by `PackageHelpDb` itself -- against R's own `tools:::fetchRdDB()`.
//!
//! This differs from `crates/rd-rds/tests/oracle_fetchrddb.rs`: there, R
//! extracts the raw `.rdb` record bytes and Rust only parses them. Here, R
//! is only the oracle of *facts* (attribute names, class, child count, per
//! child `Rd_tag`, alias count); `rd-helpdb` performs the whole lookup
//! itself and we compare the resulting [`rd_rds::RObject`] against those
//! facts.
//!
//! If `Rscript` isn't available in the environment, the test prints a clear
//! message and passes trivially rather than failing.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use rd_helpdb::PackageHelpDb;
use rd_rds::{RObject, RValue};

/// `(package, topic)` pairs to compare -- the same trio used by the
/// `rd-rds`-level oracle test.
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
  cat("pkg_dir\t", pkg_dir, "\n", sep = "")

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

  for (el in rd) {
    tag <- attr(el, "Rd_tag")
    if (is.null(tag)) {
      cat("child_tag\tNA\n")
    } else {
      cat("child_tag\t", tag, "\n", sep = "")
    }
  }

  aliases_path <- file.path(pkg_dir, "help", "aliases.rds")
  alias_count <- tryCatch(length(readRDS(aliases_path)), error = function(e) NA)
  cat("alias_count\t", alias_count, "\n", sep = "")

  cat("end\n")
}
"#;

fn build_oracle_script(topics: &[(&str, &str)]) -> String {
    let mut script = String::new();
    script.push_str("args <- commandArgs(trailingOnly = TRUE)\n");
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
    pkg_dir: Option<PathBuf>,
    attrs: Vec<String>,
    class: Vec<String>,
    n_children: usize,
    child_tags: Vec<String>,
    alias_count: Option<usize>,
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
            "pkg_dir" => {
                if let Some(block) = current.as_mut() {
                    block.pkg_dir = Some(PathBuf::from(rest));
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
            "child_tag" => {
                if let Some(block) = current.as_mut() {
                    block.child_tags.push(rest.to_string());
                }
            }
            "alias_count" => {
                if let Some(block) = current.as_mut() {
                    block.alias_count = rest.trim().parse().ok();
                }
            }
            _ => {}
        }
    }

    blocks
}

/// Extracts the `Rd_tag` attribute of a root list element as either its
/// string value or the literal `"NA"`, matching the oracle's encoding.
fn child_rd_tag(obj: &RObject) -> String {
    let Some(tag_attr) = obj.attributes().get("Rd_tag") else {
        return "NA".to_string();
    };
    let RValue::Character(values) = &tag_attr.value() else {
        return "NA".to_string();
    };
    values
        .first()
        .and_then(|value| value.as_str())
        .and_then(Result::ok)
        .map(|text| text.into_owned())
        .unwrap_or_else(|| "NA".to_string())
}

#[test]
fn oracle_matches_read_topic() {
    if !rscript_available() {
        println!("skipping: Rscript not found");
        return;
    }

    let work_dir = std::env::temp_dir().join(format!(
        "rd-helpdb-oracle-{}-{}",
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

        let pkg_dir = block
            .pkg_dir
            .as_ref()
            .unwrap_or_else(|| panic!("{}: missing pkg_dir in oracle output", block.topic));

        println!(
            "oracle topic {}: pkg_dir={} attrs={:?} class={:?} n_children={} child_tags={:?} alias_count={:?}",
            block.topic,
            pkg_dir.display(),
            block.attrs,
            block.class,
            block.n_children,
            block.child_tags,
            block.alias_count
        );

        let db = PackageHelpDb::open(pkg_dir).unwrap_or_else(|err| {
            panic!(
                "{}: open help db at {}: {err}",
                block.topic,
                pkg_dir.display()
            )
        });

        let root = db
            .raw_topic(&block.topic)
            .unwrap_or_else(|err| panic!("{}: raw_topic failed: {err}", block.topic));

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

        let child_tags: Vec<String> = children.iter().map(child_rd_tag).collect();
        assert_eq!(
            child_tags, block.child_tags,
            "{}: per-child Rd_tag sequence doesn't match the oracle",
            block.topic
        );

        let alias_count = db.aliases().expect("aliases()").len();
        if let Some(expected) = block.alias_count {
            assert_eq!(
                alias_count, expected,
                "{}: aliases().len() doesn't match the oracle's alias_count",
                block.topic
            );
        }

        compared += 1;
    }

    assert!(
        compared > 0,
        "no topics were actually compared (all skipped):\n{stdout}"
    );

    let _ = fs::remove_dir_all(&work_dir);
}
