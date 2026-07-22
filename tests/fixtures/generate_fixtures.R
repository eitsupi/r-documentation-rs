#!/usr/bin/env Rscript
#
# Generates the synthetic binary fixtures used by rd-rds's oracle/regression
# tests, and the rd-helpdb Meta/vignette.rds- and Meta/demo.rds-shaped
# fixtures copied into crates/rd-helpdb/tests/fixtures/data/ (see the
# workspace README's "Layout and tests" section for the copy contract: this
# script is the canonical generator, and crate-local copies are synced by
# copying, never regenerated in place). Run from the repository root:
#
#   Rscript tests/fixtures/generate_fixtures.R
#
# Real installed-package help DB entries (e.g. base/utils/tools) are NOT
# produced by this script -- those are read directly from the R library at
# test time as an oracle comparison. This script only produces the small,
# purpose-built synthetic fixtures that isolate individual format features,
# mirroring the framing real .rdb entries and saveRDS() output actually use.
#
# Fixtures that carry srcref/srcfile metadata are not byte-stable across
# machines or regeneration times because parsed srcfile environments embed
# working-directory and timestamp state. If those fixtures are regenerated,
# any resulting byte diffs should be treated as environment drift rather than
# an automatic sign of a format change.
#
# Every fixture is written for both serialize() format version 2 and
# version 3, since both occur in real R installations depending on R
# version / options(save.defaults). Most fixtures are written as ordinary
# gzip-compressed .rds files (saveRDS()'s default), which is the format a
# reader would see for a standalone .rds file such as Meta/*.rds. A couple
# of representative fixtures are additionally written as raw "fake .rdb
# entries": a 4-byte big-endian uncompressed-size prefix followed by a raw
# zlib deflate stream (no gzip wrapper) over a bare serialize() payload,
# mirroring the concatenated-entry format of a real <pkg>.rdb file.
#
# Provenance: the committed fixtures were generated with R 4.6.1 (not enforced by this script).

fixtures_dir <- "tests/fixtures"
src_dir <- file.path(fixtures_dir, "rd-src")
data_dir <- file.path(fixtures_dir, "data")

if (!dir.exists(src_dir)) {
  stop("expected Rd sources in ", src_dir,
       " -- run this script from the repository root")
}
dir.create(data_dir, recursive = TRUE, showWarnings = FALSE)

save_versions <- function(obj, name) {
  for (v in c(2, 3)) {
    path <- file.path(data_dir, sprintf("%s_v%d.rds", name, v))
    saveRDS(obj, path, version = v)
    message("wrote ", path)
  }
}

# Writes a "fake .rdb entry": 4-byte big-endian uncompressed size, followed
# by a raw zlib deflate stream over a bare serialize() payload.
#
# Empirically, memCompress(x, type = "gzip") in this R build actually
# produces a raw zlib stream (magic bytes 0x78 0x9c), NOT a gzip-wrapped
# one (which would start with 0x1f 0x8b) -- confirmed by inspecting the
# first bytes of its output below, and this is exactly the framing real
# <pkg>.rdb entries use. Bare serialize()/unserialize() round-tripping is
# verified once at the bottom of this script as a sanity check.
write_rdb_entry <- function(obj, name, version = 3) {
  raw <- serialize(obj, connection = NULL, version = version)
  comp <- memCompress(raw, type = "gzip")
  stopifnot(comp[1] == as.raw(0x78)) # zlib header, not gzip's 0x1f
  path <- file.path(data_dir, sprintf("%s_v%d.rdbentry", name, version))
  con <- file(path, "wb")
  on.exit(close(con))
  # Real .rdb entries use an unsigned 32-bit size prefix; this helper keeps
  # the signed 32-bit write path safe by refusing payloads that would overflow.
  stopifnot(length(raw) <= .Machine$integer.max)
  writeBin(as.integer(length(raw)), con, size = 4, endian = "big")
  writeBin(comp, con)
  message("wrote ", path)
}

parse_rd_file <- function(name) {
  # Do not embed R's GPL-licensed share/Rd/macros/system.Rd text in fixtures.
  tools::parse_Rd(file.path(src_dir, paste0(name, ".Rd")), macros = FALSE)
}

## 1. Plain minimal Rd object --------------------------------------------
## A minimal parsed-Rd-like list structure: just \name/\alias/\title/
## \description. Exercises the baseline nested-list + Rd_tag/class
## attribute shape shared by every other Rd fixture.
rd_minimal <- parse_rd_file("minimal")
save_versions(rd_minimal, "rd_minimal")

## 2. Alias-bearing topic -------------------------------------------------
## Several \alias entries: exercises repeated sibling structure and is the
## structural analog of aliases.rds's many-aliases-per-topic case.
rd_aliases <- parse_rd_file("aliases")
save_versions(rd_aliases, "rd_aliases")

## 3. Arguments-bearing topic ---------------------------------------------
## An \arguments block with five \item entries: real structural diversity
## (a genuinely nested list-of-lists) and heavy reuse of the same
## attribute-tag symbols (Rd_tag, class) across many sibling nodes.
rd_arguments <- parse_rd_file("arguments")
save_versions(rd_arguments, "rd_arguments")

## 4. Seealso/examples topic parsed from a real file on disk -------------
## Confirms (see stopifnot() below) that parsing a real .Rd file path
## attaches a genuine srcref/srcfile pair. NOTE: this only exercises an
## inline ENVSXP for the srcfile, because plain serialize()/saveRDS() (as
## used here) has no refhook and so embeds the srcfile environment
## directly -- unlike real installed-package help DBs, which are written
## with a refhook and persist the srcfile as a PERSISTSXP instead. See the
## persistsxp_* fixtures (section 10 below) for synthetic PERSISTSXP
## coverage that doesn't depend on a real installed package.
rd_seealso <- parse_rd_file("seealso")
stopifnot(!is.null(attr(rd_seealso, "srcref")))
first_child_srcref <- attr(rd_seealso[[1]], "srcref")
stopifnot(!is.null(first_child_srcref))
srcfile <- attr(first_child_srcref, "srcfile")
stopifnot(typeof(srcfile) == "environment", inherits(srcfile, "srcfile"))
save_versions(rd_seealso, "rd_seealso")

## 4b. Character-valued Rd options ----------------------------------------
## \link[pkg]{topic} and \Sexpr[options]{code} store their bracket options as
## a plain character-valued Rd_option attribute.
# Do not embed R's GPL-licensed share/Rd/macros/system.Rd text in fixtures.
rd_options <- tools::parse_Rd(textConnection(
  readLines(file.path(src_dir, "options.Rd"), warn = FALSE)
), macros = FALSE)
save_versions(rd_options, "rd_options")

## 4c. Extra (non-allowlisted) attributes on Rd nodes ----------------------
## Exercises rd-ast's attribute allowlists: nodes whose attributes stay
## within {Rd_tag, Rd_option, srcref} lower structurally (srcref
## discarded), while a node carrying anything else must fall back to the
## fully-lossless Raw representation, preserving ALL of its attributes,
## srcref included. Built from the minimal topic with:
##   - an unknown "mystery" attribute on the \title node,
##   - a "class" attribute on the \name node and a "macros" attribute on
##     the \alias node (both are only ever discarded at the ROOT, so a
##     child carrying them must go Raw),
##   - an unknown "rootextra" attribute on the root, pinning that root
##     attributes never affect lowering.
rd_extra_attr <- parse_rd_file("minimal")
node_index <- function(rd, tag_name) {
  which(vapply(rd, function(el) identical(attr(el, "Rd_tag"), tag_name),
               logical(1)))[1]
}
title_i <- node_index(rd_extra_attr, "\\title")
name_i <- node_index(rd_extra_attr, "\\name")
alias_i <- node_index(rd_extra_attr, "\\alias")
stopifnot(!is.na(title_i), !is.na(name_i), !is.na(alias_i))
attr(rd_extra_attr[[title_i]], "mystery") <- "kept"
attr(rd_extra_attr[[name_i]], "class") <- "not-root"
attr(rd_extra_attr[[alias_i]], "macros") <- "not-root-either"
attr(rd_extra_attr, "rootextra") <- "ignored"
save_versions(rd_extra_attr, "rd_extra_attr")

## 5. Repeated / shared attribute symbols (REFSXP alignment stress) ------
## The same attribute names ("class", "note") are attached repeatedly, at
## different nesting depths, so a reader must register each SYMSXP tag
## only once and correctly resolve later REFSXP back-references to it.
mk_tagged <- function(x, cls, note) {
  attr(x, "class") <- cls
  attr(x, "note") <- note
  x
}
shared_symbols <- list(
  a = mk_tagged(1, "widget", "first"),
  b = mk_tagged(list(c = mk_tagged(2, "widget", "second")), "widget", "third"),
  d = mk_tagged("x", "widget", "fourth")
)
save_versions(shared_symbols, "shared_symbols")

## 6. Shared environment + repeated symbols + vector-then-backref --------
## The same (non-singleton) environment object appears twice, the same
## symbol appears twice, and a plain integer vector is written before a
## later back-reference -- together these stress REFSXP index bookkeeping
## across mixed reference-table-eligible and non-eligible types.
e <- new.env()
e$value <- 42
attr(e, "class") <- "demo_env"
# A non-compact integer vector on purpose: compact sequences like 1:10
# serialize as an ALTREP compact_intseq node under format v3, which this
# decoder deliberately does not support (see the altrep_intseq fixture
# below). This fixture's purpose is REFSXP index bookkeeping, not ALTREP.
shared_env_refs <- list(
  vec = c(4L, 2L, 7L),
  sym_a = as.symbol("dup_sym"),
  env_first = e,
  sym_b = as.symbol("dup_sym"),
  env_second = e
)
save_versions(shared_env_refs, "shared_env_refs")
stopifnot(identical(shared_env_refs$env_first, shared_env_refs$env_second))

## 6b. ALTREP compact integer sequence (v3 only) --------------------------
## Under format version 3, R's writer serializes `1:10` as an ALTREP
## "compact_intseq" node (SEXP type 238) instead of expanding it to a plain
## INTSXP. Version 2 predates ALTREP and would materialize it to a plain
## INTSXP, which is pointless to pin here -- only v3 is written. Purpose:
## pin the decoder's documented rejection of ALTREP.
altrep_intseq <- 1:10
saveRDS(altrep_intseq, file.path(data_dir, "altrep_intseq_v3.rds"), version = 3)
message("wrote ", file.path(data_dir, "altrep_intseq_v3.rds"))

## 7. Singleton environments (no fields, just the type tag) --------------
## GLOBALENV/BASEENV/EMPTYENV serialize as a bare 4-byte type tag with no
## following fields, unlike a real (non-singleton) ENVSXP's 5 fields.
singleton_envs <- list(
  global = globalenv(),
  base = baseenv(),
  empty = emptyenv()
)
save_versions(singleton_envs, "singleton_envs")

## 8. Standalone plain (non-singleton) environment ------------------------
## An isolated real ENVSXP: locked/enclos/frame/hashtab/attrib, all
## present, outside of any Rd/srcref context.
plain_env <- new.env()
plain_env$x <- 1L
plain_env$y <- "hello"
attr(plain_env, "class") <- "plain_env_example"
save_versions(plain_env, "plain_env")

## 8c. Environment binding a closure and a promise -----------------------
## Environments may bind closures and promises; a reader that mis-frames
## CLOSXP/PROMSXP (they use the same dotted-pair wire layout as LISTSXP)
## desynchronizes here and corrupts the trailing string, so this pins the
## discard-mode walk.
e <- new.env()
e$f <- function(x) x + 1
delayedAssign("p", 1 + 2, assign.env = e)
attr(e, "class") <- "env_with_closure_demo"
env_with_closure <- list(env = e, tail = "tail-marker")
save_versions(env_with_closure, "env_with_closure")

## 8b. Namespace with a REFSXP backref (NAMESPACESXP wire coverage) ------
## A real NAMESPACESXP wire node (string-vec payload + reference-table
## registration), plus a REFSXP backref to it on the second occurrence
## (serialize() re-uses the reference table for a repeated namespace,
## unlike PERSISTSXP's refhook, which is re-invoked per occurrence -- see
## persistsxp_twice above), plus a trailing plain string proving stream
## sync afterward. Confirmed empirically: serializing a namespace works
## fine in this R version for both format versions, so no PACKAGESXP
## fallback via as.environment("package:stats") is needed.
namespace_refs <- list(
  ns_first = getNamespace("stats"),
  ns_second = getNamespace("stats"),
  tail = "tail-marker"
)
stopifnot(identical(namespace_refs$ns_first, namespace_refs$ns_second))
save_versions(namespace_refs, "namespace_refs")

## 9. aliases.rds-style named character vector ----------------------------
## Mirrors the real help/aliases.rds structure: a named character vector
## mapping alias -> Rd entry (topic) name.
aliases_vector <- c(
  minimal            = "minimal",
  multialias         = "multialias",
  "multialias-method" = "multialias",
  "multialias.default" = "multialias",
  "print.multialias" = "multialias"
)
save_versions(aliases_vector, "aliases_vector")

## 9b. aliases.rds-style named character vector with a duplicated alias --
## Real aliases.rds files can contain the same alias name twice (e.g. two
## Rd files in a package both \alias{}-ing the same name); R's own
## list2env()-based loader resolves duplicates to the LAST occurrence. This
## fixture exercises that last-wins case: "shared" appears twice, mapping
## to different topics, with "unique" in between to prove ordering is
## otherwise preserved.
aliases_vector_dup <- c(
  shared = "first-topic",
  unique = "unique-topic",
  shared = "second-topic"
)
save_versions(aliases_vector_dup, "aliases_vector_dup")

## 10. PERSISTSXP fixtures (refhook-persisted environments) --------------
## Real installed-package help DBs are written via tools:::makeLazyLoadDB(),
## which supplies a refhook to persist srcfile environments as PERSISTSXP
## nodes instead of inline ENVSXPs (see the rd_seealso note above). That
## decode path has no R-independent regression coverage otherwise, so these
## fixtures exercise it directly via serialize(..., refhook = ...).
##
## saveRDS()/readRDS() do accept a refhook argument in this R version, but
## to keep these fixtures written through the same low-level path a real
## .rdb/.rds writer uses (and to make the gzip framing explicit rather than
## relying on saveRDS()'s internals), they're written by hand here: a
## gzfile() connection plus serialize(), the same building blocks
## write_rdb_entry() above already uses for its own framing.
##
## A PERSISTSXP payload is NOT shaped like an ordinary STRSXP: empirically
## (see the stopifnot()s below) it is a leading discarded placeholder i32,
## then the actual string count as an i32 (with the usual -1 long-vector
## escape for very large counts), then that many CHARSXPs.
write_versioned_rds_with_refhook <- function(obj, path, version, refhook) {
  con <- gzfile(path, "wb")
  on.exit(close(con))
  serialize(obj, con, version = version, refhook = refhook)
}

save_versions_with_refhook <- function(obj, name, refhook) {
  for (v in c(2, 3)) {
    path <- file.path(data_dir, sprintf("%s_v%d.rds", name, v))
    write_versioned_rds_with_refhook(obj, path, v, refhook)
    message("wrote ", path)
  }
}

persist_env <- new.env()
persist_env$value <- 42
attr(persist_env, "class") <- "persist_env_demo"

## 10a. Basic case: a single persisted environment, hook returns one string.
## The hook must return NULL for anything it doesn't recognize -- returning
## a non-NULL value for the wrong object would corrupt serialization of
## everything else in the same call, so this is deliberately checked by
## also persisting a plain trailing string ("tail-marker") the hook must
## pass through untouched.
persist_hook_single <- function(x) {
  if (identical(x, persist_env)) "srcref-env" else NULL
}
persistsxp_basic <- list(env = persist_env, tail = "tail-marker")
save_versions_with_refhook(persistsxp_basic, "persistsxp_basic", persist_hook_single)

## 10b. Same environment referenced twice -- empirically confirms that a
## refhook takes precedence over back-referencing: R calls the hook again
## for the second occurrence rather than emitting a REFSXP, so a reader
## must parse two independent PERSISTSXP nodes here, each consuming its
## own reference-table slot, and still correctly resolve the plain
## trailing string afterward.
hook_call_count <- 0
persist_hook_counting <- function(x) {
  if (identical(x, persist_env)) {
    hook_call_count <<- hook_call_count + 1
    return("srcref-env")
  }
  NULL
}
persistsxp_twice <- list(first = persist_env, second = persist_env, after = "tail-marker")
twice_raw <- serialize(persistsxp_twice, connection = NULL, version = 3,
                        refhook = persist_hook_counting)
stopifnot(hook_call_count == 2) # hook re-invoked per occurrence, not backreferenced
persist_type_tag <- as.raw(c(0x00, 0x00, 0x00, 0xf7)) # PERSISTSXP == 247
tag_positions <- which(vapply(seq_len(length(twice_raw) - 3L), function(i) {
  identical(twice_raw[i:(i + 3L)], persist_type_tag)
}, logical(1)))
# This byte scan is only a heuristic for the v3 stream; it can false-positive
# on unrelated payload bytes, so it only checks that at least two candidates
# are present.
stopifnot(length(tag_positions) >= 2)
save_versions_with_refhook(persistsxp_twice, "persistsxp_twice", persist_hook_single)

## 10c. refhook returning more than one string -- exercises a PERSISTSXP
## payload length greater than 1, which a length-1-only fixture wouldn't.
persist_hook_multi <- function(x) {
  if (identical(x, persist_env)) c("a", "b", "c") else NULL
}
persistsxp_multi <- list(env = persist_env, tail = "tail-marker")
save_versions_with_refhook(persistsxp_multi, "persistsxp_multi", persist_hook_multi)

## 11. Raw zlib "fake .rdb entry" blobs -----------------------------------
## Mirrors the exact byte framing of a real <pkg>.rdb: 4-byte BE
## uncompressed size + raw zlib stream, in both serialize versions.
write_rdb_entry(rd_minimal, "rd_minimal", version = 2)
write_rdb_entry(rd_minimal, "rd_minimal", version = 3)
write_rdb_entry(rd_arguments, "rd_arguments", version = 3)

## 12. Meta/vignette.rds and Meta/demo.rds fixtures (rd-helpdb) ----------
## rd-helpdb's VignetteIndex/DemoIndex views are validated against these.
## Only version 3 is written (rd-rds's decoder is format-version-agnostic
## for the plain list/character/attribute shapes these fixtures use, so a
## v2 copy would exercise no additional decode path).
save_v3 <- function(obj, name) {
  path <- file.path(data_dir, sprintf("%s_v3.rds", name))
  saveRDS(obj, path, version = 3)
  message("wrote ", path)
}

## 12a. A two-row vignette.rds with reordered/extra columns and non-empty
## list columns -- confirms column lookup is name-based (not positional)
## and that an unrecognized extra column ("Extra") is tolerated.
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
save_v3(vignette_reordered, "vignette_reordered")

## 12b. A zero-row vignette.rds, matching packages with no vignettes that
## still ship an (empty) index.
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
save_v3(vignette_empty, "vignette_empty")

## 12c. A vignette.rds missing a required column (Keywords) -- must be
## rejected even though every remaining column is internally consistent.
vignette_missing_column <- vignette_reordered
vignette_missing_column$Keywords <- NULL
save_v3(vignette_missing_column, "vignette_missing_column")

## 12d. A vignette.rds whose row.names attribute disagrees with the
## columns' own length. All columns here mutually agree on 2 rows, but
## row.names' compact form (`c(NA, -n)`, the same wire shape a real
## data.frame's automatic row names serialize as -- see rd_seealso's
## srcref note above for another R-internal compact-encoding case) claims
## 3. Column-only cross-checks can't catch this: it must be validated
## against row.names directly.
vignette_row_names_mismatch <- structure(
  list(
    File = c("only.Rnw", "second.Rnw"),
    Title = c("Only vignette", "Second vignette"),
    PDF = c("only.pdf", "second.pdf"),
    R = c("only.R", "second.R"),
    Depends = list(character(), character()),
    Keywords = list(character(), character())
  ),
  class = "data.frame",
  row.names = c(NA_integer_, -3L)
)
save_v3(vignette_row_names_mismatch, "vignette_row_names_mismatch")

## 12e. A two-row, two-column demo.rds (Name, Title) with no dimnames --
## matches the real wire shape of an installed package's Meta/demo.rds.
demo_valid <- matrix(
  c("first", "second", "First demo", ""),
  nrow = 2L,
  ncol = 2L
)
dimnames(demo_valid) <- NULL
save_v3(demo_valid, "demo_valid")

## 12f. A zero-row demo.rds, matching packages with no demos that still
## ship an (empty) index.
demo_empty <- matrix(character(), nrow = 0L, ncol = 2L)
dimnames(demo_empty) <- NULL
save_v3(demo_empty, "demo_empty")

## 12g. A demo.rds with three columns -- real demo.rds matrices are always
## exactly two columns (Name, Title); a reader must reject anything else
## rather than silently reading the first two.
demo_three_columns <- matrix(
  c("first", "First demo", "ignored"),
  nrow = 1L,
  ncol = 3L
)
dimnames(demo_three_columns) <- NULL
save_v3(demo_three_columns, "demo_three_columns")

## Sanity check: every *_v3.rds fixture must round-trip through
## saveRDS()/readRDS(), and every *.rdbentry fixture must round-trip
## through the manual 4-byte-prefix + memDecompress(type = "gzip") +
## unserialize() path exactly the way a real .rdb reader would. The
## persistsxp_* fixtures are excluded here (see the dedicated check below)
## since a plain readRDS() with no refhook can't decode real PERSISTSXP
## content -- that's expected, the same as this project's research already
## found for real package .rdb entries.
rds_files <- list.files(data_dir, pattern = "\\.rds$", full.names = TRUE)
rds_files <- rds_files[!grepl("^persistsxp_", basename(rds_files))]
for (f in rds_files) {
  obj <- readRDS(f)
  message("round-trip OK: ", f, " (class ", paste(class(obj), collapse = ","), ")")
}

read_rdbentry <- function(f) {
  con <- file(f, "rb")
  on.exit(close(con))
  usize <- readBin(con, "integer", n = 1, size = 4, endian = "big")
  comp <- readBin(con, "raw", n = file.info(f)$size - 4)
  list(usize = usize, comp = comp)
}

rdbentry_files <- list.files(data_dir, pattern = "\\.rdbentry$", full.names = TRUE)
for (f in rdbentry_files) {
  entry <- read_rdbentry(f)
  raw <- memDecompress(entry$comp, type = "gzip")
  stopifnot(length(raw) == entry$usize)
  obj <- unserialize(raw)
  message("round-trip OK: ", f, " (class ", paste(class(obj), collapse = ","), ")")
}

# Check the complete serialized payload, including compressed RDS and rdbentry
# data, so system macro source cannot accidentally return to these fixtures.
forbidden_fixture_text <- c(
  "Rd/macros/system.Rd",
  "newcommand",
  "Rd_expr_doi",
  "CRAN.R-project.org/package="
)
scan_fixture_bytes <- function(path) {
  bytes <- readBin(path, "raw", n = file.info(path)$size)
  if (grepl("\\.rdbentry$", path)) {
    stopifnot(length(bytes) >= 4L)
    bytes <- memDecompress(bytes[-seq_len(4L)], type = "gzip")
  } else {
    bytes <- memDecompress(bytes, type = "gzip")
  }
  hits <- forbidden_fixture_text[vapply(
    forbidden_fixture_text,
    function(pattern) length(grepRaw(pattern, bytes, fixed = TRUE)) > 0,
    logical(1)
  )]
  if (length(hits)) {
    stop("forbidden text in ", path, ": ", paste(hits, collapse = ", "))
  }
}
written_files <- list.files(data_dir, pattern = "\\.(rds|rdbentry)$",
                            full.names = TRUE)
for (f in written_files) {
  scan_fixture_bytes(f)
  message("forbidden-text scan OK: ", f)
}

## Sanity check: persistsxp_*.rds fixtures must round-trip through
## readRDS() when given a refhook matching the one used to write them --
## verifying the persisted string(s) really do come back as the hook's
## input, and that the plain trailing string survives alongside them.
persistsxp_specs <- list(
  persistsxp_basic = list(
    unhook = function(name) {
      stopifnot(identical(name, "srcref-env"))
      persist_env
    },
    check = function(obj) {
      identical(obj$tail, "tail-marker") && identical(obj$env$value, 42)
    }
  ),
  persistsxp_twice = list(
    unhook = function(name) {
      stopifnot(identical(name, "srcref-env"))
      persist_env
    },
    check = function(obj) {
      identical(obj$after, "tail-marker") &&
        identical(obj$first$value, 42) && identical(obj$second$value, 42)
    }
  ),
  persistsxp_multi = list(
    unhook = function(name) {
      stopifnot(identical(name, c("a", "b", "c")))
      persist_env
    },
    check = function(obj) {
      identical(obj$tail, "tail-marker") && identical(obj$env$value, 42)
    }
  )
)

persistsxp_files <- list.files(data_dir, pattern = "^persistsxp_.*\\.rds$", full.names = TRUE)
for (f in persistsxp_files) {
  fixture_name <- sub("_v[23]\\.rds$", "", basename(f))
  spec <- persistsxp_specs[[fixture_name]]
  stopifnot(!is.null(spec))
  obj <- readRDS(f, refhook = spec$unhook)
  stopifnot(spec$check(obj))
  no_hook_ok <- tryCatch({
    readRDS(f)
    TRUE
  }, error = function(e) FALSE)
  stopifnot(!no_hook_ok) # confirms this really is a PERSISTSXP, not an inline ENVSXP
  message("round-trip OK (with refhook): ", f, " (class ", paste(class(obj), collapse = ","), ")")
}

message("\nDone. ", length(rds_files) + length(persistsxp_files), " .rds + ",
        length(rdbentry_files), " .rdbentry fixture files written to ",
        normalizePath(data_dir))
