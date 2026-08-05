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

# PACKAGES.rds main-index variation: a four-row, 17-column version-2 matrix
# wrapped in xz. It deliberately contains R NA, literal "NA", and a newline
# in a Suggests cell.
# Its row names mirror the Package column, as real CRAN main indexes do.
cran_values <- matrix(
    c(
        "alpha", "beta", "gamma", "delta",
        "1.0.0", "2.0.0", "0.9.0", "1.10.0",
        "recommended", "optional", NA_character_, "NA",
        "R (>= 4.0)", NA_character_, "R (>= 3.6)", "R (>= 4.2)",
        "utils", "curl", NA_character_, "NA",
        NA_character_, "Rcpp", NA_character_, NA_character_,
        "testthat", "knitr,\n  rmarkdown", NA_character_, "NA",
        NA_character_, NA_character_, "alphaTools", NA_character_,
        "MIT", "GPL-3", "Apache License (>= 2)", "BSD-3-Clause",
        "TRUE", "TRUE", "TRUE", "FALSE",
        "FALSE", "FALSE", "FALSE", "TRUE",
        "unix", "unix", NA_character_, "windows",
        NA_character_, "x86_64", "x86_64", NA_character_,
        "11111111111111111111111111111111", "22222222222222222222222222222222",
        "33333333333333333333333333333333", "44444444444444444444444444444444",
        "yes", "no", "yes", NA_character_,
        "alpha_1.0.0.tar.gz", "beta_2.0.0.tar.gz", NA_character_, "delta_1.10.0.tar.gz",
        "2020-01-02", "2020-02-03", "2020-03-04", NA_character_
    ),
    nrow = 4,
    dimnames = list(
        c("alpha", "beta", "gamma", "delta"),
        c(
            "Package", "Version", "Priority", "Depends", "Imports", "LinkingTo",
            "Suggests", "Enhances", "License", "License_is_FOSS",
            "License_restricts_use", "OS_type", "Archs", "MD5sum",
            "NeedsCompilation", "Path", "Published"
        )
    )
)

archive_values <- matrix(
    c(
        "samplepkg", "samplepkg", "samplepkg",
        "1.10.0", "1.2.0", "1.2.1",
        "optional", "optional", "optional",
        "R (>= 4.0)", "R (>= 4.0)", "R (>= 4.0)",
        "utils", "utils", "utils",
        NA_character_, NA_character_, NA_character_,
        enc2utf8("José"), "NA", NA_character_,
        NA_character_, "helper,\n  another", NA_character_,
        "MIT", "MIT", "MIT",
        "TRUE", "TRUE", "TRUE",
        "FALSE", "FALSE", "FALSE",
        "unix", "unix", "unix",
        "x86_64", "x86_64", "x86_64",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "cccccccccccccccccccccccccccccccc",
        "no", "no", "no"
    ),
    nrow = 3,
    dimnames = list(
        # Real CRAN archive indexes carry dimnames = list(NULL, ...); verified 2026-08-05.
        NULL,
        c(
            "Package", "Version", "Priority", "Depends", "Imports", "LinkingTo",
            "Suggests", "Enhances", "License", "License_is_FOSS",
            "License_restricts_use", "OS_type", "Archs", "MD5sum",
            "NeedsCompilation"
        )
    )
)

saveRDS(
    cran_values,
    "data/packages/packages-cran-v2-xz.rds",
    compress = "xz",
    version = 2
)

# PACKAGES.rds per-package-archive variation: a three-row, 15-column version-3
# matrix in non-semver row order, with one explicitly UTF-8-encoded cell.
saveRDS(
    archive_values,
    "data/packages/packages-archive-v3-gzip.rds",
    compress = "gzip",
    version = 3
)
