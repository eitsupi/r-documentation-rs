//! Compares `rd-ast`'s high-level [`RdDocument`] view accessors --
//! [`RdDocument::title`], [`RdDocument::arguments`], and [`text_contents`]
//! -- against an independent R-side re-implementation of the same
//! semantics, applied to the same real installed-package topics used by
//! `oracle_read_topic.rs`.
//!
//! This differs from `oracle_read_topic.rs`: that test compares
//! `rd-helpdb`'s raw [`rd_rds::RObject`] lookup against R's own
//! `tools:::fetchRdDB()`. Here, `rd-helpdb` performs the same lookup, but
//! the comparison is one layer up -- after `rd_ast::lower_r_object()` --
//! and checks the *interpreted* view (title text, argument name/description
//! pairs), not just the raw tree shape.
//!
//! ## The R mirror tracks the Rust lowering's attribute allowlist
//!
//! `rd-ast`'s lowering (`rds.rs`, see its module docs) produces
//! `RdNode::Tagged` for a node whose attributes fall within
//! `{Rd_tag, Rd_option, srcref}` -- `srcref` is known producer metadata,
//! deliberately discarded on the structured path. Any attribute outside
//! that set forces the fully-lossless `RdNode::Raw` fallback, which
//! `rd-ast`'s view accessors deliberately never interpret. The R side
//! mirrors exactly that: `lowers_to_tagged()` below counts an element as
//! structurally lowered only when its attribute set is a subset of
//! `{Rd_tag, Rd_option, srcref}`, so both sides agree on which `\title`/
//! `\arguments`/`\item` nodes participate in the view.
//!
//! Since `tools::parse_Rd()` attaches `srcref` to every node it produces
//! (verified empirically -- there is no way to suppress it in this R
//! build, so it's on every node of a real compiled help database too),
//! this test genuinely exercises the non-empty path: all three topics
//! resolve a title, and `utils::person`'s full argument list is compared
//! pair by pair. The `srcref` values in real help databases also carry
//! `PERSISTSXP`-persisted `srcfile` references, so this doubles as the
//! real-data check that srcref discarding never inspects the discarded
//! value. The trailing asserts on `titles_compared`/`arguments_compared`
//! pin that this non-empty coverage doesn't silently regress.

use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use rd_ast::{RdDocument, RdNode, RdTag, text_contents};
use rd_helpdb::PackageHelpDb;

/// `(package, topic)` pairs to compare -- the same trio used by
/// `oracle_read_topic.rs`.
const TOPICS: &[(&str, &str)] = &[
    ("utils", "flush.console"),
    ("utils", "person"),
    ("base", "AsIs"),
];

const ORACLE_SCRIPT_BODY: &str = r#"
rd_text_contents <- function(x) {
  tag <- attr(x, "Rd_tag", exact = TRUE)
  if (identical(tag, "COMMENT")) return("")
  if (is.character(x)) return(paste(x[!is.na(x)], collapse = ""))
  if (is.list(x)) return(paste(vapply(x, rd_text_contents, character(1)), collapse = ""))
  ""
}
normalize_text <- function(x) trimws(gsub("[[:space:]]+", " ", x))

# Mirrors rds.rs's per-node attribute allowlist: a node only lowers to
# RdNode::Tagged (rather than RdNode::Raw) when its attribute set falls
# within {Rd_tag, Rd_option, srcref} -- srcref is known producer metadata
# the lowering deliberately discards. See the file-level doc comment on
# the Rust side.
lowers_to_tagged <- function(x) {
  extra <- setdiff(names(attributes(x)), c("Rd_tag", "Rd_option", "srcref"))
  length(extra) == 0
}

# First top-level element with Rd_tag == tag_name that also lowers to
# Tagged (lowers_to_tagged, and is a list rather than a character leaf).
find_first_tagged <- function(rd, tag_name) {
  for (el in rd) {
    if (identical(attr(el, "Rd_tag", exact = TRUE), tag_name) &&
        is.list(el) && lowers_to_tagged(el)) {
      return(el)
    }
  }
  NULL
}

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

  title_node <- find_first_tagged(rd, "\\title")
  title_present <- !is.null(title_node)
  cat("title_present\t", as.integer(title_present), "\n", sep = "")
  if (title_present) {
    cat("title\t", normalize_text(rd_text_contents(title_node)), "\n", sep = "")
  }

  args_node <- find_first_tagged(rd, "\\arguments")
  arguments_present <- !is.null(args_node)
  cat("arguments_present\t", as.integer(arguments_present), "\n", sep = "")

  argument_count <- 0
  if (arguments_present) {
    items <- list()
    for (el in args_node) {
      tag <- attr(el, "Rd_tag", exact = TRUE)
      if (!identical(tag, "\\item")) next
      if (!is.null(attr(el, "Rd_option", exact = TRUE))) next
      if (!is.list(el) || !lowers_to_tagged(el) || length(el) != 2) next
      g1 <- el[[1]]
      g2 <- el[[2]]
      if (!is.null(attr(g1, "Rd_tag", exact = TRUE))) next
      if (!is.null(attr(g2, "Rd_tag", exact = TRUE))) next
      items[[length(items) + 1]] <- list(name = g1, desc = g2)
    }
    argument_count <- length(items)
    for (i in seq_along(items)) {
      nm <- normalize_text(rd_text_contents(items[[i]]$name))
      ds <- normalize_text(rd_text_contents(items[[i]]$desc))
      cat("argument\t", i - 1, "\t", nm, "\t", ds, "\n", sep = "")
    }
  }
  cat("argument_count\t", argument_count, "\n", sep = "")

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
    title_present: Option<bool>,
    title: Option<String>,
    arguments_present: Option<bool>,
    argument_count: Option<usize>,
    arguments: Vec<(usize, String, String)>,
}

/// Parses the oracle script's line-oriented `tag\tvalue` stdout into one
/// block per `topic\t.../end` section, matching `oracle_read_topic.rs`'s
/// parsing conventions (a bare `end` sentinel line, stray untagged lines
/// ignored).
fn parse_oracle_output(stdout: &str) -> Vec<TopicBlock> {
    let mut blocks = Vec::new();
    let mut current: Option<TopicBlock> = None;

    for line in stdout.lines() {
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
            "title_present" => {
                if let Some(block) = current.as_mut() {
                    block.title_present = Some(rest.trim() == "1");
                }
            }
            "title" => {
                if let Some(block) = current.as_mut() {
                    block.title = Some(rest.to_string());
                }
            }
            "arguments_present" => {
                if let Some(block) = current.as_mut() {
                    block.arguments_present = Some(rest.trim() == "1");
                }
            }
            "argument_count" => {
                if let Some(block) = current.as_mut() {
                    block.argument_count = rest.trim().parse().ok();
                }
            }
            "argument" => {
                if let Some(block) = current.as_mut() {
                    let mut fields = rest.splitn(3, '\t');
                    let (Some(idx), Some(name), Some(description)) =
                        (fields.next(), fields.next(), fields.next())
                    else {
                        continue;
                    };
                    if let Ok(idx) = idx.trim().parse::<usize>() {
                        block
                            .arguments
                            .push((idx, name.to_string(), description.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    blocks
}

/// `true` iff `document` has a top-level `Tagged` node with `RdTag::Arguments`
/// -- i.e. whether `\arguments` itself was recognized (as
/// opposed to `RdDocument::arguments()`'s item count, which is also zero
/// when `\arguments` is altogether absent or lowered to `Raw`).
///
/// The public `RdDocument::nodes()` accessor allows this ad hoc structural
/// check beyond the curated `view` accessors.
fn arguments_present(document: &RdDocument) -> bool {
    document
        .nodes()
        .iter()
        .any(|node| matches!(node, RdNode::Tagged(tagged) if tagged.tag() == &RdTag::Arguments))
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn oracle_matches_rd_ast_view() {
    if !rscript_available() {
        println!("skipping: Rscript not found");
        return;
    }

    let work_dir = std::env::temp_dir().join(format!(
        "rd-helpdb-oracle-rd-ast-{}-{}",
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
    let mut titles_compared = 0usize;
    let mut argument_pairs_compared = 0usize;
    let mut person_argument_pairs_compared = None;
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
            "oracle topic {}: title_present={:?} title={:?} arguments_present={:?} argument_count={:?} arguments={:?}",
            block.topic,
            block.title_present,
            block.title,
            block.arguments_present,
            block.argument_count,
            block.arguments
        );

        let db = PackageHelpDb::open(pkg_dir).unwrap_or_else(|err| {
            panic!(
                "{}: open help db at {}: {err}",
                block.topic,
                pkg_dir.display()
            )
        });
        let raw = db
            .raw_topic(&block.topic)
            .unwrap_or_else(|err| panic!("{}: raw_topic failed: {err}", block.topic));
        let document: RdDocument = rd_ast::lower_r_object(&raw)
            .unwrap_or_else(|err| panic!("{}: lower_r_object failed: {err}", block.topic));

        // -- title --
        let rust_title_present = document.title().is_some();
        let oracle_title_present = block
            .title_present
            .unwrap_or_else(|| panic!("{}: missing title_present fact", block.topic));
        assert_eq!(
            rust_title_present, oracle_title_present,
            "{}: title presence doesn't match the oracle",
            block.topic
        );
        if oracle_title_present {
            let rust_title = normalize_whitespace(&text_contents(
                document
                    .title()
                    .expect("title() is Some when rust_title_present"),
            ));
            let oracle_title = block
                .title
                .as_ref()
                .unwrap_or_else(|| panic!("{}: missing title fact", block.topic));
            assert_eq!(
                &rust_title, oracle_title,
                "{}: normalized title text doesn't match the oracle",
                block.topic
            );
            titles_compared += 1;
        }

        // -- arguments --
        let rust_arguments_present = arguments_present(&document);
        let oracle_arguments_present = block
            .arguments_present
            .unwrap_or_else(|| panic!("{}: missing arguments_present fact", block.topic));
        assert_eq!(
            rust_arguments_present, oracle_arguments_present,
            r"{}: \arguments presence doesn't match the oracle",
            block.topic
        );

        let rust_arguments: Vec<(String, String)> = document
            .arguments()
            .map(|argument| {
                (
                    normalize_whitespace(&text_contents(argument.name)),
                    normalize_whitespace(&text_contents(argument.description)),
                )
            })
            .collect();
        let oracle_argument_count = block
            .argument_count
            .unwrap_or_else(|| panic!("{}: missing argument_count fact", block.topic));
        assert_eq!(
            rust_arguments.len(),
            oracle_argument_count,
            "{}: argument count doesn't match the oracle",
            block.topic
        );

        let mut oracle_arguments: Vec<(usize, String, String)> = block.arguments.clone();
        oracle_arguments.sort_by_key(|(idx, _, _)| *idx);
        let mut topic_argument_pairs_compared = 0usize;
        for (index, (rust_name, rust_description)) in rust_arguments.iter().enumerate() {
            let (oracle_idx, oracle_name, oracle_description) = oracle_arguments
                .get(index)
                .unwrap_or_else(|| panic!("{}: missing oracle argument {index}", block.topic));
            assert_eq!(
                *oracle_idx, index,
                "{}: oracle argument index gap at {index}",
                block.topic
            );
            assert_eq!(
                rust_name, oracle_name,
                "{}: argument {index} name doesn't match the oracle",
                block.topic
            );
            assert_eq!(
                rust_description, oracle_description,
                "{}: argument {index} description doesn't match the oracle",
                block.topic
            );
            argument_pairs_compared += 1;
            topic_argument_pairs_compared += 1;
        }
        if block.topic == "person" {
            person_argument_pairs_compared = Some(topic_argument_pairs_compared);
        }

        compared += 1;
    }

    assert!(
        compared > 0,
        "no topics were actually compared (all skipped):\n{stdout}"
    );
    // Since srcref is discarded producer metadata, real help-database
    // topics lower structurally: every compared topic must have produced a
    // real (non-empty-path) title comparison, and at least one topic
    // (utils::person) a pairwise argument comparison.
    assert_eq!(
        titles_compared, compared,
        "every compared topic should have a title on both sides:\n{stdout}"
    );
    assert!(
        argument_pairs_compared > 0,
        "expected at least one argument name/description pair to be compared:\n{stdout}"
    );
    assert_eq!(
        person_argument_pairs_compared,
        Some(16),
        "utils::person should compare exactly 16 argument pairs:\n{stdout}"
    );

    let _ = fs::remove_dir_all(&work_dir);
}
