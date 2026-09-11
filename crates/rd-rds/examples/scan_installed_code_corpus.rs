//! Emit a machine-readable scan of pinned installed-package code databases.
//!
//! This example is used by the scheduled corpus workflow. It deliberately
//! keeps the scanner in a separate process from the R oracle and depends only
//! on the public package-level API.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use rd_rds::{
    SexpKind, file, lazyload,
    package::{
        FailureCause, FormalsInspection, FormalsNotApplicable, FormalsUnavailable, InstalledCodeDb,
        InstalledCodeError, MetadataField, NamespaceMetadata, StoredKind,
    },
};

#[derive(Debug)]
struct ManifestRow {
    package: String,
    version: String,
}

fn usage() -> &'static str {
    "usage: scan_installed_code_corpus --profile PROFILE --library DIR --manifest FILE --output FILE"
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut profile = None;
    let mut library = None;
    let mut manifest = None;
    let mut output = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let slot = match arg.as_str() {
            "--profile" => &mut profile,
            "--library" => &mut library,
            "--manifest" => &mut manifest,
            "--output" => &mut output,
            _ => return Err(format!("unknown argument {arg:?}; {}", usage()).into()),
        };
        *slot = Some(args.next().ok_or(usage())?);
    }
    let profile = profile.ok_or(usage())?;
    let library = PathBuf::from(library.ok_or(usage())?);
    let manifest = PathBuf::from(manifest.ok_or(usage())?);
    let output = PathBuf::from(output.ok_or(usage())?);

    let rows = read_manifest(&manifest, &profile)?;
    if rows.is_empty() {
        return Err(format!("manifest has no packages for profile {profile:?}").into());
    }
    let mut report = String::new();
    for row in rows {
        let package_dir = library.join(&row.package);
        let package_report = scan_package(&package_dir, &row, &profile)?;
        report.push_str(&package_report);
        report.push('\n');
    }
    fs::write(output, report)?;
    Ok(())
}

fn read_manifest(path: &Path, profile: &str) -> Result<Vec<ManifestRow>, Box<dyn Error>> {
    let contents = fs::read_to_string(path)?;
    let mut rows = Vec::new();
    let mut seen = BTreeMap::new();
    for (line_number, line) in contents.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 9 {
            return Err(format!(
                "manifest line {} has {} fields, expected 9",
                line_number + 1,
                fields.len()
            )
            .into());
        }
        if fields[0] != profile {
            continue;
        }
        let package = fields[1].to_owned();
        if seen.insert(package.clone(), ()).is_some() {
            return Err(
                format!("manifest repeats package {package:?} for profile {profile:?}").into(),
            );
        }
        rows.push(ManifestRow {
            package,
            version: fields[2].to_owned(),
        });
    }
    Ok(rows)
}

fn scan_package(
    package_dir: &Path,
    row: &ManifestRow,
    profile: &str,
) -> Result<String, Box<dyn Error>> {
    let db = InstalledCodeDb::open(package_dir)
        .map_err(|error| format!("{}: cannot open code database: {error}", row.package))?;
    let metadata_path = package_dir.join("Meta/nsInfo.rds");
    let declared_names = if metadata_path.exists() {
        let metadata_object = file::read(&metadata_path)
            .map_err(|error| format!("{}: cannot read namespace metadata: {error}", row.package))?;
        let metadata = NamespaceMetadata::from_object(&metadata_object).map_err(|error| {
            format!("{}: cannot parse namespace metadata: {error}", row.package)
        })?;
        match metadata.declared_exports() {
            MetadataField::Present(exports) => exports
                .iter()
                .map(|export| export.exported_name().to_owned())
                .collect::<Vec<_>>(),
            MetadataField::Missing
            | MetadataField::Invalid(_)
            | MetadataField::UnsupportedSchema { .. }
            | _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let mut counts = BTreeMap::<&str, usize>::new();
    for binding in db.stored_bindings() {
        *counts.entry(binding.name()).or_default() += 1;
    }
    let unique_names = counts
        .iter()
        .filter_map(|(name, count)| (*count == 1).then_some(*name))
        .collect::<Vec<_>>();
    let ambiguous_names = counts
        .iter()
        .filter_map(|(name, count)| (*count > 1).then_some(*name))
        .collect::<Vec<_>>();
    let entries = db
        .stored_bindings()
        .iter()
        .enumerate()
        .map(|(index, binding)| scan_binding(&db, index, binding.name(), counts[binding.name()]))
        .collect::<Vec<_>>();

    let mut value = String::new();
    write!(
        value,
        "{{\"schema\":\"installed-code-corpus/v1\",\"profile\":{},\"package\":{},\"package_version\":{},\"stored_entries\":[",
        json_string(profile),
        json_string(&row.package),
        json_string(&row.version)
    )?;
    join_json(&mut value, entries.iter(), |entry, out| out.push_str(entry))?;
    write!(
        value,
        "],\"unique_names\":{},\"ambiguous_names\":{},\"declared_names\":{},\"scanner_error\":null}}",
        json_array(unique_names.iter().copied()),
        json_array(ambiguous_names.iter().copied()),
        json_array(declared_names.iter().map(String::as_str)),
    )?;
    Ok(value)
}

fn scan_binding(db: &InstalledCodeDb, index: usize, name: &str, count: usize) -> String {
    if count > 1 {
        return entry_json(index, name, "ambiguous", EntryDetails::default());
    }
    match db.inspect_stored_binding(name) {
        Ok(inspection) => {
            let (formals_state, formals, phase, offset, reason) =
                formals_json(inspection.formals());
            entry_json(
                index,
                name,
                "ok",
                EntryDetails {
                    kind: Some(inspection.kind()),
                    formals: Some((formals_state, formals)),
                    phase,
                    offset,
                    reason,
                    // Runtime namespace bindings are observed by the R oracle;
                    // this process cannot establish eligibility by itself.
                    oracle_eligible: false,
                    ..EntryDetails::default()
                },
            )
        }
        Err(InstalledCodeError::Record { source, .. }) => entry_json(
            index,
            name,
            "record_error",
            EntryDetails {
                error: Some(source.to_string()),
                error_reason: Some(lazyload_error_reason(&source).to_owned()),
                ..EntryDetails::default()
            },
        ),
        Err(InstalledCodeError::Inspection { failure, .. }) => entry_json(
            index,
            name,
            "prefix_error",
            EntryDetails {
                formals: Some(("unavailable", Vec::new())),
                phase: Some(phase_name(failure.phase())),
                offset: Some(failure.offset()),
                reason: Some(reason_for_cause(failure.cause())),
                error: Some(failure.to_string()),
                ..EntryDetails::default()
            },
        ),
        Err(error) => entry_json(
            index,
            name,
            "record_error",
            EntryDetails {
                error: Some(error.to_string()),
                error_reason: Some("other".to_owned()),
                ..EntryDetails::default()
            },
        ),
    }
}

#[derive(Default)]
struct EntryDetails {
    kind: Option<StoredKind>,
    formals: Option<(&'static str, Vec<String>)>,
    phase: Option<&'static str>,
    offset: Option<usize>,
    reason: Option<String>,
    error: Option<String>,
    error_reason: Option<String>,
    oracle_eligible: bool,
}

fn entry_json(index: usize, name: &str, record_fetch: &str, details: EntryDetails) -> String {
    let (formals_state, formal_values) = details.formals.unwrap_or(("not_observed", Vec::new()));
    let formals_json = json_array(formal_values.iter().map(String::as_str));
    format!(
        "{{\"index\":{index},\"name\":{},\"record_fetch\":{},\"root_kind\":{},\"type_code\":{},\"formals_state\":{},\"formals\":{formals_json},\"formals_phase\":{},\"formals_offset\":{},\"formals_reason\":{},\"record_error\":{},\"record_error_reason\":{},\"oracle_eligible\":{}}}",
        json_string(name),
        json_string(record_fetch),
        details
            .kind
            .map_or_else(|| "null".to_owned(), |kind| json_string(kind_name(kind))),
        details
            .kind
            .map_or_else(|| "null".to_owned(), |kind| type_code(kind).to_string()),
        json_string(formals_state),
        details.phase.map_or_else(|| "null".to_owned(), json_string),
        details
            .offset
            .map_or_else(|| "null".to_owned(), |value| value.to_string()),
        details
            .reason
            .map_or_else(|| "null".to_owned(), |value| json_string(&value)),
        details
            .error
            .map_or_else(|| "null".to_owned(), |value| json_string(&value)),
        details
            .error_reason
            .map_or_else(|| "null".to_owned(), |value| json_string(&value)),
        details.oracle_eligible,
    )
}

fn lazyload_error_reason(error: &lazyload::Error) -> &'static str {
    match error {
        lazyload::Error::Io { .. } => "io",
        lazyload::Error::CompressionUnsupported { .. } => "compression",
        lazyload::Error::RecordRangeOverflow { .. } | lazyload::Error::RecordOutOfRange { .. } => {
            "range"
        }
        lazyload::Error::StoredRecordSizeLimitExceeded { .. } => "stored_size",
        lazyload::Error::DecompressedRecordSizeLimitExceeded { .. } => "decompressed_size",
        lazyload::Error::Decompression { .. } => "decompression",
        lazyload::Error::RecordLengthPrefixMissing
        | lazyload::Error::RecordSizeMismatch { .. }
        | lazyload::Error::TrailingRecordBytes { .. } => "framing",
        lazyload::Error::UnknownReference { .. }
        | lazyload::Error::UnsupportedRecordReference { .. } => "reference",
        lazyload::Error::DataFileChanged | lazyload::Error::IndexChanged => "database_changed",
        _ => "other",
    }
}

fn formals_json(
    formals: &FormalsInspection,
) -> (
    &'static str,
    Vec<String>,
    Option<&'static str>,
    Option<usize>,
    Option<String>,
) {
    match formals {
        FormalsInspection::Available(values) => (
            "available",
            values
                .iter()
                .map(|formal| format!("{}={:?}", formal.name(), formal.default()))
                .collect(),
            None,
            None,
            None,
        ),
        FormalsInspection::NotApplicable(reason) => (
            "not_applicable",
            Vec::new(),
            None,
            None,
            Some(
                match reason {
                    FormalsNotApplicable::BuiltIn => "built_in",
                    FormalsNotApplicable::Special => "special",
                    FormalsNotApplicable::NonClosure => "non_closure",
                    _ => "unknown",
                }
                .to_owned(),
            ),
        ),
        FormalsInspection::Unavailable(reason) => match reason {
            FormalsUnavailable::PromiseNotEvaluated => (
                "unavailable",
                Vec::new(),
                None,
                None,
                Some("promise_not_evaluated".to_owned()),
            ),
            FormalsUnavailable::PersistentReferenceUnresolved => (
                "unavailable",
                Vec::new(),
                None,
                None,
                Some("persistent_reference_unresolved".to_owned()),
            ),
            FormalsUnavailable::Prefix(failure) => (
                "unavailable",
                Vec::new(),
                Some(phase_name(failure.phase())),
                Some(failure.offset()),
                Some(reason_for_cause(failure.cause())),
            ),
            _ => (
                "unavailable",
                Vec::new(),
                None,
                None,
                Some("unknown".to_owned()),
            ),
        },
        _ => (
            "unknown",
            Vec::new(),
            None,
            None,
            Some("unknown".to_owned()),
        ),
    }
}

fn kind_name(kind: StoredKind) -> &'static str {
    match kind {
        SexpKind::Nil => "Nil",
        SexpKind::Sym => "Sym",
        SexpKind::PairList => "PairList",
        SexpKind::Env => "Env",
        SexpKind::Char => "Char",
        SexpKind::Logical => "Logical",
        SexpKind::Integer => "Integer",
        SexpKind::Real => "Real",
        SexpKind::String => "String",
        SexpKind::List => "List",
        SexpKind::ExtPtr => "ExtPtr",
        SexpKind::WeakRef => "WeakRef",
        SexpKind::Persist => "Persist",
        SexpKind::Package => "Package",
        SexpKind::Namespace => "Namespace",
        SexpKind::Ref => "Ref",
        SexpKind::Closure => "Closure",
        SexpKind::BuiltIn => "BuiltIn",
        SexpKind::Special => "Special",
        SexpKind::Promise => "Promise",
        SexpKind::Lang => "Lang",
        SexpKind::DotDotDot => "DotDotDot",
        SexpKind::ByteCode => "ByteCode",
        SexpKind::S4 => "S4",
        SexpKind::Complex => "Complex",
        SexpKind::Raw => "Raw",
        SexpKind::Other(_) => "Other",
        _ => "Unknown",
    }
}

fn type_code(kind: StoredKind) -> u8 {
    match kind {
        SexpKind::Nil => 0,
        SexpKind::Sym => 1,
        SexpKind::PairList => 2,
        SexpKind::Closure => 3,
        SexpKind::Env => 4,
        SexpKind::Promise => 5,
        SexpKind::Lang => 6,
        SexpKind::Special => 7,
        SexpKind::BuiltIn => 8,
        SexpKind::Char => 9,
        SexpKind::Logical => 10,
        SexpKind::Integer => 13,
        SexpKind::Real => 14,
        SexpKind::Complex => 15,
        SexpKind::String => 16,
        SexpKind::DotDotDot => 17,
        SexpKind::List => 19,
        SexpKind::ByteCode => 21,
        SexpKind::ExtPtr => 22,
        SexpKind::WeakRef => 23,
        SexpKind::Raw => 24,
        SexpKind::S4 => 25,
        SexpKind::Persist => 247,
        SexpKind::Package => 248,
        SexpKind::Namespace => 249,
        SexpKind::Ref => 255,
        SexpKind::Other(code) => code,
        _ => 255,
    }
}

fn phase_name(phase: rd_rds::package::FailurePhase) -> &'static str {
    match phase {
        rd_rds::package::FailurePhase::Root => "root",
        rd_rds::package::FailurePhase::Attributes => "attributes",
        rd_rds::package::FailurePhase::Environment => "environment",
        rd_rds::package::FailurePhase::Formals => "formals",
        rd_rds::package::FailurePhase::Default(_) => "default",
        rd_rds::package::FailurePhase::BodyTag => "body_tag",
        _ => "unknown",
    }
}

fn reason_for_cause(cause: &FailureCause) -> String {
    match cause {
        FailureCause::Unsupported { type_code, .. } => format!("unsupported_type_code_{type_code}"),
        FailureCause::Malformed => "malformed".to_owned(),
        FailureCause::ResourceLimit => "resource_limit".to_owned(),
        _ => "unknown".to_owned(),
    }
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{:04x}", character as u32)
                    .expect("writing to String cannot fail");
            }
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

fn json_array<'a>(values: impl Iterator<Item = &'a str>) -> String {
    let mut result = String::from("[");
    for (index, value) in values.enumerate() {
        if index > 0 {
            result.push(',');
        }
        result.push_str(&json_string(value));
    }
    result.push(']');
    result
}

fn join_json<T>(
    output: &mut String,
    values: impl Iterator<Item = T>,
    mut render: impl FnMut(T, &mut String),
) -> Result<(), std::fmt::Error> {
    for (index, value) in values.enumerate() {
        if index > 0 {
            output.push(',');
        }
        render(value, output);
    }
    Ok(())
}
