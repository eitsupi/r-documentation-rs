//! Compare all four hsearch matrices with R 4.6.1's installed metadata.

use std::{path::PathBuf, process::Command};

use rd_helpdb::HelpSearchIndex;

const ORACLE: &str = r#"
if (getRversion() != "4.6.1") quit(status = 42L)
hex <- function(x) {
  if (is.na(x)) return("-")
  paste(sprintf("%02x", as.integer(charToRaw(enc2utf8(x)))), collapse = "")
}
emit <- function(...) cat(paste(c(...), collapse = "\t"), "\n", sep = "")
for (package in c("base", "stats", "utils", "tools")) {
  path <- find.package(package)
  index <- readRDS(file.path(path, "Meta", "hsearch.rds"))
  metadata <- readRDS(file.path(path, "Meta", "Rd.rds"))
  description <- read.dcf(file.path(path, "DESCRIPTION"))
  default_encoding <- if ("Encoding" %in% colnames(description)) {
    description[1, "Encoding"]
  } else NULL
  rebuilt <- tools:::.build_hsearch_index(
    metadata, package, default_encoding
  )
  stopifnot(identical(index, rebuilt))
  emit("package", hex(path))
  for (matrix_index in seq_along(index)) {
    matrix <- index[[matrix_index]]
    emit("matrix", matrix_index, nrow(matrix), ncol(matrix),
         vapply(colnames(matrix), hex, ""))
    if (nrow(matrix)) {
      for (row in seq_len(nrow(matrix))) {
        emit("row", vapply(matrix[row, ], hex, ""))
      }
    }
  }
}
"#;

fn decode(value: &str) -> Option<String> {
    if value == "-" {
        return None;
    }
    assert_eq!(value.len() % 2, 0);
    let bytes = value
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    Some(String::from_utf8(bytes).unwrap())
}

#[test]
fn all_hsearch_matrices_match_r() {
    let output = match Command::new("Rscript")
        .args(["--vanilla", "-e", ORACLE])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping help-search oracle: Rscript unavailable");
            return;
        }
        Err(error) => panic!("run Rscript: {error}"),
    };
    if output.status.code() == Some(42) {
        eprintln!("skipping help-search oracle: requires R 4.6.1");
        return;
    }
    assert!(
        output.status.success(),
        "R oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let lines: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    let mut cursor = 0;
    let mut packages = 0;
    while cursor < lines.len() {
        let package: Vec<_> = lines[cursor].split('\t').collect();
        cursor += 1;
        assert_eq!(package[0], "package");
        let path = PathBuf::from(decode(package[1]).unwrap());
        let index = HelpSearchIndex::read_installed(&path).unwrap().unwrap();
        for matrix_index in 0..4 {
            let header: Vec<_> = lines[cursor].split('\t').collect();
            cursor += 1;
            assert_eq!(header[0], "matrix");
            assert_eq!(header[1].parse::<usize>().unwrap(), matrix_index + 1);
            let nrow = header[2].parse::<usize>().unwrap();
            let ncol = header[3].parse::<usize>().unwrap();
            let names: Vec<_> = header[4..]
                .iter()
                .map(|value| decode(value).unwrap())
                .collect();
            assert_eq!(names.len(), ncol);
            let expected_names = match matrix_index {
                0 => vec![
                    "Package", "LibPath", "ID", "Name", "Title", "Topic", "Encoding",
                ],
                1 => vec!["Alias", "ID", "Package"],
                2 => vec!["Keyword", "ID", "Package"],
                3 => vec!["Concept", "ID", "Package"],
                _ => unreachable!(),
            };
            assert_eq!(names, expected_names);
            let actual_len = match matrix_index {
                0 => index.base_len(),
                1 => index.aliases_len(),
                2 => index.keywords_len(),
                3 => index.concepts_len(),
                _ => unreachable!(),
            };
            assert_eq!(nrow, actual_len, "matrix {matrix_index}");
            for row in 0..nrow {
                let values: Vec<_> = lines[cursor].split('\t').collect();
                cursor += 1;
                assert_eq!(values[0], "row");
                assert_eq!(values.len() - 1, ncol, "matrix {matrix_index}, row {row}");
                let values: Vec<_> = values[1..].iter().map(|value| decode(value)).collect();
                let actual: Vec<_> = match matrix_index {
                    0 => {
                        let entry = index.base_entries().nth(row).unwrap();
                        vec![
                            entry.package.clone(),
                            entry.lib_path.clone(),
                            entry.id.clone(),
                            entry.name.clone(),
                            entry.title.clone(),
                            entry.topic.clone(),
                            entry.encoding.clone(),
                        ]
                    }
                    1 => {
                        let entry = index.aliases().nth(row).unwrap();
                        vec![entry.alias.clone(), entry.id.clone(), entry.package.clone()]
                    }
                    2 => {
                        let entry = index.keywords().nth(row).unwrap();
                        vec![
                            entry.keyword.clone(),
                            entry.id.clone(),
                            entry.package.clone(),
                        ]
                    }
                    3 => {
                        let entry = index.concepts().nth(row).unwrap();
                        vec![
                            entry.concept.clone(),
                            entry.id.clone(),
                            entry.package.clone(),
                        ]
                    }
                    _ => unreachable!(),
                };
                assert_eq!(actual, values, "matrix {matrix_index}, row {row}");
            }
        }
        packages += 1;
    }
    assert_eq!(packages, 4);
}
