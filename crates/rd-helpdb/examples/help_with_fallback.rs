//! Read help by a `Meta/Rd.rds` alias, preserving titles when full help fails.
//!
//! cargo run -p rd-helpdb --example help_with_fallback -- <pkg_dir> <alias>

use std::{path::Path, process::ExitCode};

use rd_ast::{RdDocument, text_contents_lossy};
use rd_helpdb::{Error, HelpTopicIndex, PackageHelpDb};

pub(crate) struct Help {
    pub title: Option<String>,
    pub document: Option<RdDocument>,
}

pub(crate) fn lookup(package_dir: &Path, alias: &str) -> Result<Option<Help>, Error> {
    let Some(index) = HelpTopicIndex::read_installed(package_dir)? else {
        return Ok(None);
    };
    let Some(entry) = index.find_alias(alias) else {
        return Ok(None);
    };
    // A consumer can present metadata even when the selected page is missing
    // or corrupt. Applications can also log these optional-help failures.
    let document = entry.topic_key().and_then(|key| {
        let db = PackageHelpDb::open(package_dir).ok()?;
        let raw = db.raw_topic(key).ok()?;
        rd_ast::lower_r_object(&raw).ok()
    });
    let title = entry.title.as_str().map(str::to_owned).or_else(|| {
        document
            .as_ref()?
            .title_lossy()
            .map(|field| text_contents_lossy(field.body()))
    });
    if title.is_none() && document.is_none() {
        return Ok(None);
    }
    Ok(Some(Help { title, document }))
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [package_dir, alias] = args.as_slice() else {
        eprintln!("usage: help_with_fallback <pkg_dir> <alias>");
        return ExitCode::FAILURE;
    };
    let Some(alias) = alias.to_str() else {
        eprintln!("alias must be UTF-8");
        return ExitCode::FAILURE;
    };
    match lookup(Path::new(package_dir), alias) {
        Ok(Some(help)) => {
            println!(
                "title: {}",
                help.title.as_deref().unwrap_or("(unavailable)")
            );
            println!("full help available: {}", help.document.is_some());
            ExitCode::SUCCESS
        }
        Ok(None) => {
            eprintln!("no help for {alias:?}");
            ExitCode::FAILURE
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
