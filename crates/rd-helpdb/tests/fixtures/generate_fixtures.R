#!/usr/bin/env Rscript

# Regenerate the typed vignette/demo index fixtures:
#
#   Rscript crates/rd-helpdb/tests/fixtures/generate_fixtures.R

args <- commandArgs(trailingOnly = FALSE)
file_arg <- grep("^--file=", args, value = TRUE)
if (length(file_arg) != 1L) {
  stop("run this file with Rscript")
}
script_path <- normalizePath(sub("^--file=", "", file_arg), mustWork = TRUE)
data_dir <- file.path(dirname(script_path), "data")
dir.create(data_dir, recursive = TRUE, showWarnings = FALSE)

save_fixture <- function(value, name) {
  saveRDS(value, file.path(data_dir, name), version = 3L, compress = "gzip")
}

vignette_reordered <- structure(
  list(
    Keywords = list("models", ""),
    Title = c("First vignette", "Second vignette"),
    Extra = c("ignored-one", "ignored-two"),
    Depends = list(c("tools", "stats"), character()),
    File = c("first.Rnw", "second.Rmd"),
    R = c("first.R", "second.R"),
    PDF = c("first.pdf", "second.html")
  ),
  class = "data.frame",
  row.names = c(NA_integer_, -2L)
)
save_fixture(vignette_reordered, "vignette_reordered_v3.rds")

vignette_empty <- structure(
  list(
    File = character(),
    Title = character(),
    PDF = character(),
    R = character(),
    Depends = list(),
    Keywords = list()
  ),
  class = "data.frame",
  row.names = integer()
)
save_fixture(vignette_empty, "vignette_empty_v3.rds")

vignette_missing_column <- vignette_reordered
vignette_missing_column$Keywords <- NULL
save_fixture(vignette_missing_column, "vignette_missing_column_v3.rds")

demo_valid <- matrix(
  c("first", "second", "First demo", ""),
  nrow = 2L,
  ncol = 2L
)
dimnames(demo_valid) <- NULL
save_fixture(demo_valid, "demo_valid_v3.rds")

demo_empty <- matrix(character(), nrow = 0L, ncol = 2L)
dimnames(demo_empty) <- NULL
save_fixture(demo_empty, "demo_empty_v3.rds")

demo_three_columns <- matrix(
  c("first", "First demo", "ignored"),
  nrow = 1L,
  ncol = 3L
)
dimnames(demo_three_columns) <- NULL
save_fixture(demo_three_columns, "demo_three_columns_v3.rds")
