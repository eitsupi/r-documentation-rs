# Generate the versioned RDS fixtures used by rd-ast escape-semantics tests.
# Run this script from this directory or from the repository root.
# Provenance: the committed fixtures were generated with R 4.6.1 (not enforced by this script).

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

dir.create("data", showWarnings = FALSE)
# Do not embed R's GPL-licensed share/Rd/macros/system.Rd text in fixtures.
rd <- tools::parse_Rd("rd-src/escapes.Rd", macros = FALSE)
saveRDS(rd, "data/rd_escapes_v2.rds", version = 2)
saveRDS(rd, "data/rd_escapes_v3.rds", version = 3)

forbidden_fixture_text <- c(
  "Rd/macros/system.Rd",
  "newcommand",
  "Rd_expr_doi",
  "CRAN.R-project.org/package="
)
written_files <- file.path("data", c("rd_escapes_v2.rds", "rd_escapes_v3.rds"))
for (path in written_files) {
  bytes <- readBin(path, "raw", n = file.info(path)$size)
  bytes <- memDecompress(bytes, type = "gzip")
  hits <- forbidden_fixture_text[vapply(
    forbidden_fixture_text,
    function(pattern) length(grepRaw(pattern, bytes, fixed = TRUE)) > 0,
    logical(1)
  )]
  if (length(hits)) {
    stop("forbidden text in ", path, ": ", paste(hits, collapse = ", "))
  }
  message("forbidden-text scan OK: ", path)
}
