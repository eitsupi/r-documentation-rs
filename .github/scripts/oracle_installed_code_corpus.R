#!/usr/bin/env Rscript
# Produce the independent R-side report for the installed-code corpus.
# This process reads the same installed package files as a normal R session,
# but it does not call the Rust scanner or consume its output.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 4L) {
  stop("usage: oracle_installed_code_corpus.R PROFILE LIBRARY MANIFEST OUTPUT")
}
profile <- args[[1L]]
library_dir <- normalizePath(args[[2L]], mustWork = TRUE)
manifest_path <- normalizePath(args[[3L]], mustWork = TRUE)
output_path <- args[[4L]]

fields <- c("profile", "package", "version", "source", "snapshot", "r_version",
            "os", "arch", "locale")
manifest_lines <- readLines(manifest_path, warn = FALSE)
manifest_lines <- manifest_lines[nzchar(manifest_lines) & !startsWith(manifest_lines, "#")]
manifest_rows <- do.call(rbind, lapply(manifest_lines, function(line) {
  values <- strsplit(line, "\t", fixed = TRUE)[[1L]]
  if (length(values) != length(fields)) stop("malformed manifest row")
  values
}))
colnames(manifest_rows) <- fields
rows <- manifest_rows[manifest_rows[, "profile"] == profile, , drop = FALSE]
if (!nrow(rows)) stop("manifest has no packages for profile: ", profile)
packages <- rows[, "package"]
if (anyDuplicated(packages)) stop("manifest repeats a package for profile: ", profile)
.libPaths(c(library_dir, .libPaths()))

json_string <- function(value) {
  value <- as.character(value)
  chars <- strsplit(value, "", fixed = TRUE)[[1L]]
  escaped <- vapply(chars, function(char) {
    if (char == "\"") return("\\\"")
    if (char == "\\") return("\\\\")
    if (char == "\n") return("\\n")
    if (char == "\r") return("\\r")
    if (char == "\t") return("\\t")
    code <- utf8ToInt(char)
    if (length(code) == 1L && code < 32L) return(sprintf("\\u%04x", code))
    char
  }, character(1L), USE.NAMES = FALSE)
  paste0("\"", paste0(escaped, collapse = ""), "\"")
}
json_bool <- function(value) if (isTRUE(value)) "true" else "false"
json_array <- function(values) paste0("[", paste(values, collapse = ","), "]")
json_string_array <- function(values) json_array(vapply(values, json_string, character(1L)))
json_object <- function(values) {
  if (!length(values)) return("{}")
  parts <- paste0(vapply(names(values), json_string, character(1L)), ":", unname(values))
  paste0("{", paste(parts, collapse = ","), "}")
}

kind_info <- function(value) {
  type <- typeof(value)
  if (inherits(value, "rd_rds_active")) return(list(name = "Active", code = NA_integer_))
  if (inherits(value, "rd_rds_persist")) return(list(name = "Persist", code = 247L))
  if (isS4(value) && type != "closure") return(list(name = "S4", code = 25L))
  if (is.list(value) && all(c("enclos", "bindings") %in% names(value))) {
    return(list(name = "Env", code = 4L))
  }
  mapping <- list(
    NULL = c("Nil", 0L), symbol = c("Sym", 1L), pairlist = c("PairList", 2L),
    environment = c("Env", 4L), character = c("String", 16L), logical = c("Logical", 10L),
    integer = c("Integer", 13L), double = c("Real", 14L), list = c("List", 19L),
    externalptr = c("ExtPtr", 22L), weakref = c("WeakRef", 23L),
    language = c("Lang", 6L), bytecode = c("ByteCode", 21L), raw = c("Raw", 24L),
    complex = c("Complex", 15L), closure = c("Closure", 3L), builtin = c("BuiltIn", 8L),
    special = c("Special", 7L), promise = c("Promise", 5L)
  )
  result <- mapping[[type]]
  if (is.null(result)) list(name = "Unknown", code = NA_integer_)
  else list(name = result[[1L]], code = as.integer(result[[2L]]))
}

formals_info <- function(value, kind) {
  if (kind$name %in% c("BuiltIn", "Special")) {
    return(list(state = "not_applicable", values = character(), reason = tolower(kind$name)))
  }
  if (kind$name != "Closure") {
    if (kind$name == "Promise") return(list(state = "unavailable", values = character(), reason = "promise_not_evaluated"))
    if (kind$name == "Persist") return(list(state = "unavailable", values = character(), reason = "persistent_reference_unresolved"))
    return(list(state = "not_applicable", values = character(), reason = "non_closure"))
  }
  formal_values <- tryCatch(formals(value), error = function(error) error)
  if (inherits(formal_values, "error")) {
    return(list(state = "unavailable", values = character(), reason = conditionMessage(formal_values)))
  }
  # formals(NULL) is the valid representation of a closure with no formal
  # arguments. Handle it before inspecting names so an empty pairlist cannot
  # acquire a synthetic default entry through vector recycling.
  if (!length(formal_values)) {
    return(list(state = "available", values = character(), reason = NULL))
  }
  names <- names(formal_values)
  keep <- !is.na(names) & nzchar(names)
  defaults <- vapply(which(keep), function(index) {
    missing_default <- is.symbol(formal_values[[index]]) &&
      identical(as.character(formal_values[[index]]), "")
    if (missing_default) "Absent" else "Present"
  }, character(1L))
  list(
    state = "available",
    values = paste0(names[keep], "=", defaults),
    reason = NULL
  )
}

entry_json <- function(index, name, key, data_path, compressed, references, oracle_eligible,
                       ambiguous, runtime_value) {
  result <- list(
    index = as.integer(index), name = name, record_fetch = "record_error",
    root_kind = NULL, type_code = NULL, formals_state = "not_observed", formals = character(),
    formals_phase = NULL, formals_offset = NULL, formals_reason = NULL,
    record_error = NULL, oracle_eligible = oracle_eligible
  )
  if (isTRUE(ambiguous)) {
    result$record_fetch <- "ambiguous"
    encoded <- list(
      index = as.character(result$index), name = json_string(result$name),
      record_fetch = json_string(result$record_fetch), root_kind = "null",
      type_code = "null", formals_state = json_string(result$formals_state),
      formals = json_string_array(result$formals), formals_phase = "null",
      formals_offset = "null", formals_reason = "null", record_error = "null",
      record_error_reason = "null", oracle_eligible = "false",
      runtime_formals_state = json_string("not_observed"),
      runtime_formals = "[]", runtime_formals_reason = "null"
    )
    return(json_object(encoded))
  }
  # The oracle observes each record's root shape. Persistence references are
  # represented by an empty environment so a cyclic environment graph cannot
  # turn a record fetch into unbounded recursive evaluation.
  hook <- function(reference_name) {
    if (is.null(references[[reference_name]])) {
      stop("missing persistence reference: ", reference_name)
    }
    structure(list(reference = reference_name), class = "rd_rds_persist")
  }
  value <- tryCatch(lazyLoadDBfetch(key, data_path, compressed, hook), error = function(error) error)
  if (inherits(value, "error")) {
    result$record_error <- conditionMessage(value)
  } else {
    kind <- kind_info(value)
    formals <- formals_info(value, kind)
    result$record_fetch <- "ok"
    result$root_kind <- kind$name
    result$type_code <- kind$code
    result$formals_state <- formals$state
    result$formals <- formals$values
    result$formals_reason <- formals$reason
  }
  runtime_formals <- if (isTRUE(oracle_eligible)) {
    formals_info(runtime_value, kind_info(runtime_value))
  } else {
    list(state = "not_observed", values = character(), reason = NULL)
  }
  encoded <- list(
    index = as.character(result$index), name = json_string(result$name),
    record_fetch = json_string(result$record_fetch),
    root_kind = if (is.null(result$root_kind)) "null" else json_string(result$root_kind),
    type_code = if (is.null(result$type_code) || is.na(result$type_code)) "null" else as.character(result$type_code),
    formals_state = json_string(result$formals_state), formals = json_string_array(result$formals),
    formals_phase = "null", formals_offset = "null",
    formals_reason = if (is.null(result$formals_reason)) "null" else json_string(result$formals_reason),
    record_error = if (is.null(result$record_error)) "null" else json_string(result$record_error),
    record_error_reason = if (is.null(result$record_error)) "null" else json_string("other"),
    oracle_eligible = json_bool(result$oracle_eligible),
    runtime_formals_state = json_string(runtime_formals$state),
    runtime_formals = json_string_array(runtime_formals$values),
    runtime_formals_reason = if (is.null(runtime_formals$reason)) "null" else json_string(runtime_formals$reason)
  )
  json_object(encoded)
}

declared_names <- function(namespace_info) {
  exports <- namespace_info$exports
  if (is.null(exports)) return(character())
  values <- as.character(exports)
  names <- names(exports)
  if (is.null(names)) return(values)
  names[is.na(names) | names == ""] <- values[is.na(names) | names == ""]
  names
}

runtime_kind <- function(value) {
  if (inherits(value, "error")) return(list(name = "Unavailable", code = NA_integer_))
  kind_info(value)
}

report_lines <- character()
for (package in packages) {
  package_dir <- find.package(package, quiet = TRUE)
  if (!length(package_dir)) stop("package is not installed: ", package)
  package_dir <- normalizePath(package_dir[[1L]], mustWork = TRUE)
  namespace_path <- file.path(package_dir, "Meta", "nsInfo.rds")
  namespace_info <- if (file.exists(namespace_path)) {
    tryCatch(readRDS(namespace_path), error = function(error) stop("cannot read nsInfo.rds for ",
      package, ": ", conditionMessage(error)))
  } else {
    NULL
  }
  index_path <- file.path(package_dir, "R", paste0(package, ".rdx"))
  data_path <- file.path(package_dir, "R", paste0(package, ".rdb"))
  index <- readRDS(index_path)
  variables <- index$variables
  variable_names <- names(variables)
  if (is.null(variable_names)) stop("variables map has no names for ", package)

  namespace <- asNamespace(package)
  runtime_names <- ls(namespace, all.names = TRUE)
  active_names <- runtime_names[vapply(runtime_names, function(name) {
    tryCatch(bindingIsActive(name, namespace), error = function(error) FALSE)
  }, logical(1L))]
  runtime_values <- setNames(lapply(runtime_names, function(name) {
    if (name %in% active_names) return(structure(list(name = name), class = "rd_rds_active"))
    tryCatch(get(name, envir = namespace, inherits = FALSE), error = function(error) error)
  }), runtime_names)
  runtime_kinds <- vapply(runtime_names, function(name) {
    value <- runtime_values[[name]]
    info <- runtime_kind(value)
    json_object(c(name = json_string(name), kind = json_string(info$name),
                  type_code = if (is.na(info$code)) "null" else as.character(info$code)))
  }, character(1L))
  runtime_kind_names <- vapply(runtime_names, function(name) {
    value <- runtime_values[[name]]
    runtime_kind(value)$name
  }, character(1L))
  duplicate_names <- duplicated(variable_names) | duplicated(variable_names, fromLast = TRUE)
  entries <- vapply(seq_along(variables), function(position) {
    entry_json(position - 1L, variable_names[[position]], variables[[position]], data_path,
               isTRUE(index$compressed), index$references,
               variable_names[[position]] %in% runtime_names &&
                 !(variable_names[[position]] %in% active_names) &&
                 runtime_kind_names[[variable_names[[position]]]] == "Closure",
               duplicate_names[[position]], runtime_values[[variable_names[[position]]]])
  }, character(1L))
  report_lines <- c(report_lines, json_object(c(
    schema = json_string("installed-code-corpus/v1"), profile = json_string(profile),
    package = json_string(package), package_version = json_string(rows[rows[, "package"] == package, "version"]),
    stored_entries = json_array(entries), declared_names = json_string_array(declared_names(namespace_info)),
    runtime_names = json_string_array(runtime_names), active_bindings = json_string_array(active_names),
    runtime_kinds = json_array(runtime_kinds), oracle_error = "null"
  )))
}
writeLines(report_lines, output_path, useBytes = TRUE)
