#!/usr/bin/env Rscript

# TODO: consider running the oracle scripts via r-lib/ir
# (https://github.com/r-lib/ir) so the pinned R version and package
# dependencies are self-described in the script instead of relying on the
# ambient R installation.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 2L) stop("usage: corpus_oracle.R FILE_LIST OUTPUT_DIR")
expected_r_version <- "4.6.1"
if (!identical(as.character(getRversion()), expected_r_version))
  stop("corpus oracle requires R ", expected_r_version, "; found R ", getRversion())
list_path <- args[[1L]]
output_dir <- args[[2L]]
dir.create(output_dir, recursive = TRUE, showWarnings = FALSE)
paths <- readLines(list_path, encoding = "UTF-8", warn = FALSE)

strip_srcref <- function(x) {
  if (is.list(x)) for (i in seq_along(x)) x[[i]] <- strip_srcref(x[[i]])
  a <- attributes(x)
  if (!is.null(a)) {
    for (name in setdiff(names(a), c("srcref", "wholeSrcref", "srcfile"))) attr(x, name) <- strip_srcref(a[[name]])
    for (name in intersect(names(a), c("srcref", "wholeSrcref", "srcfile"))) attr(x, name) <- NULL
  }
  x
}

serialize_bytes <- function(tree) {
  con <- rawConnection(raw(), "wb")
  on.exit(close(con), add = TRUE)
  serialize(tree, con, ascii = FALSE, xdr = TRUE, version = 3)
  rawConnectionValue(con)
}

parse_one <- function(path) {
  size <- unname(file.info(path)$size)
  bytes <- if (size == 0L) raw() else readBin(path, "raw", size)
  text <- rawToChar(bytes)
  con <- rawConnection(bytes, "rb")
  on.exit(close(con), add = TRUE)
  warnings <- character()
  value <- withCallingHandlers(
    tryCatch(
      tools::parse_Rd(con, srcfile = srcfilecopy("rd-source-corpus.Rd", text), encoding = "UTF-8"),
      error = function(e) structure(list(message = conditionMessage(e)), class = "corpus-error")
    ),
    warning = function(w) {
      warnings <<- c(warnings, conditionMessage(w))
      invokeRestart("muffleWarning")
    }
  )
  if (inherits(value, "corpus-error"))
    return(list(status = "error", message = value$message))
  value <- strip_srcref(value)
  attributes(value) <- NULL
  if (length(warnings)) return(list(status = "warning", message = paste(warnings, collapse = " | ")))
  list(status = "ok", bytes = serialize_bytes(value))
}

write_result <- function(index, result) {
  if (identical(result$status, "ok"))
    writeBin(result$bytes, file.path(output_dir, paste0(index - 1L, ".rds")))
  detail <- if (is.null(result$message)) "" else gsub("[\r\n\t]", " ", result$message)
  write(paste(index, result$status, detail, sep = "\t"), manifest, append = TRUE)
}

manifest <- file.path(output_dir, "manifest.tsv")
file.create(manifest)
for (index in seq_along(paths)) {
  result <- tryCatch({
    setTimeLimit(cpu = 10, elapsed = 10, transient = TRUE)
    parse_one(paths[[index]])
  }, error = function(e) {
    if (grepl("time limit|elapsed time|CPU time", conditionMessage(e), ignore.case = TRUE))
      list(status = "timeout", message = conditionMessage(e))
    else list(status = "error", message = conditionMessage(e))
  })
  setTimeLimit(cpu = Inf, elapsed = Inf, transient = FALSE)
  write_result(index, result)
}
