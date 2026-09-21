//! Compare installed metadata with R 4.6.1 and verify its derived topic keys.

use std::{collections::HashSet, path::PathBuf, process::Command};

use rd_helpdb::{HelpTopicIndex, PackageHelpDb};

const ORACLE: &str = r#"
if (getRversion() != "4.6.1") quit(status = 42L)
hex <- function(x) {
  if (is.na(x)) return("-")
  paste(sprintf("%02x", as.integer(charToRaw(enc2utf8(x)))), collapse = "")
}
emit <- function(...) cat(paste(..., sep = "\t"), "\n", sep = "")
for (package in c("base", "stats", "utils", "tools")) {
  path <- find.package(package)
  metadata <- readRDS(file.path(path, "Meta", "Rd.rds"))
  emit("package", hex(path), nrow(metadata))
  for (row in seq_len(nrow(metadata))) {
    fields <- c("row", hex(metadata$File[[row]]), hex(metadata$Title[[row]]),
                hex(metadata$Name[[row]]),
                vapply(metadata$Aliases[[row]], hex, ""))
    cat(paste(fields, collapse = "\t"), "\n", sep = "")
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
fn metadata_matches_r_and_keys_address_installed_help_topics() {
    let output = match Command::new("Rscript")
        .args(["--vanilla", "-e", ORACLE])
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("skipping help metadata oracle: Rscript unavailable");
            return;
        }
        Err(error) => panic!("run Rscript: {error}"),
    };
    if output.status.code() == Some(42) {
        eprintln!("skipping help metadata oracle: requires R 4.6.1");
        return;
    }
    assert!(
        output.status.success(),
        "R oracle: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    let mut packages = 0;
    while let Some(line) = lines.next() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields[0], "package");
        let path = PathBuf::from(decode(fields[1]).unwrap());
        let count: usize = fields[2].parse().unwrap();
        let index = HelpTopicIndex::read_installed(&path).unwrap().unwrap();
        assert_eq!(index.len(), count, "{}", path.display());
        let db = PackageHelpDb::open(&path).unwrap();
        let keys: HashSet<_> = db.topics().collect();
        for entry in index.entries() {
            let fields: Vec<_> = lines.next().expect("oracle row").split('\t').collect();
            assert_eq!(fields[0], "row");
            assert_eq!(entry.file.as_str(), decode(fields[1]).as_deref());
            assert_eq!(entry.title.as_str(), decode(fields[2]).as_deref());
            assert_eq!(entry.name.as_str(), decode(fields[3]).as_deref());
            assert_eq!(
                entry.aliases,
                fields[4..]
                    .iter()
                    .map(|value| decode(value))
                    .collect::<Vec<_>>()
            );
            let key = entry.topic_key().expect("installed file name");
            assert!(
                keys.contains(key),
                "{}: missing topic {key:?}",
                path.display()
            );
        }
        packages += 1;
    }
    assert_eq!(packages, 4);
}
