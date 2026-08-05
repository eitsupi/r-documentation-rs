//! Small demonstration of the typed [`rd_rds::package::PackagesMatrix`] view.
//! It reads one `PACKAGES.rds` file, prints a fixed-size row synopsis, and is
//! intentionally not a command-line product: there are no filters, sorting,
//! output modes, recursive discovery, downloads, color, or paging, and the
//! output is not a stable machine-readable format.
//!
//! ```text
//! cargo run -p rd-rds --example inspect_packages -- <PACKAGES.rds>
//! ```

use std::process::ExitCode;

use rd_rds::{file, package::PackagesMatrix};

const DISPLAY_LIMIT: usize = 120;
const ROW_LIMIT: usize = 3;

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
        return Err(r"usage: inspect_packages <PACKAGES.rds>
example: inspect_packages /path/to/PACKAGES.rds"
            .to_string());
    }
    let path = &args[0];
    let object = file::read(path).map_err(|error| format_read_error(path, error))?;
    let matrix = PackagesMatrix::from_object(&object)
        .map_err(|error| format!("failed to view {path} as a PACKAGES matrix: {error}"))?;
    let columns: Vec<&str> = matrix.column_names().collect();

    println!("path: {path}");
    println!(
        "dimensions: {} rows, {} columns",
        matrix.len(),
        columns.len()
    );
    println!("columns:");
    for column in &columns {
        println!("  {}", display_label(column));
    }

    println!("rows:");
    for row_index in 0..matrix.len().min(ROW_LIMIT) {
        println!("  row {}:", row_index + 1);
        let row = matrix
            .row(row_index)
            .expect("row index is bounded by the matrix length");
        for column in &columns {
            let value = match row.get(column).expect("column came from the matrix") {
                Some(value) => truncate_cell(value),
                None => r"<NA>".to_string(),
            };
            println!("    {}: {value}", display_label(column));
        }
    }
    println!("omitted rows: {}", matrix.len().saturating_sub(ROW_LIMIT));

    Ok(())
}

fn truncate_cell(value: &str) -> String {
    truncate_cell_with_limit(value, DISPLAY_LIMIT)
}

fn truncate_cell_with_limit(value: &str, limit: usize) -> String {
    let mut fragments = Vec::new();
    let mut escaped_length = 0;
    let mut overflowed = false;
    for character in value.chars() {
        let fragment_length = escaped_width(character);
        if escaped_length + fragment_length > limit {
            overflowed = true;
            break;
        }
        fragments.push((escape_character(character), fragment_length));
        escaped_length += fragment_length;
    }
    if !overflowed {
        return fragments
            .into_iter()
            .map(|(fragment, _)| fragment)
            .collect();
    }

    while escaped_length + 3 > limit {
        let (_, fragment_length) = fragments
            .pop()
            .expect("the limit must accommodate the ellipsis");
        escaped_length -= fragment_length;
    }
    let mut escaped: String = fragments
        .into_iter()
        .map(|(fragment, _)| fragment)
        .collect();
    escaped.push_str(r"...");
    escaped
}

fn display_label(value: &str) -> String {
    format!(
        r#""{}""#,
        truncate_cell_with_limit(value, DISPLAY_LIMIT - 2)
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

fn format_read_error(path: &str, error: file::ReadError) -> String {
    match error {
        file::ReadError::CompressionDisabled { format } => {
            let quoted_path = shell_quote(path);
            format!(
                r"cannot read {path}: {format} compression support is disabled
retry with: cargo run -p rd-rds --no-default-features --features {format} --example inspect_packages -- {quoted_path}"
            )
        }
        file::ReadError::UnknownEnvelope { magic } => {
            format!("cannot read {path}: unrecognized RDS envelope (magic {magic:02x?})")
        }
        error => format!("failed to read {path}: {error}"),
    }
}

fn shell_quote(path: &str) -> String {
    // POSIX shells and Windows cmd.exe use different quoting syntax. The Windows
    // branch targets PowerShell; legacy cmd.exe may re-expand a literal '%' in a path.
    if cfg!(windows) {
        format!(r#""{path}""#)
    } else {
        format!("'{}'", path.replace('\'', r"'\''"))
    }
}
