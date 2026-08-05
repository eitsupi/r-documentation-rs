# Generate a deterministic synthetic Meta/archive.rds fixture.
# Provenance: R 4.6.1; generated from hand-constructed fictional data without
# network access, real files, or environment-dependent metadata.

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

stopifnot(getRversion() == "4.6.1")
dir.create("data/archive_metadata", recursive = TRUE, showWarnings = FALSE)

fixed_time <- as.POSIXct("2020-01-02 03:04:05", tz = "UTC")
later_time <- as.POSIXct("2020-02-03 04:05:06", tz = "UTC")

archive_info <- function(paths, sizes, directories, modes, timestamps, uids, gids,
                         users, groups) {
    data.frame(
        size = as.numeric(sizes),
        isdir = directories,
        mode = structure(as.integer(modes), class = "octmode"),
        mtime = as.POSIXct(timestamps, origin = "1970-01-01", tz = "UTC"),
        ctime = as.POSIXct(timestamps, origin = "1970-01-01", tz = "UTC"),
        atime = as.POSIXct(timestamps, origin = "1970-01-01", tz = "UTC"),
        uid = as.integer(uids),
        gid = as.integer(gids),
        uname = users,
        grname = groups,
        row.names = paths,
        check.names = FALSE,
        stringsAsFactors = FALSE
    )
}

metadata <- list(
    alpha = archive_info(
        c("alpha/alpha_0.1.0.tar.gz", "alpha/alpha_0.2.0.tar.gz"),
        c(1234, 2345),
        c(FALSE, FALSE),
        c(420L, 420L),
        c(as.numeric(fixed_time), as.numeric(later_time)),
        c(100L, 100L),
        c(200L, 200L),
        c("fixture-user", "fixture-user"),
        c("fixture-group", "fixture-group")
    ),
    beta = archive_info(
        "beta/beta_1.0.0.tar.gz",
        3456,
        FALSE,
        420L,
        as.numeric(fixed_time),
        101L,
        201L,
        "fixture-user",
        "fixture-group"
    )
)

saveRDS(
    metadata,
    "data/archive_metadata/archive-metadata-v3-gzip.rds",
    compress = "gzip",
    version = 3
)
