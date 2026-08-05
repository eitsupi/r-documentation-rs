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
        println!("  {column}");
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
            println!("    {column}: {value}");
        }
    }
    println!("omitted rows: {}", matrix.len().saturating_sub(ROW_LIMIT));

    Ok(())
}

fn truncate_cell(value: &str) -> String {
    let mut escaped = String::new();
    let mut truncated = false;
    for character in value.chars() {
        let fragment = escape_character(character);
        if escaped.chars().count() + fragment.chars().count() + 3 > DISPLAY_LIMIT {
            truncated = true;
            break;
        }
        escaped.push_str(&fragment);
    }
    if truncated {
        escaped.push_str(r"...");
    }
    escaped
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

fn format_read_error(path: &str, error: file::ReadError) -> String {
    match error {
        file::ReadError::CompressionDisabled { format } => format!(
            r"cannot read {path}: {format} compression support is disabled
retry with: cargo run -p rd-rds --no-default-features --features {format} --example inspect_packages -- {path}"
        ),
        file::ReadError::UnknownEnvelope { magic } => {
            format!("cannot read {path}: unrecognized RDS envelope (magic {magic:02x?})")
        }
        error => format!("failed to read {path}: {error}"),
    }
}
