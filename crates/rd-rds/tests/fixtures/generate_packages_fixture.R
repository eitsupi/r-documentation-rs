# Generate deterministic CRAN-like PACKAGES.rds fixtures.
# Provenance: R 4.6.1; generated from a hand-constructed matrix without
# network access or tools::write_PACKAGES input directories.

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

stopifnot(getRversion() == "4.6.1")
dir.create("data/packages", recursive = TRUE, showWarnings = FALSE)

values <- matrix(
    c(
        "cli", "curl", "data.table", "examplepkg",
        "4.6.1", "4.3.2", "1.17.8", "0.1.0",
        "MIT + file LICENSE", "MIT", "MPL-2.0", "GPL-3",
        "TRUE", "FALSE", "FALSE", NA_character_,
        "R (>= 4.1)", "R (>= 4.1)", "R (>= 3.1.0)", NA_character_,
        "Rcpp", "Rcpp", NA_character_, "NA",
        NA_character_, NA_character_, NA_character_, NA_character_,
        NA_character_, NA_character_, NA_character_, NA_character_,
        NA_character_, NA_character_, NA_character_, NA_character_,
        "https://cran.r-project.org", "https://cran.r-project.org", NA_character_, NA_character_,
        "100000", "200000", "300000", "400000"
    ),
    nrow = 4,
    dimnames = list(
        c("cli", "curl", "data.table", "examplepkg"),
        c("Package", "Version", "License", "NeedsCompilation", "Depends", "Imports",
          "Suggests", "MD5sum", "Path", "DownloadURL", "Filesize")
    )
)

saveRDS(values, "data/packages/packages-cran-gzip.rds", compress = "gzip", version = 3)
saveRDS(values, "data/packages/packages-cran-xz.rds", compress = "xz", version = 3)
