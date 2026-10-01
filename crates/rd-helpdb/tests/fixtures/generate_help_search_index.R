# Generate independently authored Meta/hsearch.rds fixtures with R 4.6.1.
# Run from the repository root. Only rd-helpdb consumes these fixtures.
stopifnot(getRversion() == "4.6.1")
out <- "crates/rd-helpdb/tests/fixtures/data"

base <- rbind(
  c("fixturepkg", "", "1", "first", "First title", "first", "UTF-8"),
  c("fixturepkg", "", "2", "second", "", NA_character_, "UTF-8"),
  c(NA_character_, "", "2", "second", "Second title", "second", NA_character_),
  c("fixturepkg", "", "", "", "", "", "")
)
colnames(base) <- c("Package", "LibPath", "ID", "Name", "Title", "Topic", "Encoding")
aliases <- rbind(
  c("first", "1", "fixturepkg"),
  c("first", "1", "fixturepkg"),
  c(NA_character_, "2", "fixturepkg"),
  c("", NA_character_, "")
)
colnames(aliases) <- c("Alias", "ID", "Package")
keywords <- rbind(
  c("keyword", "1", "fixturepkg"),
  c("keyword", "1", "fixturepkg"),
  c(NA_character_, "2", "fixturepkg"),
  c("", NA_character_, "")
)
colnames(keywords) <- c("Keyword", "ID", "Package")
concepts <- rbind(
  c("concept", "1", "fixturepkg"),
  c(NA_character_, "2", "fixturepkg"),
  c("", "", "")
)
colnames(concepts) <- c("Concept", "ID", "Package")
valid <- list(base, aliases, keywords, concepts)
reordered <- lapply(valid, function(matrix) {
  matrix[, rev(seq_len(ncol(matrix))), drop = FALSE]
})
latin1_title <- rawToChar(as.raw(c(0x63, 0x61, 0x66, 0xe9)))
Encoding(latin1_title) <- "latin1"
latin1 <- valid
latin1[[1]][1, "Title"] <- latin1_title

empty <- lapply(list(
  c("Package", "LibPath", "ID", "Name", "Title", "Topic", "Encoding"),
  c("Alias", "ID", "Package"),
  c("Keyword", "ID", "Package"),
  c("Concept", "ID", "Package")
), function(columns) {
  matrix(character(), nrow = 0L, ncol = length(columns),
         dimnames = list(NULL, columns))
})

legacy_base <- base[, c("Package", "LibPath", "ID", "Name", "Title", "Topic")]
colnames(legacy_base) <- c("Package", "LibPath", "ID", "name", "title", "topic")
legacy_base_with_encoding <- base
colnames(legacy_base_with_encoding) <- c(
  "Package", "LibPath", "ID", "name", "title", "topic", "Encoding"
)
legacy <- list(
  legacy_base,
  `colnames<-`(aliases, c("Aliases", "ID", "Package")),
  `colnames<-`(keywords, c("Keywords", "ID", "Package")),
  `colnames<-`(concepts, c("Concepts", "ID", "Package"))
)
legacy_with_encoding <- list(
  legacy_base_with_encoding,
  `colnames<-`(aliases, c("Aliases", "ID", "Package")),
  `colnames<-`(keywords, c("Keywords", "ID", "Package")),
  `colnames<-`(concepts, c("Concepts", "ID", "Package"))
)

for (version in c(2L, 3L)) {
  saveRDS(valid, file.path(out, sprintf("help_search_valid_v%d.rds", version)), version = version)
}
saveRDS(reordered, file.path(out, "help_search_reordered_v3.rds"), version = 3L)
saveRDS(latin1, file.path(out, "help_search_latin1_v3.rds"), version = 3L)
saveRDS(empty, file.path(out, "help_search_empty_v3.rds"), version = 3L)
saveRDS(legacy, file.path(out, "help_search_legacy_v3.rds"), version = 3L)
saveRDS(legacy_with_encoding,
        file.path(out, "help_search_legacy_encoding_v3.rds"), version = 3L)

# Build a small package from source so this fixture exercises the exact
# R CMD INSTALL metadata path, including both ASCII and UTF-8 Rd input.
package_root <- file.path(tempdir(), "rd-helpdb-help-search-fixture")
unlink(package_root, recursive = TRUE, force = TRUE)
dir.create(file.path(package_root, "R"), recursive = TRUE)
dir.create(file.path(package_root, "man"), recursive = TRUE)
writeLines(c(
  "Package: hsearchfixture",
  "Version: 1.0.0",
  "Title: Help Search Fixture",
  "Description: A small independently generated help-search fixture.",
  "License: MIT",
  "Encoding: UTF-8"
), file.path(package_root, "DESCRIPTION"))
writeLines("", file.path(package_root, "NAMESPACE"))
writeLines(c(
  "\\name{ascii-topic}",
  "\\alias{ascii-topic}",
  "\\title{An ASCII topic}",
  "\\description{A fixture topic with a duplicate keyword.}",
  "\\keyword{datasets}",
  "\\concept{fixture}",
  "\\concept{fixture}",
  "\\docType{data}"
), file.path(package_root, "man/ascii-topic.Rd"))
writeLines(c(
  "\\name{utf8-topic}",
  "\\alias{utf8-topic}",
  "\\title{UTF-8 café topic}",
  "\\description{A topic containing café and 日本語.}",
  "\\keyword{Unicode}",
  "\\concept{café}"
), file.path(package_root, "man/utf8-topic.Rd"), useBytes = TRUE)
library_root <- file.path(tempdir(), "rd-helpdb-help-search-library")
unlink(library_root, recursive = TRUE, force = TRUE)
dir.create(library_root, recursive = TRUE)
install_output <- system2(
  file.path(R.home("bin"), "R"),
  c("CMD", "INSTALL", "--no-staged-install", "--no-byte-compile",
    "--no-test-load", "-l", shQuote(library_root), shQuote(package_root)),
  stdout = TRUE, stderr = TRUE
)
if (!is.null(attr(install_output, "status")) && attr(install_output, "status") != 0L) {
  stop(paste(install_output, collapse = "\n"))
}
installed <- file.path(library_root, "hsearchfixture")
installed_index <- readRDS(file.path(installed, "Meta/hsearch.rds"))
installed_rd <- readRDS(file.path(installed, "Meta/Rd.rds"))
description <- read.dcf(file.path(installed, "DESCRIPTION"))
rebuilt <- tools:::.build_hsearch_index(
  installed_rd, "hsearchfixture", description[1, "Encoding"]
)
stopifnot(identical(installed_index, rebuilt))
saveRDS(installed_index,
        file.path(out, "help_search_installed_package_v3.rds"), version = 3L)
unlink(c(package_root, library_root), recursive = TRUE, force = TRUE)

stopifnot(identical(
  readRDS(file.path(out, "help_search_valid_v2.rds")), valid
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_valid_v3.rds")), valid
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_reordered_v3.rds")), reordered
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_latin1_v3.rds")), latin1
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_empty_v3.rds")), empty
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_legacy_v3.rds")), legacy
))
stopifnot(identical(
  readRDS(file.path(out, "help_search_legacy_encoding_v3.rds")),
  legacy_with_encoding
))
