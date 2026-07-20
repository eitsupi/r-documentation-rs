# Generate paired user-macro fixtures with direct and refhook provenance.
# Provenance: R 4.6.1; run with `Rscript generate_user_macros_fixture.R`.

script_path <- sub("^--file=", "", commandArgs(trailingOnly = FALSE)[
    grep("^--file=", commandArgs(trailingOnly = FALSE))
])
setwd(dirname(normalizePath(script_path)))

stopifnot(getRversion() == "4.6.1")
dir.create("data", showWarnings = FALSE)
rd <- tools::parse_Rd("rd-src/user-macros.Rd", macros = "rd-src/fixture-macros.Rd")
# The macro table's parent holds parser closures whose source text includes the
# macro-definition command. Keep the parsed USERMACRO markers and their
# environment-backed srcrefs, but detach that implementation environment so
# no definition source is serialized into this fixture.
parent.env(attr(rd, "macros")) <- emptyenv()

saveRDS(rd, "data/rd_user_macros_v3.rds", version = 3)
connection <- file("data/rd_user_macros_refhook_v3.rds", open = "wb")
serialize(rd, connection, version = 3, refhook = function(environment) "srcfile")
close(connection)

forbidden_fixture_text <- c("Rd/macros/system.Rd", "system.Rd", "newcommand")
written_files <- file.path("data", c("rd_user_macros_v3.rds", "rd_user_macros_refhook_v3.rds"))
for (path in written_files) {
  bytes <- readBin(path, "raw", n = file.info(path)$size)
  if (grepl("\\.rds$", path)) bytes <- tryCatch(memDecompress(bytes, type = "gzip"), error = function(e) bytes)
  hits <- forbidden_fixture_text[vapply(forbidden_fixture_text,
    function(pattern) length(grepRaw(pattern, bytes, fixed = TRUE)) > 0, logical(1))]
  if (length(hits)) stop("forbidden text in ", path, ": ", paste(hits, collapse = ", "))
  message("forbidden-text scan OK: ", path)
}
