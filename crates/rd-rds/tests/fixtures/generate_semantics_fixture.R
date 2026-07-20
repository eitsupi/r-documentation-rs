# Generate the versioned RDS fixtures used by rd-ast's semantic-view tests.
# Run this script from this directory or from the repository root.
# Provenance: the committed fixtures were generated with R 4.6.1 (not enforced by this script).

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
# Work from the script's directory with relative paths so the srcref
# recorded inside the RDS output stays machine-independent.
setwd(dirname(normalizePath(script_path)))

dir.create("data", showWarnings = FALSE)
rd <- tools::parse_Rd("rd-src/semantics.Rd", macros = FALSE)
saveRDS(rd, "data/rd_semantics_v2.rds", version = 2)
saveRDS(rd, "data/rd_semantics_v3.rds", version = 3)

forbidden_fixture_text <- c(
    "Rd/macros/system.Rd",
    "newcommand",
    "Rd_expr_doi",
    "CRAN.R-project.org/package=",
    "ifelse{latex}"
)
written_files <- file.path("data", c("rd_semantics_v2.rds", "rd_semantics_v3.rds"))
for (path in written_files) {
  bytes <- readBin(path, "raw", n = file.info(path)$size)
  bytes <- memDecompress(bytes, type = "gzip")
  hits <- forbidden_fixture_text[vapply(
    forbidden_fixture_text,
    function(pattern) length(grepRaw(pattern, bytes, fixed = TRUE)) > 0,
    logical(1)
  )]
  if (length(hits)) stop("forbidden text in ", path, ": ", paste(hits, collapse = ", "))
  message("forbidden-text scan OK: ", path)
}
