# Generate deterministic closure-prefix fixtures for the crate-private
# serialized-object inspector. Run with R 4.6.1 (or a compatible R release).
# The primary output is four complete XDR RDS streams: plain and
# compiler-produced closures for serialization formats 2 and 3. Additional
# streams below cover diagnostic prefix shapes and a namespace environment.

args <- commandArgs(trailingOnly = TRUE)
if (length(args) != 1L) stop("usage: Rscript generate_closure_inspection_fixture.R OUTPUT_DIR")
out <- normalizePath(args[[1]], mustWork = FALSE)
dir.create(out, recursive = TRUE, showWarnings = FALSE)

make_closure <- function(alpha, ..., `not syntactic` = NULL, unicode_placeholder = NULL, omega = 42) alpha
unicode_name <- "非構文"
Encoding(unicode_name) <- "UTF-8"
# Replace an ASCII placeholder so the UTF-8 marker remains on the symbol in
# both serialization formats. Defining the Unicode name directly can intern
# it as a native symbol on some R versions.
formal_names <- names(formals(make_closure))
formal_names[4L] <- unicode_name
names(formals(make_closure)) <- formal_names
for (version in c(2L, 3L)) {
  compiled_closure <- compiler::cmpfun(make_closure)
  writeBin(
    serialize(make_closure, NULL, version = version),
    file.path(out, sprintf("closure_formals_v%d.rds", version))
  )
  writeBin(
    serialize(compiled_closure, NULL, version = version),
    file.path(out, sprintf("closure_formals_compiled_v%d.rds", version))
  )
}

write_v3 <- function(object, name) {
  writeBin(serialize(object, NULL, version = 3L), file.path(out, name))
}

write_v3_with_refhook <- function(object, name) {
  connection <- file(file.path(out, name), open = "wb")
  serialize(
    object,
    connection,
    version = 3L,
    refhook = function(environment) "closure-environment"
  )
  close(connection)
}

# These small diagnostic streams are all produced by R serialization itself.
# They deliberately exercise unsupported objects in different inspected
# prefix phases without copying any serializer implementation.
default_holder <- function(value) value
compiled_default <- compiler::cmpfun(function(inner) inner)
default_formals <- formals(default_holder)
default_formals$value <- compiled_default
formals(default_holder) <- default_formals
default_holder <- compiler::cmpfun(default_holder)
write_v3(default_holder, "closure_default_compiled_v3.rds")

altrep_holder <- function(value) value
altrep_formals <- formals(altrep_holder)
altrep_formals$value <- 1:1000000
formals(altrep_holder) <- altrep_formals
write_v3(altrep_holder, "closure_default_altrep_v3.rds")

inline_environment <- compiler::cmpfun(function(value) value)
inline_environment <- local({
  env <- new.env(parent = emptyenv())
  assign("helper", inline_environment, envir = env)
  environment(inline_environment) <- env
  inline_environment
})
write_v3(inline_environment, "closure_environment_compiled_v3.rds")

attributes_compiled <- make_closure
attr(attributes_compiled, "inspection_payload") <- compiled_default
write_v3(attributes_compiled, "closure_attributes_compiled_v3.rds")

setClass("InspectionS4", slots = c(value = "integer"))
attributes_s4 <- make_closure
attr(attributes_s4, "inspection_s4") <- new("InspectionS4", value = 1L)
write_v3(attributes_s4, "closure_attributes_s4_v3.rds")

namespace_closure <- make_closure
environment(namespace_closure) <- asNamespace("stats")
write_v3(namespace_closure, "closure_namespace_v3.rds")

persisted_closure <- make_closure
environment(persisted_closure) <- new.env(parent = emptyenv())
write_v3_with_refhook(persisted_closure, "closure_environment_persisted_v3.rds")
