# Generate Meta/nsInfo.rds from a minimal package installed with R.
# The checked-in artifact was generated with R 4.6.1; run with
# Rscript generate_installed_namespace_fixture.R OUTPUT_DIR.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: Rscript generate_installed_namespace_fixture.R OUTPUT_DIR")
out <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(out, recursive = TRUE, showWarnings = FALSE)
script_arg <- grep("^--file=", commandArgs(), value = TRUE)
if (length(script_arg) != 1L) stop("cannot determine generator path")
script_path <- sub("^--file=", "", script_arg)
source_dir <- normalizePath(file.path(dirname(script_path), "namespace-package"), mustWork = TRUE)

# R records source paths in some installed metadata. Keep the temporary
# library outside OUTPUT_DIR and remove it after copying the stable artifact.
library_dir <- file.path(dirname(tempdir()), "rd-rds-namespace-fixture-library")
if (dir.exists(library_dir)) unlink(library_dir, recursive = TRUE, force = TRUE)
tryCatch({
  dir.create(library_dir, recursive = TRUE, showWarnings = FALSE)
  status <- system2(file.path(R.home("bin"), "R"),
                    c("CMD", "INSTALL", "--no-multiarch", "--no-test-load",
                      "--no-byte-compile", "-l", shQuote(library_dir),
                      shQuote(source_dir)))
  if (!identical(status, 0L)) stop("R CMD INSTALL failed")
  installed_dir <- file.path(library_dir, "namespacefixture")
  metadata <- file.path(installed_dir, "Meta", "nsInfo.rds")
  if (!file.copy(metadata, file.path(out, "namespace-installed.rds"), overwrite = TRUE)) {
    stop("failed to copy installed Meta/nsInfo.rds")
  }
}, finally = unlink(library_dir, recursive = TRUE, force = TRUE))
