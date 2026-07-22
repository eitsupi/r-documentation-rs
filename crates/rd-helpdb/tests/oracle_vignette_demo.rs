//! Compares typed vignette/demo indexes against R's `readRDS()` results for
//! installed standard packages. If `Rscript` is unavailable, the test skips.

use std::{path::PathBuf, process::Command};

use rd_helpdb::{DemoEntry, PackageHelpDb, VignetteEntry};

const ORACLE_SCRIPT: &str = r#"
hex <- function(value) {
  bytes <- as.integer(charToRaw(enc2utf8(value)))
  paste(sprintf("%02x", bytes), collapse = "")
}
emit <- function(...) cat(paste(..., sep = "\t", collapse = "\t"), "\n", sep = "")

for (pkg in c("stats", "tools")) {
  pkg_dir <- tryCatch(find.package(pkg), error = function(e) NULL)
  emit("package", hex(pkg))
  if (is.null(pkg_dir)) {
    emit("skip", hex(paste("package not found:", pkg)))
    emit("end")
    next
  }
  emit("pkg_dir", hex(pkg_dir))

  vignette_path <- file.path(pkg_dir, "Meta", "vignette.rds")
  vignette_present <- file.exists(vignette_path)
  emit("vignette_present", as.integer(vignette_present))
  if (vignette_present) {
    index <- readRDS(vignette_path)
    emit("vignette_count", nrow(index))
    for (row in seq_len(nrow(index))) {
      emit(
        "vignette", row - 1L,
        hex(index$File[[row]]), hex(index$Title[[row]]),
        hex(index$PDF[[row]]), hex(index$R[[row]])
      )
      depends <- index$Depends[[row]]
      do.call(
        emit,
        c(list("depends", row - 1L, length(depends)), as.list(vapply(depends, hex, "")))
      )
      keywords <- index$Keywords[[row]]
      do.call(
        emit,
        c(list("keywords", row - 1L, length(keywords)), as.list(vapply(keywords, hex, "")))
      )
    }
  }

  demo_path <- file.path(pkg_dir, "Meta", "demo.rds")
  demo_present <- file.exists(demo_path)
  emit("demo_present", as.integer(demo_present))
  if (demo_present) {
    index <- readRDS(demo_path)
    emit("demo_count", nrow(index))
    for (row in seq_len(nrow(index))) {
      emit("demo", row - 1L, hex(index[row, 1L]), hex(index[row, 2L]))
    }
  }
  emit("end")
}
"#;

#[derive(Debug, Default)]
struct OraclePackage {
    package: String,
    skip: Option<String>,
    pkg_dir: Option<PathBuf>,
    vignettes: Option<Vec<VignetteEntry>>,
    demos: Option<Vec<DemoEntry>>,
}

fn decode_hex(value: &str) -> String {
    assert_eq!(value.len() % 2, 0, "odd-length hex string: {value:?}");
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("hex is ASCII");
            u8::from_str_radix(pair, 16)
                .unwrap_or_else(|error| panic!("invalid hex {pair}: {error}"))
        })
        .collect();
    String::from_utf8(bytes).expect("R oracle emitted UTF-8")
}

fn parse_count(fields: &[&str], line: &str) -> usize {
    fields[2]
        .parse()
        .unwrap_or_else(|error| panic!("invalid count in {line:?}: {error}"))
}

fn parse_oracle_output(stdout: &str) -> Vec<OraclePackage> {
    let mut packages = Vec::new();
    let mut current: Option<OraclePackage> = None;

    for line in stdout.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        match fields[0] {
            "package" => {
                assert!(
                    current.is_none(),
                    "package block did not end before {line:?}"
                );
                current = Some(OraclePackage {
                    package: decode_hex(fields[1]),
                    ..Default::default()
                });
            }
            "skip" => {
                current.as_mut().expect("skip within package").skip = Some(decode_hex(fields[1]))
            }
            "pkg_dir" => {
                current.as_mut().expect("pkg_dir within package").pkg_dir =
                    Some(PathBuf::from(decode_hex(fields[1])));
            }
            "vignette_present" => {
                current
                    .as_mut()
                    .expect("vignette_present within package")
                    .vignettes = (fields[1] == "1").then(Vec::new);
            }
            "vignette_count" => {
                let expected: usize = fields[1].parse().expect("vignette_count is numeric");
                current
                    .as_mut()
                    .expect("vignette_count within package")
                    .vignettes
                    .as_mut()
                    .expect("vignette file is present")
                    .reserve(expected);
            }
            "vignette" => {
                assert_eq!(fields.len(), 6, "malformed vignette line: {line:?}");
                let row: usize = fields[1].parse().expect("vignette row is numeric");
                let entries = current
                    .as_mut()
                    .expect("vignette within package")
                    .vignettes
                    .as_mut()
                    .expect("vignette file is present");
                assert_eq!(row, entries.len(), "vignette rows are ordered");
                entries.push(VignetteEntry {
                    file: decode_hex(fields[2]),
                    title: decode_hex(fields[3]),
                    pdf: decode_hex(fields[4]),
                    r: decode_hex(fields[5]),
                    depends: Vec::new(),
                    keywords: Vec::new(),
                });
            }
            "depends" | "keywords" => {
                let row: usize = fields[1].parse().expect("list-column row is numeric");
                let count = parse_count(&fields, line);
                assert_eq!(
                    fields.len(),
                    count + 3,
                    "malformed list-column line: {line:?}"
                );
                let values = fields[3..].iter().map(|value| decode_hex(value)).collect();
                let entry = &mut current
                    .as_mut()
                    .expect("list column within package")
                    .vignettes
                    .as_mut()
                    .expect("vignette file is present")[row];
                if fields[0] == "depends" {
                    entry.depends = values;
                } else {
                    entry.keywords = values;
                }
            }
            "demo_present" => {
                current.as_mut().expect("demo_present within package").demos =
                    (fields[1] == "1").then(Vec::new);
            }
            "demo_count" => {
                let expected: usize = fields[1].parse().expect("demo_count is numeric");
                current
                    .as_mut()
                    .expect("demo_count within package")
                    .demos
                    .as_mut()
                    .expect("demo file is present")
                    .reserve(expected);
            }
            "demo" => {
                assert_eq!(fields.len(), 4, "malformed demo line: {line:?}");
                let row: usize = fields[1].parse().expect("demo row is numeric");
                let entries = current
                    .as_mut()
                    .expect("demo within package")
                    .demos
                    .as_mut()
                    .expect("demo file is present");
                assert_eq!(row, entries.len(), "demo rows are ordered");
                entries.push(DemoEntry {
                    name: decode_hex(fields[2]),
                    title: decode_hex(fields[3]),
                });
            }
            "end" => packages.push(current.take().expect("end within package")),
            _ => {}
        }
    }

    assert!(current.is_none(), "unterminated oracle package block");
    packages
}

#[test]
fn oracle_matches_vignettes_and_demos() {
    let output = match Command::new("Rscript").args(["-e", ORACLE_SCRIPT]).output() {
        Ok(output) if output.status.success() => output,
        Ok(output) => panic!(
            "oracle R script failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("skipping: Rscript not found");
            return;
        }
        Err(error) => panic!("failed to run Rscript: {error}"),
    };

    let stdout = String::from_utf8(output.stdout).expect("oracle stdout is UTF-8");
    let packages = parse_oracle_output(&stdout);
    assert_eq!(packages.len(), 2, "unexpected oracle output:\n{stdout}");

    for oracle in packages {
        if let Some(reason) = oracle.skip {
            println!("skipping {}: {reason}", oracle.package);
            continue;
        }
        let pkg_dir = oracle.pkg_dir.expect("oracle package directory");
        let db = PackageHelpDb::open(&pkg_dir).unwrap_or_else(|error| {
            panic!("open {} at {}: {error}", oracle.package, pkg_dir.display())
        });

        let actual_vignettes = db.vignettes().expect("vignettes()");
        assert_eq!(
            actual_vignettes
                .as_ref()
                .map(|index| index.entries().cloned().collect::<Vec<_>>()),
            oracle.vignettes,
            "{} vignette index differs from readRDS()",
            oracle.package
        );

        let actual_demos = db.demos().expect("demos()");
        assert_eq!(
            actual_demos
                .as_ref()
                .map(|index| index.entries().cloned().collect::<Vec<_>>()),
            oracle.demos,
            "{} demo index differs from readRDS()",
            oracle.package
        );

        match oracle.package.as_str() {
            "stats" => {
                assert!(
                    actual_vignettes.is_some(),
                    "stats vignette.rds should exist"
                );
                assert!(actual_demos.is_some(), "stats demo.rds should exist");
            }
            "tools" => {
                assert!(
                    actual_vignettes.is_none(),
                    "tools vignette.rds should be absent"
                );
                assert!(actual_demos.is_none(), "tools demo.rds should be absent");
            }
            _ => unreachable!("oracle requested only stats and tools"),
        }
    }
}
