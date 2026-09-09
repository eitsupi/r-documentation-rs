# Generate deterministic Meta/nsInfo.rds fixtures for NamespaceMetadata.
# The script intentionally builds the metadata shape directly instead of
# reading an installed package or copying a package's binary artifact.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: Rscript generate_namespace_metadata_fixture.R OUTPUT_DIR")
out <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(out, recursive = TRUE, showWarnings = FALSE)

imports <- list(
  "base",
  list("stats", structure(c("mean", "median"), names = c("mean", "average"))),
  list(package = "utils", except = c("head", "tail"))
)

s3methods <- matrix(
  c("print", "widget", NA_character_, "base",
    "format", "widget", "format.widget", "utils"),
  nrow = 2L,
  ncol = 4L,
  byrow = TRUE
)

metadata <- list(
  exports = c("alpha", "alpha", "beta"),
  exportPatterns = character(),
  imports = imports,
  S3methods = s3methods,
  exportClasses = c("Widget", "Widget"),
  exportMethods = c("show", "show"),
  exportClassPatterns = c("^Widget"),
  unknownField = list("ignored")
)

saveRDS(metadata, file.path(out, "namespace-v2.rds"), compress = FALSE, version = 2L)
saveRDS(metadata, file.path(out, "namespace-v3.rds"), compress = FALSE, version = 3L)
