# Generate deterministic standalone RDS envelope fixtures.
# Provenance: R 4.6.1; zstd 1.5.5 (extSoftVersion()[["zstd"]]); run with
# `Rscript generate_envelope_fixture.R`.

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

stopifnot(getRversion() == "4.6.1")
zstd_version <- extSoftVersion()[["zstd"]]
stopifnot(!is.null(zstd_version), nzchar(zstd_version))
dir.create("data/envelope", recursive = TRUE, showWarnings = FALSE)

values <- matrix(
    c(
        "base", "stats", NA_character_,
        "4.6.1", "4.6.1", "4.6.1",
        "R (>= 4.0)", "R (>= 4.0)", "R (>= 4.0)",
        "utils", "methods", "NA",
        "https://example.invalid/base", "https://example.invalid/stats", "https://example.invalid/tools",
        "1000", "2000", "3000"
    ),
    nrow = 3,
    dimnames = list(
        c("base", "stats", "tools"),
        c("Package", "Version", "Depends", "Imports", "DownloadURL", "Filesize")
    )
)

saveRDS(values, "data/envelope/packages-small-uncompressed.rds", compress = FALSE, version = 3)
saveRDS(values, "data/envelope/packages-small-gzip.rds", compress = "gzip", version = 3)
saveRDS(values, "data/envelope/packages-small-xz.rds", compress = "xz", version = 3)
saveRDS(values, "data/envelope/packages-small-bzip2.rds", compress = "bzip2", version = 3)
saveRDS(values, "data/envelope/packages-small-zstd.rds", compress = "zstd", version = 3)
