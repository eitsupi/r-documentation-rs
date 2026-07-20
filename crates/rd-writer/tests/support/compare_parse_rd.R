args <- commandArgs(trailingOnly = TRUE)
if (length(args) == 2) {
  parse_with_warnings <- function(path) {
    warnings <- character()
    tryCatch(
      list(
        value = withCallingHandlers(
          tools::parse_Rd(path),
          warning = function(condition) {
            warnings <<- c(warnings, conditionMessage(condition))
            invokeRestart("muffleWarning")
          }
        ),
        warnings = warnings,
        error = NULL
      ),
      error = function(condition) {
        list(value = NULL, warnings = warnings, error = conditionMessage(condition))
      }
    )
  }
  normalize <- function(x) {
    if (is.list(x)) x <- lapply(x, normalize)
    for (name in c("srcref", "Rd_srcref", "macros")) attr(x, name) <- NULL
    x
  }
  original <- parse_with_warnings(args[[1]])
  written <- parse_with_warnings(args[[2]])
  if (!is.null(original$error)) {
    message("Skipping R-incompatible dialect fixture: ", original$error)
    quit(save = "no", status = 0)
  }
  if (!is.null(written$error)) {
    stop("written output failed to parse in R: ", written$error)
  }
  stopifnot(identical(normalize(original$value), normalize(written$value)))
  unexpected <- setdiff(written$warnings, original$warnings)
  if (length(unexpected) > 0) stop(paste("new warnings:", paste(unexpected, collapse = "; ")))
}
