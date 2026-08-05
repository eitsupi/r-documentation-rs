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
    let original_length = value.chars().count();
    let escaped_length = value
        .chars()
        .map(escape_character)
        .map(|fragment| fragment.chars().count())
        .sum::<usize>();
    if escaped_length + 2 <= DISPLAY_LIMIT {
        let escaped: String = value.chars().map(escape_character).collect();
        return format!(r#""{escaped}""#);
    }

    let mut prefix = String::new();
    let mut included = 0;
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        let escaped = escape_character(character);
        let suffix_length = if characters.peek().is_some() { 5 } else { 2 };
        if prefix.chars().count() + escaped.chars().count() + suffix_length > DISPLAY_LIMIT {
            break;
        }
        prefix.push_str(&escaped);
        included += 1;
    }
    format!(
        r#""{prefix}..."" (original length: {original_length}, omitted: {})"#,
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
            let quoted_path = shell_quote(path);
            format!(
                r"cannot read {path}: {format} compression support is disabled
retry with: cargo run -p rd-rds --no-default-features --features {format} --example inspect_rds -- {quoted_path}"
            )
        }
        file::ReadError::UnknownEnvelope { magic } => {
            format!("cannot read {path}: unrecognized RDS envelope (magic {magic:02x?})")
        }
        error => format!("failed to read {path}: {error}"),
    }
}

fn shell_quote(path: &str) -> String {
    format!("'{}'", path.replace('\'', r"'\''"))
}
