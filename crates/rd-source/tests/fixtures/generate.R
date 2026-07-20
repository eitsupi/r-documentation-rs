#!/usr/bin/env Rscript

# TODO: consider running the oracle scripts via r-lib/ir
# (https://github.com/r-lib/ir) so the pinned R version and the RcppTOML
# dependency are self-described in the script instead of relying on the
# ambient R installation.

args <- commandArgs(trailingOnly = TRUE)
mode <- if (length(args) == 0L) "--check" else args[[1L]]
if (!mode %in% c("--check", "--update")) stop("use --check or --update")
expected_r_version <- "4.6.1"
if (!identical(as.character(getRversion()), expected_r_version))
  stop("fixture oracle requires R ", expected_r_version, "; found R ", getRversion())

script <- normalizePath(sub("^--file=", "", commandArgs()[grep("^--file=", commandArgs())][1L]))
root <- dirname(script)
rd_dir <- file.path(root, "rd")
oracle_dir <- file.path(root, "oracle")
manifest_path <- file.path(root, "cases.toml")

if (!requireNamespace("RcppTOML", quietly = TRUE)) stop("RcppTOML is required to read cases.toml")
# escape = FALSE returns decoded string values; the default re-escapes
# LF/CR/backslash (but not TAB) for display.
toml <- RcppTOML::parseTOML(manifest_path, escape = FALSE)
as_array <- function(x) if (is.null(x)) NULL else if (is.character(x) || is.numeric(x)) as.vector(x) else x
normalize_obligation_set <- function(set) {
  if (is.null(set)) return(NULL)
  if (is.null(set$labels)) set$labels <- character()
  for (field in c("labels", "comment_paths", "comment_values", "expected_leaf_kinds", "required_tags"))
    if (!is.null(set[[field]])) set[[field]] <- as_array(set[[field]])
  set
}
normalize_case <- function(case) {
  if (is.null(case$obligations)) case$obligations <- character()
  for (field in c("obligations", "comment_paths", "comment_values", "expected_leaf_kinds", "required_tags"))
    if (!is.null(case[[field]])) case[[field]] <- as_array(case[[field]])
  case$oracle_obligations <- normalize_obligation_set(case$oracle_obligations)
  case$source_obligations <- normalize_obligation_set(case$source_obligations)
  case
}
cases <- lapply(toml$case, normalize_case)
rd_names <- sub("\\.Rd$", "", list.files(rd_dir, pattern = "\\.Rd$"))
oracle_names <- sub("\\.rds$", "", list.files(oracle_dir, pattern = "\\.rds$"))
manifest_names <- vapply(cases, `[[`, character(1), "name")
oracle_manifest_names <- manifest_names[vapply(cases, function(x) !identical(x$comparison, "source-only"), logical(1))]
if (!setequal(rd_names, manifest_names) ||
    (!identical(mode, "--update") && !setequal(oracle_manifest_names, oracle_names)) ||
    (identical(mode, "--update") && !all(oracle_names %in% oracle_manifest_names))) {
  stop("rd/, oracle/, and cases.toml basenames do not agree")
}

strip_srcref <- function(x) {
  if (is.list(x)) {
    for (i in seq_along(x)) x[[i]] <- strip_srcref(x[[i]])
  }
  a <- attributes(x)
  if (!is.null(a)) {
    for (name in setdiff(names(a), c("srcref", "wholeSrcref", "srcfile"))) attr(x, name) <- strip_srcref(a[[name]])
    for (name in intersect(names(a), c("srcref", "wholeSrcref", "srcfile"))) attr(x, name) <- NULL
  }
  x
}

assert_clean <- function(x, fixture_root) {
  visit <- function(value) {
    attrs <- attributes(value)
    if (!is.null(attrs)) {
      forbidden <- intersect(names(attrs), c("srcref", "wholeSrcref", "srcfile"))
      if (length(forbidden)) stop("sanitized fixture retained attributes: ", paste(forbidden, collapse = ", "))
      for (attribute in attrs) visit(attribute)
    }
    if (is.character(value) && length(value) == 1L && !is.na(value) && grepl(fixture_root, value, fixed = TRUE))
      stop("sanitized fixture retained fixture root path")
    if (is.list(value)) for (child in value) visit(child)
  }
  visit(x)
  invisible(x)
}

parse_fixture <- function(path, synthetic_name = "fixture.Rd") {
  size <- unname(file.info(path)$size)
  bytes <- if (size == 0L) raw() else readBin(path, "raw", size)
  text <- rawToChar(bytes)
  con <- rawConnection(bytes, open = "rb")
  on.exit(close(con), add = TRUE)
  parsed <- withCallingHandlers(
    tools::parse_Rd(con, srcfile = srcfilecopy(synthetic_name, text),
                    encoding = "UTF-8"),
    warning = function(w) stop(path, ": ", conditionMessage(w), call. = FALSE)
  )
  parsed <- strip_srcref(parsed)
  attributes(parsed) <- NULL
  assert_clean(parsed, normalizePath(root, winslash = "/", mustWork = TRUE))
  parsed
}

node_tag <- function(x) {
  tag <- attr(x, "Rd_tag")
  if (length(tag) == 1L && !is.na(tag)) as.character(tag) else ""
}
walk_nodes <- function(x, f) {
  f(x)
  if (is.list(x)) for (child in x) walk_nodes(child, f)
}

check_case <- function(case, tree) {
  comments <- list(); whitespace <- FALSE; groups <- FALSE; option_values <- character(); absent_links <- 0L; adjacent_text <- FALSE
  walk_nodes(tree, function(x) {
    tag <- node_tag(x)
    if (is.list(x) && length(x) >= 2L) {
      for (i in seq_len(length(x) - 1L)) {
        if (identical(node_tag(x[[i]]), "TEXT") && identical(node_tag(x[[i + 1L]]), "TEXT")) adjacent_text <<- TRUE
      }
    }
    if (identical(tag, "COMMENT")) comments[[length(comments) + 1L]] <<- x
    if (identical(tag, "TEXT") && length(x) == 1L && grepl("^[[:space:]]+$", as.character(x[[1L]]))) whitespace <<- TRUE
    if (is.list(x) && !nzchar(tag) && !is.null(x)) groups <<- TRUE
    option <- attr(x, "Rd_option")
    if (identical(tag, "\\link") && is.null(option)) absent_links <<- absent_links + 1L
    if (!is.null(option)) option_values <<- c(option_values, if (length(option) == 0L) "empty" else "nonempty")
  })
  stopifnot(length(comments) == case$comments)
  if (length(comments)) stopifnot(all(vapply(comments, function(x) startsWith(as.character(x[[1L]]), "%"), logical(1))))
  labels <- case$obligations
  obligation <- function(label) label %in% labels
  known <- c("adjacent_text", "whitespace_text", "comments", "comment_paths", "groups", "option_absent", "option_nonempty", "option_present_empty", "decoded_escapes", "unicode_exact", "node_sequence", "exact_leaf_values", "leaf_kinds", "decoded_escapes_by_kind", "group_shape", "empty_document", "document_shape", "tag_arity", "option_shape", "option_nodes", "crlf_source", "required_tags")
  for (label in labels[nzchar(labels)]) if (!(label %in% known)) stop(case$name, ": unknown obligation label: ", label)
  consistency <- list(
    list(field = "comment_paths", label = "comment_paths"),
    list(field = "comment_values", label = "comment_paths"),
    list(field = "expected_sequence", label = "node_sequence"),
    list(field = "expected_leaves", label = "exact_leaf_values"),
    list(field = "expected_leaf_kinds", label = "leaf_kinds"),
    list(field = "expected_decoded", label = "decoded_escapes_by_kind"),
    list(field = "expected_groups", label = "group_shape"),
    list(field = "expected_root_nodes", label = "document_shape"),
    list(field = "expected_arities", label = "tag_arity"),
    list(field = "expected_options", label = "option_shape"),
    list(field = "expected_option_nodes", label = "option_nodes"),
    list(field = "required_tags", label = "required_tags")
  )
  for (entry in consistency) {
    present <- entry$field %in% names(case)
    nonempty <- present && length(case[[entry$field]]) > 0L
    obligated <- obligation(entry$label)
    if (present && !obligated)
      stop(case$name, ": manifest field '", entry$field, "' is present but obligation label '", entry$label, "' is absent")
    if (obligated && !isTRUE(entry$optional) && !nonempty)
      stop(case$name, ": obligation label '", entry$label, "' requires non-empty field '", entry$field, "'")
  }
  comment_pairs <- list()
  collect <- function(x, path) {
    if (identical(node_tag(x), "COMMENT")) comment_pairs[[length(comment_pairs) + 1L]] <<- c(path, as.character(x[[1L]]))
    if (is.list(x)) for (i in seq_along(x)) collect(x[[i]], paste0(path, "/", i - 1L))
  }
  for (i in seq_along(tree)) collect(tree[[i]], paste0("root/", i - 1L))
  if (obligation("comment_paths")) {
    paths <- case$comment_paths
    values <- case$comment_values
    stopifnot(identical(vapply(comment_pairs, `[[`, character(1), 1L), paths), identical(vapply(comment_pairs, `[[`, character(1), 2L), values))
  }
  kind <- function(x) { tag <- node_tag(x); if (!nzchar(tag)) if (is.list(x)) "GROUP" else "TEXT" else sub("^\\\\", "", tag) }
  node_at <- function(path) { if (path == "root") return(tree); ix <- as.integer(strsplit(sub("^root/", "", path), "/", fixed = TRUE)[[1L]]) + 1L; z <- tree[[ix[[1L]]]]; if (length(ix) > 1L) for (i in ix[-1L]) z <- z[[i]]; z }
  leaves <- list(); leaf_walk <- function(x) { if (!is.list(x) && length(x) == 1L && kind(x) %in% c("TEXT", "RCODE", "VERB", "COMMENT")) leaves[[length(leaves) + 1L]] <<- c(kind(x), as.character(x[[1L]])) else if (is.list(x)) for (y in x) leaf_walk(y) }; for (x in tree) leaf_walk(x)
  if (obligation("node_sequence")) for (entry in case$expected_sequence) { actual <- vapply(node_at(entry$path), kind, character(1)); expected <- unname(entry$kinds); if (!identical(actual, expected)) stop(case$name, ": node sequence at ", entry$path, " expected ", paste(expected, collapse=","), " got ", paste(actual, collapse=",")) }
  if (obligation("exact_leaf_values")) { expected <- lapply(case$expected_leaves, function(x) unname(c(x$kind, x$value))); if (!identical(leaves, expected)) stop(case$name, ": leaves expected ", paste(vapply(expected, paste, collapse=":", character(1)), collapse="|"), " got ", paste(vapply(leaves, paste, collapse=":", character(1)), collapse="|")) }
  if (obligation("leaf_kinds")) { actual <- vapply(leaves, `[[`, character(1), 1L); expected <- unname(case$expected_leaf_kinds); if (!identical(actual, expected)) stop(case$name, ": leaf kinds expected ", paste(expected,collapse="|"), " got ",paste(actual,collapse="|")) }
  if (obligation("decoded_escapes_by_kind")) for (entry in case$expected_decoded) { vals <- vapply(leaves[vapply(leaves, `[[`, character(1), 1L) == entry$kind], `[[`, character(1), 2L); if (!any(vapply(vals, function(v) grepl(entry$value, v, fixed=TRUE), logical(1)))) stop(case$name, ": decoded value mismatch ", entry$kind, ":", encodeString(entry$value)); stopifnot(!any(vapply(vals, function(v) any(vapply(c("\\%", "\\{", "\\}", "\\\\"), function(e) grepl(e, v, fixed=TRUE), logical(1))), logical(1)))) }
  if (obligation("group_shape")) for (entry in case$expected_groups) { z <- node_at(entry$path); stopifnot(is.list(z), !nzchar(node_tag(z)), is.null(attr(z,"Rd_option")), length(z) == entry$children) }
  if (obligation("empty_document")) stopifnot(is.list(tree), length(tree) == 0L)
  if (obligation("document_shape")) stopifnot(length(tree) == case$expected_root_nodes)
  if (obligation("tag_arity")) {
    actual <- list()
    walk_nodes(tree, function(x) {
      tag <- node_tag(x)
      if (nzchar(tag) && is.list(x)) {
        named <- sub("^\\\\", "", tag)
        positional <- sum(vapply(x, function(child) is.list(child) && !nzchar(node_tag(child)), logical(1)))
        actual[[named]] <<- c(actual[[named]], positional)
      }
    })
    for (entry in case$expected_arities) {
      if (!identical(actual[[entry$tag]], entry$counts)) stop(case$name, ": tag arity mismatch for ", entry$tag)
    }
  }
  if (obligation("option_shape")) {
    expected <- vapply(case$expected_options, function(x) paste(x$tag, x$presence, sep=":"), character(1))
    wanted <- unique(vapply(case$expected_options, `[[`, character(1), "tag"))
    actual <- character()
    walk_nodes(tree, function(x) {
      tag <- sub("^\\\\", "", node_tag(x))
      if (tag %in% wanted) {
        option <- attr(x, "Rd_option")
        actual <<- c(actual, paste(tag, if (is.null(option)) "absent" else if (length(option) == 0L) "empty" else "nonempty", sep = ":"))
      }
    })
    if (!identical(actual, expected)) stop(case$name, ": option shape mismatch")
  }
  if (obligation("option_nodes")) {
    descriptor <- function(x) {
      tag <- node_tag(x)
      kind <- if (nzchar(tag)) sub("^\\\\", "", tag) else if (is.list(x)) "GROUP" else "TEXT"
      leaf <- if (!is.list(x) && length(x) == 1L) as.character(x[[1L]]) else if (is.list(x) && length(x) == 1L && !is.null(node_tag(x)) && nzchar(node_tag(x)) && !is.list(x[[1L]]) && length(x[[1L]]) == 1L) as.character(x[[1L]][[1L]]) else NULL
      if (is.null(leaf)) kind else paste0(kind, ":", leaf)
    }
    actual <- character()
    walk_nodes(tree, function(x) {
      option <- attr(x, "Rd_option")
      if (!is.null(option)) {
        nodes <- if (nzchar(node_tag(option))) list(option) else option
        actual <<- c(actual, paste0(sub("^\\\\", "", node_tag(x)), ":", paste(vapply(nodes, descriptor, character(1)), collapse = ",")))
      }
    })
    expected <- vapply(case$expected_option_nodes, function(entry) paste0(entry$tag, ":", paste(vapply(entry$nodes, function(node) if (is.null(node$value)) node$kind else paste(node$kind, node$value, sep=":"), character(1)), collapse=",")), character(1))
    if (!identical(actual, expected)) stop(case$name, ": option nodes mismatch: expected ", paste(expected, collapse = "|"), " got ", paste(actual, collapse = "|"))
  }
  if (obligation("crlf_source")) {
    path <- file.path(root, case$rd)
    bytes <- readBin(path, "raw", file.info(path)$size)
    lf <- which(bytes == as.raw(10))
    stopifnot(length(lf) > 0L, any(bytes[-length(bytes)] == as.raw(13) & bytes[-1L] == as.raw(10)), all(lf > 1L), all(bytes[lf - 1L] == as.raw(13)))
  }
  if (obligation("whitespace_text")) stopifnot(whitespace)
  if (obligation("groups")) stopifnot(groups)
  if (obligation("option_nonempty")) stopifnot("nonempty" %in% option_values)
  if (obligation("option_absent")) stopifnot(absent_links > 0L)
  if (obligation("option_present_empty")) stopifnot("empty" %in% option_values)
  if (obligation("required_tags")) {
    wanted <- case$required_tags
    found <- character()
    walk_nodes(tree, function(x) {
      tag <- sub("^\\\\", "", node_tag(x))
      if (nzchar(tag)) found <<- c(found, tag)
    })
    for (tag in wanted) if (!(tag %in% found)) stop(case$name, ": required tag missing: ", tag)
  }
  if (obligation("adjacent_text")) stopifnot(adjacent_text)
  tree
}

cat("R version:", R.version.string, "\n")
cat("parse settings: encoding=UTF-8, keep.source=TRUE, permissive=FALSE, xdr=TRUE, version=3, compression=none\n")
serialize_oracle <- function(tree, con) {
  on.exit(close(con), add = TRUE)
  serialize(tree, con, ascii = FALSE, xdr = TRUE, version = 3)
}
serialize_bytes <- function(tree) {
  con <- rawConnection(raw(), "wb")
  on.exit(close(con), add = TRUE)
  serialize(tree, con, ascii = FALSE, xdr = TRUE, version = 3)
  rawConnectionValue(con)
}
# root is crates/rd-source/tests/fixtures, so the repository root is four
# levels up (fixtures -> tests -> rd-source -> crates -> repo).
repository_root <- normalizePath(file.path(root, "../../../.."), winslash = "/", mustWork = TRUE)
if (!file.exists(file.path(repository_root, "Cargo.toml")))
  stop("derived repository root looks wrong: ", repository_root)
repository_root_bytes <- charToRaw(repository_root)
assert_no_repository_root <- function(bytes, case_name) {
  if (length(repository_root_bytes) && length(bytes) >= length(repository_root_bytes)) {
    for (i in seq_len(length(bytes) - length(repository_root_bytes) + 1L))
      if (identical(bytes[i:(i + length(repository_root_bytes) - 1L)], repository_root_bytes))
        stop("oracle contains repository root path for ", case_name)
  }
}
for (case in cases) {
  if (identical(case$comparison, "source-only")) {
    parse_fixture(file.path(root, case$rd))
    next
  }
  oracle_case <- case
  if (identical(case$comparison, "intentional-divergence")) {
    oracle_case <- modifyList(oracle_case, case$oracle_obligations)
    oracle_case$obligations <- oracle_case$labels
    oracle_case$labels <- NULL
  }
  tree <- check_case(oracle_case, parse_fixture(file.path(root, case$rd)))
  target <- file.path(oracle_dir, paste0(case$name, ".rds"))
  if (mode == "--update") {
    actual <- serialize_bytes(tree)
    assert_no_repository_root(actual, case$name)
    writeBin(actual, target)
  } else {
    actual <- serialize_bytes(tree)
    assert_no_repository_root(actual, case$name)
    expected <- readBin(target, "raw", file.info(target)$size)
    if (!identical(expected, actual)) stop("oracle differs for ", case$name, "; run --update")
    variant <- parse_fixture(file.path(root, case$rd), "/tmp/variant/fixture.Rd")
    if (!identical(actual, serialize_bytes(variant))) stop("oracle changes with synthetic source path for ", case$name)
  }
}
cat("Processed", length(cases), "foundation fixtures in", mode, "mode.\n")
