//! End-to-end demo: look up a topic (or alias) in an installed package's
//! help database, lower it into a canonical [`rd_ast::RdDocument`], and
//! print its title, aliases, and argument list via `rd-ast`'s high-level
//! view accessors.
//!
//! ```text
//! cargo run --example read_topic -- <pkg_dir> <topic-or-alias>
//! ```

use std::process::ExitCode;

use rd_ast::{RdDocument, text_contents};
use rd_helpdb::PackageHelpDb;

fn main() -> ExitCode {
    if let Err(message) = run() {
        eprintln!("{message}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: read_topic <pkg_dir> <topic-or-alias>\n\
             example: read_topic /opt/R/4.6.1/lib/R/library/utils person"
            .to_string());
    }
    let pkg_dir = &args[0];
    let topic_or_alias = &args[1];

    let db = PackageHelpDb::open(pkg_dir)
        .map_err(|err| format!("failed to open package help db at {pkg_dir}: {err}"))?;

    let topic = if db.topics().any(|topic| topic == topic_or_alias) {
        topic_or_alias.clone()
    } else {
        match db
            .resolve_alias(topic_or_alias)
            .map_err(|err| format!("resolve_alias({topic_or_alias:?}) failed: {err}"))?
        {
            Some(resolved) => {
                println!("alias '{topic_or_alias}' -> topic '{resolved}'");
                resolved.to_string()
            }
            None => {
                return Err(format!(
                    "'{topic_or_alias}' is neither a known topic nor a known alias in {}",
                    db.package()
                ));
            }
        }
    };

    let raw = db
        .raw_topic(&topic)
        .map_err(|err| format!("raw_topic({topic:?}) failed: {err}"))?;
    let document: RdDocument = rd_ast::lower_r_object(&raw)
        .map_err(|err| format!("lower_r_object for topic {topic:?} failed: {err}"))?;

    println!("package: {}", db.package());
    println!("topic: {topic}");

    let title = document
        .title()
        .map(|nodes| normalize_whitespace(&text_contents(nodes)))
        .unwrap_or_else(|| r"(no \title section)".to_string());
    println!("title: {title}");

    println!("aliases:");
    for alias in document.aliases() {
        println!("  {}", normalize_whitespace(&alias));
    }

    println!("arguments:");
    let mut printed_any_argument = false;
    for argument in document.arguments() {
        printed_any_argument = true;
        let name = normalize_whitespace(&text_contents(argument.name));
        let description = normalize_whitespace(&text_contents(argument.description));
        println!("  {name}: {description}");
    }
    if !printed_any_argument {
        println!("  (none)");
    }

    Ok(())
}

/// Collapses runs of whitespace (including newlines) into single spaces and
/// trims the result.
fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
