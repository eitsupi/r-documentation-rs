//! Human-oriented, deliberately incomplete structural synopsis of one decoded
//! RDS object. The output is neither stable nor machine-readable, and this is
//! intentionally not a command-line product: it has no semantic data-frame or
//! package-index interpretation, flags, filters, sorting, output modes,
//! recursive discovery, downloads, color, or paging.
//!
//! ```text
//! cargo run -p rd-rds --example inspect_rds -- <file.rds>
//! ```

use std::process::ExitCode;

use rd_rds::{EnvHandle, RObject, RStr, RValue, file};

const MAX_DEPTH: usize = 4;
const MAX_CHILDREN: usize = 8;
const MAX_NODES: usize = 200;
const DISPLAY_LIMIT: usize = 120;

struct NodeBudget {
    printed: usize,
}

impl NodeBudget {
    fn new() -> Self {
        Self { printed: 0 }
    }

    fn take(&mut self) -> bool {
        if self.printed >= MAX_NODES {
            false
        } else {
            self.printed += 1;
            true
        }
    }
}

fn main() -> ExitCode {
    if let Err(message) = run() {
        eprintln!("{message}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(r"usage: inspect_rds <file.rds>
example: inspect_rds /path/to/archive.rds"
            .to_string());
    }
    let path = &args[0];
    let object = file::read(path).map_err(|error| format_read_error(path, error))?;

    println!("path: {path}");
    let mut budget = NodeBudget::new();
    render_object(&object, "root", 0, 0, &mut budget);
    Ok(())
}

fn render_object(
    object: &RObject,
    label: &str,
    indent: usize,
    depth: usize,
    budget: &mut NodeBudget,
) -> bool {
    if !budget.take() {
        return false;
    }

    println!(
        "{}{}: {}",
        spaces(indent),
        label,
        describe_value(object.value())
    );
    render_attributes(object, indent + 2, depth, budget);
    render_children(object, indent + 2, depth, budget);
    true
}

fn render_attributes(object: &RObject, indent: usize, depth: usize, budget: &mut NodeBudget) {
    let attributes = object.attributes();
    if attributes.is_empty() {
        return;
    }
    println!("{}attributes:", spaces(indent));
    if depth >= MAX_DEPTH {
        print_elision(
            indent + 2,
            attributes.len(),
            attributes.len(),
            "depth limit",
        );
        return;
    }

    let shown = attributes.len().min(MAX_CHILDREN);
    let mut omitted = attributes.len() - shown;
    let mut omission_reason = (omitted > 0).then_some("child limit");
    for (index, attribute) in attributes.iter().take(shown).enumerate() {
        let name = truncate_string(attribute.name().as_str());
        let label = format!("attribute[{index}] {name}");
        if !render_object(attribute.value(), &label, indent + 2, depth + 1, budget) {
            omitted += shown - index;
            omission_reason = Some("node budget");
            break;
        }
    }
    if omitted > 0 {
        print_elision(
            indent + 2,
            attributes.len(),
            omitted,
            omission_reason.unwrap_or("node budget"),
        );
    }
}

fn render_children(object: &RObject, indent: usize, depth: usize, budget: &mut NodeBudget) {
    let RValue::List(values) = object.value() else {
        return;
    };
    println!("{}elements:", spaces(indent));
    if values.is_empty() {
        println!("{}(none)", spaces(indent + 2));
        return;
    }
    if depth >= MAX_DEPTH {
        print_elision(indent + 2, values.len(), values.len(), "depth limit");
        return;
    }

    let shown = values.len().min(MAX_CHILDREN);
    let mut omitted = values.len() - shown;
    let mut omission_reason = (omitted > 0).then_some("child limit");
    for (index, value) in values.iter().take(shown).enumerate() {
        let label = list_label(object, index);
        if !render_object(value, &label, indent + 2, depth + 1, budget) {
            omitted += shown - index;
            omission_reason = Some("node budget");
            break;
        }
    }
    if omitted > 0 {
        print_elision(
            indent + 2,
            values.len(),
            omitted,
            omission_reason.unwrap_or("node budget"),
        );
    }
}

fn list_label(object: &RObject, index: usize) -> String {
    let name = object
        .names()
        .and_then(|names| names.get(index))
        .map(display_name);
    match name {
        Some(name) => format!("element[{index}] name={name}"),
        None => format!("element[{index}]"),
    }
}

fn display_name(value: &RStr) -> String {
    match value {
        RStr::Na => r"<NA>".to_string(),
        RStr::Value { .. } => match value.as_str() {
            Some(Ok(value)) => truncate_string(&value),
            Some(Err(_)) => r"<invalid string>".to_string(),
            None => r"<NA>".to_string(),
        },
        _ => r"<unhandled variant>".to_string(),
    }
}

fn describe_value(value: &RValue) -> String {
    match value {
        RValue::Null => r"kind=NULL length=0 preview=NULL".to_string(),
        RValue::Logical(values) => describe_vector("logical", values.len(), || {
            values
                .iter()
                .take(MAX_CHILDREN)
                .map(|value| match value {
                    Some(value) => value.to_string(),
                    None => r"<NA>".to_string(),
                })
                .collect()
        }),
        RValue::Integer(values) => describe_vector("integer", values.len(), || {
            values
                .iter()
                .take(MAX_CHILDREN)
                .map(|value| match value {
                    Some(value) => value.to_string(),
                    None => r"<NA>".to_string(),
                })
                .collect()
        }),
        RValue::Real(values) => describe_vector("real", values.len(), || {
            values
                .iter()
                .take(MAX_CHILDREN)
                .map(|value| match value {
                    Some(value) => value.to_string(),
                    None => r"<NA>".to_string(),
                })
                .collect()
        }),
        RValue::Character(values) => describe_vector("character", values.len(), || {
            values
                .iter()
                .take(MAX_CHILDREN)
                .map(display_string)
                .collect()
        }),
        RValue::List(values) => {
            format!(r#"kind=list length={} preview=<list>"#, values.len())
        }
        RValue::Symbol(symbol) => {
            format!("kind=symbol preview={}", truncate_string(symbol.as_str()))
        }
        RValue::Persisted(values) => format!(
            "kind=persisted length={} preview=<persisted reference>",
            values.as_slice().len()
        ),
        RValue::Environment(handle) => {
            format!("kind=environment preview={}", describe_environment(handle))
        }
        _ => r"kind=<unhandled variant> preview=<unhandled variant>".to_string(),
    }
}

fn describe_vector(kind: &str, length: usize, values: impl FnOnce() -> Vec<String>) -> String {
    let preview = values().join(", ");
    let omitted = length.saturating_sub(MAX_CHILDREN);
    if omitted == 0 {
        format!("kind={kind} length={length} preview=[{preview}]")
    } else {
        format!(
            "kind={kind} length={length} preview=[{preview}, ...] (original length: {length}, omitted: {omitted})"
        )
    }
}

fn display_string(value: &RStr) -> String {
    match value {
        RStr::Na => r"<NA>".to_string(),
        RStr::Value { .. } => match value.as_str() {
            Some(Ok(value)) => truncate_string(&value),
            Some(Err(_)) => r"<invalid string>".to_string(),
            None => r"<NA>".to_string(),
        },
        _ => r"<unhandled variant>".to_string(),
    }
}

fn truncate_string(value: &str) -> String {
    let mut fragments = Vec::new();
    let mut escaped_length = 0;
    let mut original_length = 0;
    let mut overflowed = false;
    for character in value.chars() {
        original_length += 1;
        if overflowed {
            continue;
        }
        let fragment_length = escaped_width(character);
        if escaped_length + fragment_length > DISPLAY_LIMIT - 2 {
            overflowed = true;
            continue;
        }
        fragments.push((escape_character(character), fragment_length));
        escaped_length += fragment_length;
    }
    if !overflowed {
        let prefix: String = fragments
            .into_iter()
            .map(|(fragment, _)| fragment)
            .collect();
        return format!(r#""{prefix}""#);
    }

    while escaped_length + 5 > DISPLAY_LIMIT {
        let (_, fragment_length) = fragments
            .pop()
            .expect("the limit must accommodate the quotes and ellipsis");
        escaped_length -= fragment_length;
    }
    let included = fragments.len();
    let prefix: String = fragments
        .into_iter()
        .map(|(fragment, _)| fragment)
        .collect();
    format!(
        r#""{prefix}..." (original length: {original_length}, omitted: {})"#,
        original_length - included
    )
}

fn escape_character(character: char) -> String {
    match character {
        '\\' => r"\\".to_string(),
        '"' => r#"\""#.to_string(),
        '\n' => r"\n".to_string(),
        '\r' => r"\r".to_string(),
        '\t' => r"\t".to_string(),
        '\0' => r"\0".to_string(),
        character if character.is_control() => format!(r"\u{{{:x}}}", character as u32),
        character => character.to_string(),
    }
}

// Keep these match arms in sync so the width matches the retained fragment.
fn escaped_width(character: char) -> usize {
    match character {
        '\\' | '"' | '\n' | '\r' | '\t' | '\0' => 2,
        character if character.is_control() => 5 + (character as u32).ilog(16) as usize,
        _ => 1,
    }
}

fn print_elision(indent: usize, original_length: usize, omitted: usize, reason: &str) {
    println!(
        "{}<elided: original length: {original_length}, omitted: {omitted} ({reason})>",
        spaces(indent)
    );
}

fn describe_environment(handle: &EnvHandle) -> &'static str {
    match handle {
        EnvHandle::Global => r"<global environment handle>",
        EnvHandle::Base => r"<base environment handle>",
        EnvHandle::Empty => r"<empty environment handle>",
        EnvHandle::Other => r"<opaque environment handle>",
        _ => r"<unhandled variant>",
    }
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}

fn format_read_error(path: &str, error: file::ReadError) -> String {
    match error {
        file::ReadError::CompressionDisabled { format } => {
            // Keep this as a shell-neutral template. Path quoting depends on the user's
            // shell, which cannot be inferred from the target operating system.
            format!(
                r"cannot read {path}: {format} compression support is disabled
retry with {format} enabled (replace <PATH> with the input path):
  cargo run -p rd-rds --no-default-features --features {format} --example inspect_rds -- <PATH>"
            )
        }
        file::ReadError::UnknownEnvelope { magic } => {
            format!("cannot read {path}: unrecognized RDS envelope (magic {magic:02x?})")
        }
        error => format!("failed to read {path}: {error}"),
    }
}
