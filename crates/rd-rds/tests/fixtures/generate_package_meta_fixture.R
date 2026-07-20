# Generate a deterministic synthetic Meta/package.rds fixture.
# Provenance: R 4.6.1; generated from hand-constructed fictional data without
# network access or an installed package source tree.

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

stopifnot(getRversion() == "4.6.1")
dir.create("data/package-meta", recursive = TRUE, showWarnings = FALSE)

description <- c(
    "fixturepkg", "0.1.0", "tools", "A tiny synthetic package fixture",
    "Jane Fixture <jane@example.com>",
    "Jane Fixture <jane@example.com>", "jane@example.com",
    "Provides deterministic metadata for parser tests.", "MIT",
    "R (>= 4.6.0)", NA_character_, "no", "UTF-8", NA_character_,
    "2026-06-24 19:14:59 UTC"
)
names(description) <- c(
    "Package", "Version", "Priority", "Title", "Author", "Maintainer",
    "Contact", "Description", "License", "Imports", "Suggests",
    "NeedsCompilation", "Encoding", "Enhances", "Built"
)

built <- list(
    R = structure(
        list(c(4L, 6L, 1L)),
        class = c("R_system_version", "package_version", "numeric_version")
    ),
    Platform = "x86_64-pc-linux-gnu",
    Date = "2026-06-24 19:14:59 UTC",
    OStype = "unix"
)

metadata <- list(
    DESCRIPTION = description,
    Built = built,
    Rdepends = NULL,
    Rdepends2 = NULL,
    Depends = list(),
    Suggests = list(fixturehelper = list(name = "fixturehelper")),
    Imports = list(fixturetools = list(name = "fixturetools")),
    LinkingTo = list()
)
names(metadata) <- c(
    "DESCRIPTION", "Built", "Rdepends", "Rdepends2", "Depends", "Suggests",
    "Imports", "LinkingTo"
)
class(metadata) <- "packageDescription2"

saveRDS(metadata, "data/package-meta/fixturepkg-package.rds", compress = "gzip", version = 3)
