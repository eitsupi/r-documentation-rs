# Generate small, independently authored lazy-load database fixtures.
# The data payload is an R serialization stream; R's "gzip" memCompress
# profile is the zlib stream used by makeLazyLoadDB's record layer.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: Rscript generate_lazyload_fixture.R OUTPUT_DIR")
out <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(out, recursive = TRUE, showWarnings = FALSE)

be32 <- function(value) {
  stopifnot(value >= 0, value <= 2^32 - 1)
  as.raw(c(
    bitwAnd(bitwShiftR(value, 24L), 255L),
    bitwAnd(bitwShiftR(value, 16L), 255L),
    bitwAnd(bitwShiftR(value, 8L), 255L),
    bitwAnd(value, 255L)
  ))
}

make_database <- function(version, suffix = paste0("-v", version)) {
  payload <- serialize(list(marker = "lazyload-fixture", value = 42L), NULL,
                       version = version)
  raw_rdb <- file.path(out, paste0("raw", suffix, ".rdb"))
  writeBin(payload, raw_rdb)
  raw_rdx <- list(
    variables = structure(list(first = c(0L, length(payload))), names = "first"),
    references = list(),
    compressed = FALSE
  )
  saveRDS(raw_rdx, file.path(out, paste0("raw", suffix, ".rdx")),
          compress = FALSE, version = version)

  second_payload <- serialize(list(marker = "lazyload-fixture", value = 43L), NULL,
                              version = version)
  first_record <- c(be32(length(payload)), memCompress(payload, type = "gzip"))
  second_record <- c(be32(length(second_payload)), memCompress(second_payload, type = "gzip"))
  zlib_record <- c(first_record, second_record)
  zlib_rdb <- file.path(out, paste0("zlib", suffix, ".rdb"))
  writeBin(zlib_record, zlib_rdb)
  zlib_rdx <- list(
    variables = structure(
      list(duplicate = c(0L, length(first_record)),
           other = c(0L, length(first_record)),
           duplicate = c(length(first_record), length(second_record))),
      names = c("duplicate", "other", "duplicate")
    ),
    references = list(),
    compressed = TRUE
  )
  saveRDS(zlib_rdx, file.path(out, paste0("zlib", suffix, ".rdx")),
          compress = FALSE, version = version)
  list(payload = payload, first_record = first_record, zlib_record = zlib_record)
}

v2 <- make_database(2L)
v3 <- make_database(3L, "")
payload <- v3$payload
first_record <- v3$first_record
zlib_record <- v3$zlib_record

compound_rdx <- list(
  variables = structure(list(first = c(0L, length(first_record))), names = "first"),
  references = structure(
    list(env = list(
      eagerKey = c(0L, length(first_record)),
      lazyKeys = structure(list(line = c(0L, length(first_record))), names = "line")
    )),
    names = "env"
  ),
  compressed = TRUE
)
saveRDS(compound_rdx, file.path(out, "compound.rdx"), compress = FALSE, version = 3L)

for (code in c(2L, 3L)) {
  unsupported_rdx <- list(
    variables = structure(list(first = c(0L, length(zlib_record))), names = "first"),
    references = list(),
    compressed = code
  )
  saveRDS(unsupported_rdx, file.path(out, paste0("unsupported-", code, ".rdx")),
          compress = FALSE, version = 3L)
}

bad_range <- list(
  variables = structure(list(first = c(1000L, 2L)), names = "first"),
  references = list(),
  compressed = FALSE
)
saveRDS(bad_range, file.path(out, "bad-range.rdx"), compress = FALSE, version = 3L)

bad_overflow <- list(
  variables = structure(list(first = c(2^53, 1)), names = "first"),
  references = list(),
  compressed = FALSE
)
saveRDS(bad_overflow, file.path(out, "bad-overflow.rdx"), compress = FALSE, version = 3L)
