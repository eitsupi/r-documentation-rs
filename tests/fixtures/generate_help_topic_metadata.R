# Generate independently authored Meta/Rd.rds fixtures with R 4.6.1.
# Run from the repository root. Only rd-helpdb consumes these fixtures.
stopifnot(getRversion() == "4.6.1")
out <- "crates/rd-helpdb/tests/fixtures/data"

metadata <- structure(list(
  Title = c("First topic title", NA_character_, "Title without a page", ""),
  Name = c("first", NA_character_, "unlisted-topic", ""),
  Aliases = list(c("shared", "first", NA_character_, "first"),
                 c("shared", "second"), "title-only", character()),
  File = c("first-topic.Rd", "second-topic.Rd", NA_character_, "nested.Rd.Rd"),
  Extra = rep("ignored", 4L)
), class = "data.frame", row.names = c(NA_integer_, -4L))
empty <- metadata[FALSE, , drop = FALSE]
aliases_only <- metadata["Aliases"]

for (version in c(2L, 3L)) {
  for (name in c("metadata", "empty", "aliases_only")) {
    filename <- sprintf("help_topics_%s_v%d.rds", name, version)
    path <- file.path(out, filename)
    saveRDS(get(name), path, version = version)
    stopifnot(identical(readRDS(path), get(name)))
  }
}

# Use an existing, independently authored Rd source without system macros.
topic <- tools::parse_Rd("tests/fixtures/rd-src/minimal.Rd", macros = FALSE)
# Source references embed filesystem paths and timestamps. They are irrelevant
# to help fallback and would make this fixture depend on the build machine.
strip_source <- function(x) {
  attr(x, "srcref") <- NULL
  if (is.list(x)) {
    for (i in seq_along(x)) x[[i]] <- strip_source(x[[i]])
  }
  x
}
topic <- strip_source(topic)
payload <- serialize(topic, NULL, version = 3L)
index <- list(
  variables = list("first-topic" = c(0L, length(payload))),
  references = list(),
  compressed = FALSE
)
saveRDS(index, file.path(out, "help_topics.rdx"), version = 3L)
writeBin(payload, file.path(out, "help_topics.rdb"))
