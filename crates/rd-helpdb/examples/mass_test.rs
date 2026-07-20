//! Local-verification mass scan: walks *every* installed package's help
//! database and reports any decode failures across the whole corpus.
//!
//! This is deliberately an example, not a test: it depends on whatever R
//! packages happen to be installed on the machine it runs on, so it is not
//! part of normal CI. Run it by hand after touching the decoder:
//!
//! ```text
//! cargo run --example mass_test -- [<library_dir>...]
//! ```
//!
//! With no arguments, library directories are discovered by running
//! `Rscript -e "cat(paste(.libPaths(), collapse = \"\n\"))"`. Pass one or
//! more library directories explicitly if `Rscript` is not on `PATH`.
//!
//! For every package that has a compiled help database (`help/<pkg>.rdx`),
//! this checks that:
//! - [`PackageHelpDb::open`] succeeds;
//! - every topic's [`PackageHelpDb::raw_topic`] decodes to an `RValue::List`
//!   with at least one element carrying an `Rd_tag` attribute;
//! - every topic's decoded `RObject` lowers via [`lower_r_object`] into an
//!   `RdDocument` without error;
//! - every reference key's [`PackageHelpDb::reference`] decodes;
//! - [`PackageHelpDb::aliases`] decodes;
//! - [`PackageHelpDb::search_index`] decodes, if `Meta/hsearch.rds` exists.
//!
//! Failures are recorded (not fatal) so the scan can cover the whole
//! corpus in one run. Any failure whose error message mentions RDS type
//! code 238 (ALTREP) is called out separately: prior research over the
//! full CRAN/base corpus found ALTREP never appears in help-DB value
//! trees, so a sighting here would be notable.
//!
//! Beyond decode/lowering success, this also walks every successfully
//! lowered `RdDocument` and classifies every [`RdNode::Raw`] node -- the
//! opaque lossless fallback used whenever a node's shape is genuinely
//! non-canonical (see `rd-ast`'s crate documentation). The only accepted Raw
//! exception is the corpus-backed USERMACRO definition shape; unexpected
//! shapes are reported with examples and fail the scan. Counts remain
//! informational because installed corpora vary by machine.

use std::{
    collections::HashMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use rd_ast::{
    RawNodeClassification, RawRdNode, RdDocument, RdNode, classify_raw_node, lower_r_object,
};
use rd_helpdb::PackageHelpDb;
use rd_rds::{RObject, RValue};

/// A single recorded failure: which package/unit failed and why.
struct Failure {
    package: String,
    unit: String,
    message: String,
}

impl Failure {
    /// Whether the error message mentions RDS type code 238 (ALTREP).
    fn is_altrep(&self) -> bool {
        self.message.contains("type code 238")
    }
}

/// Running totals accumulated across every scanned library directory.
#[derive(Default)]
struct Summary {
    packages_scanned: usize,
    packages_skipped: usize,
    packages_opened_failed: usize,
    total_topics: usize,
    total_references: usize,
    packages_without_hsearch: usize,
    failures: Vec<Failure>,
    raw_stats: RawFallbackStats,
}

/// Maximum number of `package::topic` example locations kept per Raw-node
/// tag bucket, so the final report stays actionable rather than dumping
/// every occurrence.
const MAX_EXAMPLES_PER_TAG: usize = 5;

/// Aggregated statistics about the `RObject` -> `RdDocument` lowering pass
/// and, in particular, how often it falls back to [`RdNode::Raw`].
#[derive(Default)]
struct RawFallbackStats {
    /// Topics for which [`lower_r_object`] returned `Ok`.
    topics_lowered: usize,
    /// Of those, how many contained at least one `RdNode::Raw` node.
    topics_with_raw: usize,
    /// Topics for which [`lower_r_object`] returned `Err`.
    lowering_failures: usize,
    /// Histogram of genuine Raw-node occurrences keyed by the node's `tag`
    /// (Raw nodes without a tag are bucketed under `<untagged-raw>`).
    tag_counts: HashMap<String, TagBucket>,
    expected_usermacro: usize,
    unexpected: Vec<UnexpectedBucket>,
}

/// Per-tag occurrence count and a handful of example locations, for one
/// bucket of [`RawFallbackStats::tag_counts`].
#[derive(Default)]
struct TagBucket {
    count: usize,
    examples: Vec<String>,
}

struct UnexpectedBucket {
    tag: Option<String>,
    reason: String,
    offending_attributes: Vec<String>,
    parent_tag: Option<String>,
    count: usize,
    examples: Vec<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let library_dirs = if args.is_empty() {
        match discover_library_paths() {
            Ok(paths) => paths,
            Err(message) => {
                eprintln!("{message}");
                eprintln!(
                    "usage: mass_test [<library_dir>...]\n\
                     pass one or more R library directories explicitly if Rscript is unavailable"
                );
                return ExitCode::FAILURE;
            }
        }
    } else {
        args.into_iter().map(PathBuf::from).collect()
    };

    let mut summary = Summary::default();
    for lib_dir in &library_dirs {
        scan_library(lib_dir, &mut summary);
    }

    for message in summary.raw_stats.unexpected_messages() {
        record_failure(&mut summary, "<raw-guard>", "Raw", message);
    }

    print_final_summary(&summary);

    if summary.failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Discovers library paths via `Rscript -e "cat(paste(.libPaths(), ...))"`.
fn discover_library_paths() -> Result<Vec<PathBuf>, String> {
    let output = Command::new("Rscript")
        .args(["-e", r#"cat(paste(.libPaths(), collapse = "\n"))"#])
        .output()
        .map_err(|err| format!("failed to run Rscript to discover library paths: {err}"))?;

    if !output.status.success() {
        return Err(format!(
            "Rscript exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<PathBuf> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect();

    if paths.is_empty() {
        return Err("Rscript reported no library paths (.libPaths() was empty)".to_string());
    }

    Ok(paths)
}

/// Enumerates package subdirectories of `lib_dir` (sorted) and scans each
/// one that has a compiled help database; others are counted as skipped.
fn scan_library(lib_dir: &Path, summary: &mut Summary) {
    let entries = match fs::read_dir(lib_dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!(
                "failed to read library directory {}: {err}",
                lib_dir.display()
            );
            record_failure(
                summary,
                &lib_dir.display().to_string(),
                "<read_dir>",
                err.to_string(),
            );
            return;
        }
    };

    let mut pkg_dirs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    pkg_dirs.sort();

    println!("== library: {} ==", lib_dir.display());

    for pkg_dir in pkg_dirs {
        let Some(pkg) = pkg_dir.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        let rdx_path = pkg_dir.join("help").join(format!("{pkg}.rdx"));
        if !rdx_path.is_file() {
            summary.packages_skipped += 1;
            continue;
        }
        scan_package(&pkg_dir, pkg, summary);
    }
}

/// Scans a single package's help database: open, every topic, every
/// reference key, aliases, and (if present) the help-search index.
fn scan_package(pkg_dir: &Path, pkg: &str, summary: &mut Summary) {
    summary.packages_scanned += 1;
    let failures_before = summary.failures.len();

    let db = match PackageHelpDb::open(pkg_dir) {
        Ok(db) => db,
        Err(err) => {
            summary.packages_opened_failed += 1;
            record_failure(summary, pkg, "<open>", err.to_string());
            println!("{pkg}: 0 topics, 0 refs, FAILED (1 failures)");
            return;
        }
    };

    let mut n_topics = 0usize;
    for topic in db.topics() {
        n_topics += 1;
        summary.total_topics += 1;

        let rd = match db.raw_topic(topic) {
            Ok(rd) => rd,
            Err(err) => {
                record_failure(summary, pkg, topic, err.to_string());
                continue;
            }
        };

        if let Err(message) = check_topic_shape(&rd) {
            record_failure(summary, pkg, topic, message);
        }

        // Lower the already-decoded RObject rather than decoding the
        // topic a second time -- see the module documentation.
        match lower_r_object(&rd) {
            Ok(document) => {
                summary.raw_stats.topics_lowered += 1;
                let location = format!("{pkg}::{topic}");
                let raw_count = count_raw_nodes(&document, &location, &mut summary.raw_stats);
                if raw_count > 0 {
                    summary.raw_stats.topics_with_raw += 1;
                }
            }
            Err(err) => {
                summary.raw_stats.lowering_failures += 1;
                record_failure(summary, pkg, topic, format!("lowering failed: {err}"));
            }
        }
    }

    let mut n_refs = 0usize;
    for key in db.reference_keys() {
        n_refs += 1;
        summary.total_references += 1;
        // The decoded record is dropped immediately at the end of this
        // statement -- we only care whether decoding succeeded.
        if let Err(err) = db.reference(key) {
            record_failure(summary, pkg, key, err.to_string());
        }
    }

    if let Err(err) = db.aliases() {
        record_failure(summary, pkg, "<aliases>", err.to_string());
    }

    let hsearch_path = pkg_dir.join("Meta").join("hsearch.rds");
    if hsearch_path.is_file() {
        if let Err(err) = db.search_index() {
            record_failure(summary, pkg, "<hsearch>", err.to_string());
        }
    } else {
        summary.packages_without_hsearch += 1;
    }

    let package_failures = summary.failures.len() - failures_before;
    if package_failures == 0 {
        println!("{pkg}: {n_topics} topics, {n_refs} refs, ok");
    } else {
        println!("{pkg}: {n_topics} topics, {n_refs} refs, FAILED ({package_failures} failures)");
    }
}

/// Checks the shape every decoded Rd help topic must have: a `List` with
/// at least one element carrying an `Rd_tag` attribute.
fn check_topic_shape(rd: &RObject) -> Result<(), String> {
    let RValue::List(items) = &rd.value() else {
        return Err("topic did not decode as an RValue::List".to_string());
    };
    if !items
        .iter()
        .any(|item| item.attributes().get("Rd_tag").is_some())
    {
        return Err("no top-level element carries an Rd_tag attribute".to_string());
    }
    Ok(())
}

/// Walks every node of `document` (recursively, including `Tagged`
/// options/children and `Raw` options/children) and tallies each
/// [`RdNode::Raw`] encountered into `stats`, crediting `location`
/// (`package::topic`) as an example site. Returns the number of Raw nodes
/// found in this document.
fn count_raw_nodes(document: &RdDocument, location: &str, stats: &mut RawFallbackStats) -> usize {
    count_raw_in_nodes(document.nodes(), location, stats, None)
}

fn count_raw_in_nodes(
    nodes: &[RdNode],
    location: &str,
    stats: &mut RawFallbackStats,
    parent_tag: Option<&str>,
) -> usize {
    nodes
        .iter()
        .map(|node| count_raw_in_node(node, location, stats, parent_tag))
        .sum()
}

fn count_raw_in_node(
    node: &RdNode,
    location: &str,
    stats: &mut RawFallbackStats,
    parent_tag: Option<&str>,
) -> usize {
    match node {
        RdNode::Text(_) | RdNode::RCode(_) | RdNode::Verb(_) | RdNode::Comment(_) => 0,
        RdNode::Tagged(tagged) => {
            let mut count = 0;
            if let Some(option) = tagged.option() {
                count +=
                    count_raw_in_nodes(option, location, stats, Some(tagged.tag().as_rd_tag()));
            }
            count
                + count_raw_in_nodes(
                    tagged.children(),
                    location,
                    stats,
                    Some(tagged.tag().as_rd_tag()),
                )
        }
        RdNode::Group(group) => count_raw_in_nodes(group.children(), location, stats, parent_tag),
        RdNode::Raw(raw) => {
            record_raw_node(raw, location, stats, parent_tag);
            let mut count = 1;
            if let Some(option) = raw.option() {
                count += count_raw_in_nodes(option, location, stats, raw.tag());
            }
            count + count_raw_in_nodes(raw.children(), location, stats, raw.tag())
        }
        // Future RdNode variants are intentionally not counted until their
        // recursive children and Raw semantics have been defined here.
        _ => 0,
    }
}

/// Records one `RdNode::Raw` occurrence into the tag histogram, adding
/// `location` as an example site (up to [`MAX_EXAMPLES_PER_TAG`] per tag,
/// deduplicated since a single topic can contain multiple Raw nodes of the
/// same tag).
fn record_raw_node(
    raw: &RawRdNode,
    location: &str,
    stats: &mut RawFallbackStats,
    parent_tag: Option<&str>,
) {
    let key = raw.tag().unwrap_or("<untagged-raw>").to_string();
    let bucket = stats.tag_counts.entry(key).or_default();
    bucket.count += 1;
    if bucket.examples.len() < MAX_EXAMPLES_PER_TAG
        && !bucket.examples.iter().any(|e| e == location)
    {
        bucket.examples.push(location.to_string());
    }

    match classify_raw_node(raw) {
        RawNodeClassification::ExpectedUserMacroDefinition => stats.expected_usermacro += 1,
        RawNodeClassification::Unexpected(unexpected) => {
            stats.record_unexpected(
                unexpected.tag().map(str::to_string),
                format!("{:?}", unexpected.reason()),
                unexpected.offending_attributes().to_vec(),
                parent_tag.map(str::to_string),
                location,
            );
        }
        _ => {
            stats.record_unexpected(
                raw.tag().map(str::to_string),
                "UnrecognizedClassification".to_string(),
                Vec::new(),
                parent_tag.map(str::to_string),
                location,
            );
        }
    }
}

impl RawFallbackStats {
    fn record_unexpected(
        &mut self,
        tag: Option<String>,
        reason: String,
        offending_attributes: Vec<String>,
        parent_tag: Option<String>,
        location: &str,
    ) {
        let index = self.unexpected.iter().position(|bucket| {
            bucket.tag == tag
                && bucket.reason == reason
                && bucket.offending_attributes == offending_attributes
                && bucket.parent_tag == parent_tag
        });
        let index = index.unwrap_or_else(|| {
            self.unexpected.push(UnexpectedBucket {
                tag,
                reason,
                offending_attributes,
                parent_tag,
                count: 0,
                examples: Vec::new(),
            });
            self.unexpected.len() - 1
        });
        let bucket = &mut self.unexpected[index];
        bucket.count += 1;
        if bucket.examples.len() < MAX_EXAMPLES_PER_TAG
            && !bucket.examples.iter().any(|example| example == location)
        {
            bucket.examples.push(location.to_string());
        }
    }

    fn unexpected_messages(&self) -> Vec<String> {
        self.unexpected
            .iter()
            .map(|bucket| {
                format!(
                    "unexpected Raw shape: tag={}, reason={}, offending attributes={}, parent tag={}, count={}, examples={}",
                    bucket.tag.as_deref().unwrap_or("<untagged>"),
                    bucket.reason,
                    if bucket.offending_attributes.is_empty() { "<none>".to_string() } else { bucket.offending_attributes.join(", ") },
                    bucket.parent_tag.as_deref().unwrap_or("<none>"),
                    bucket.count,
                    bucket.examples.join(", ")
                )
            })
            .collect()
    }
}

fn record_failure(summary: &mut Summary, package: &str, unit: &str, message: String) {
    summary.failures.push(Failure {
        package: package.to_string(),
        unit: unit.to_string(),
        message,
    });
}

fn print_final_summary(summary: &Summary) {
    println!();
    println!("== summary ==");
    println!(
        "packages scanned: {} (skipped: {}, failed to open: {})",
        summary.packages_scanned, summary.packages_skipped, summary.packages_opened_failed
    );
    println!("total topics checked: {}", summary.total_topics);
    println!(
        "total reference records checked: {}",
        summary.total_references
    );
    println!(
        "packages without Meta/hsearch.rds: {}",
        summary.packages_without_hsearch
    );
    println!("total failures: {}", summary.failures.len());

    if !summary.failures.is_empty() {
        println!();
        let shown = summary.failures.len().min(20);
        println!("first {shown} failure(s):");
        for failure in summary.failures.iter().take(20) {
            println!(
                "  {}::{}: {}",
                failure.package, failure.unit, failure.message
            );
        }
    }

    let altrep: Vec<&Failure> = summary.failures.iter().filter(|f| f.is_altrep()).collect();
    println!();
    if altrep.is_empty() {
        println!("ALTREP sightings: none");
    } else {
        println!("== ALTREP encountered ({}) ==", altrep.len());
        for failure in &altrep {
            println!(
                "  {}::{}: {}",
                failure.package, failure.unit, failure.message
            );
        }
    }

    print_raw_fallback_summary(&summary.raw_stats);
}

/// Prints Raw totals, the informational expected-category count, and any
/// unexpected structural categories with example locations.
fn print_raw_fallback_summary(stats: &RawFallbackStats) {
    println!();
    println!("== lowering / Raw-fallback summary ==");
    println!("topics lowered successfully: {}", stats.topics_lowered);
    println!("topics that failed to lower: {}", stats.lowering_failures);
    println!(
        "topics containing at least one Raw node: {}",
        stats.topics_with_raw
    );

    let total_raw_nodes: usize = stats.tag_counts.values().map(|bucket| bucket.count).sum();
    println!("total Raw node occurrences: {total_raw_nodes}");
    println!(
        "expected USERMACRO definition Raw nodes (informational): {}",
        stats.expected_usermacro
    );
    println!(
        "unexpected Raw shape categories: {}",
        stats.unexpected.len()
    );

    for message in stats.unexpected_messages() {
        println!("  {message}");
    }

    if stats.tag_counts.is_empty() {
        println!("no Raw nodes encountered");
        return;
    }

    let mut tags: Vec<(&String, &TagBucket)> = stats.tag_counts.iter().collect();
    tags.sort_by(|(a_tag, a_bucket), (b_tag, b_bucket)| {
        b_bucket
            .count
            .cmp(&a_bucket.count)
            .then_with(|| a_tag.cmp(b_tag))
    });

    println!();
    println!("Raw node tag histogram ({} distinct tags):", tags.len());
    for (tag, bucket) in tags {
        println!("  {tag}: {}", bucket.count);
        for example in &bucket.examples {
            println!("      e.g. {example}");
        }
    }
}
