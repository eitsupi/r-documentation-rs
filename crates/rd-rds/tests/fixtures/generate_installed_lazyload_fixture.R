# Generate an installed-package lazy-load fixture from the minimal package.
# The checked-in fixture was generated with R 4.6.1; run with
# Rscript generate_installed_lazyload_fixture.R OUTPUT_DIR.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: Rscript generate_installed_lazyload_fixture.R OUTPUT_DIR")
out <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(out, recursive = TRUE, showWarnings = FALSE)
script_arg <- grep("^--file=", commandArgs(), value = TRUE)
if (length(script_arg) != 1L) stop("cannot determine generator path")
script_path <- sub("^--file=", "", script_arg)
source_dir <- normalizePath(file.path(dirname(script_path), "lazyload-package"), mustWork = TRUE)
# Keep the install path independent of OUTPUT_DIR. R serializes this path in
# namespace/environment records, so a stable path keeps equivalent outputs
# byte-identical across separate output directories.
library_dir <- file.path(dirname(tempdir()), "rd-rds-lazyload-fixture-library")
if (dir.exists(library_dir)) unlink(library_dir, recursive = TRUE, force = TRUE)
tryCatch({
  dir.create(library_dir, recursive = TRUE, showWarnings = FALSE)
  status <- system2(file.path(R.home("bin"), "R"),
                    c("CMD", "INSTALL", "--no-multiarch", "--no-test-load", "--no-byte-compile",
                      "-l", shQuote(library_dir), shQuote(source_dir)))
  if (!identical(status, 0L)) stop("R CMD INSTALL failed")
  installed_dir <- file.path(library_dir, "lazyfixture")
  copied_rdx <- file.copy(file.path(installed_dir, "R", "lazyfixture.rdx"),
                          file.path(out, "lazyfixture.rdx"), overwrite = TRUE)
  copied_rdb <- file.copy(file.path(installed_dir, "R", "lazyfixture.rdb"),
                          file.path(out, "lazyfixture.rdb"), overwrite = TRUE)
  if (!isTRUE(copied_rdx) || !isTRUE(copied_rdb)) {
    stop("failed to copy installed lazy-load database files")
  }
}, finally = unlink(library_dir, recursive = TRUE, force = TRUE))
