#!/usr/bin/env bash
# Prepare the fixed-snapshot package library used by the installed-code corpus
# jobs. Base and stats are supplied by setup-r; external packages are resolved
# from the pinned repository snapshot.
set -euo pipefail

if [[ "$#" -ne 4 ]]; then
  echo "usage: $0 PROFILE LIBRARY MANIFEST PROVENANCE" >&2
  exit 2
fi

profile="$1"
library="$2"
manifest="$3"
provenance="$4"
mkdir -p "$library"

printf 'profile\tpackage\tpackage_version\tsource\tsnapshot\tr_version\tos\tarch\tlocale\tinstalled_path\n' > "$provenance"

# Validate the complete manifest before dependency installation.
declare -A manifest_packages
while IFS=$'\t' read -r row_profile package version source snapshot r_version os arch locale; do
  [[ -z "$row_profile" || "$row_profile" == \#* || "$row_profile" != "$profile" ]] && continue
  [[ -z "${manifest_packages[$package]+present}" ]] || {
    echo "manifest repeats package $package for $profile" >&2
    exit 1
  }
  manifest_packages[$package]=1
  if [[ "$source" == P3M-snapshot && "$snapshot" != "2026-09-01" ]]; then
    echo "P3M snapshot for $profile/$package must be 2026-09-01, got $snapshot" >&2
    exit 1
  fi
done < "$manifest"
for package in base stats Matrix dplyr rlang R6; do
  [[ -n "${manifest_packages[$package]+present}" ]] || {
    echo "manifest is missing $package for $profile" >&2
    exit 1
  }
done

Rscript --vanilla - "$library" <<'RS'
args <- commandArgs(trailingOnly = TRUE)
library_dir <- args[[1L]]
options(repos = c(CRAN = "https://packagemanager.posit.co/cran/2026-09-01"))
.libPaths(c(library_dir, .libPaths()))

# TODO: Consider replacing this bootstrap installation with a declarative
# package manifest, following approaches used by tools such as rig or rv.
install.packages(c("Matrix", "dplyr", "rlang", "R6"),
                 lib = library_dir,
                 dependencies = c("Depends", "Imports", "LinkingTo"), quiet = TRUE)
RS

declare -A seen_packages
while IFS=$'\t' read -r row_profile package version source snapshot r_version os arch locale; do
  [[ -z "$row_profile" || "$row_profile" == \#* ]] && continue
  [[ "$row_profile" == "$profile" ]] || continue
  [[ -z "${seen_packages[$package]+present}" ]] || {
    echo "manifest repeats package $package for $profile" >&2
    exit 1
  }
  seen_packages[$package]=1
  installed_path="$library/$package"
  if [[ "$source" == setup-r ]]; then
    # Base and stats are supplied by setup-r.
    system_path="$(Rscript --vanilla -e 'cat(system.file(package=commandArgs(TRUE)[[1]]))' "$package")"
    [[ -d "$system_path" ]] || {
      echo "R installation has no package $package" >&2
      exit 1
    }
    if [[ ! -e "$installed_path" ]]; then
      ln -s "$system_path" "$installed_path"
    fi
  elif [[ "$source" != P3M-snapshot ]]; then
    echo "unknown package source $source for $package" >&2
    exit 1
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$row_profile" "$package" "$version" "$source" "$snapshot" "$r_version" \
    "$os" "$arch" "$locale" "$installed_path" >> "$provenance"
done < "$manifest"

for package in base stats Matrix dplyr rlang R6; do
  [[ -n "${seen_packages[$package]+present}" ]] || {
    echo "manifest is missing $package for $profile" >&2
    exit 1
  }
done

printf '# installed_package\tprofile\texpected_version\tactual_version\tbuilt\tr_version\tos\tarch\tlocale\tinstalled_path\n' >> "$provenance"
Rscript --vanilla - "$library" "$profile" "$manifest" >> "$provenance" <<'RS'
args <- commandArgs(trailingOnly = TRUE)
library_dir <- args[[1L]]
profile <- args[[2L]]
manifest_path <- args[[3L]]
.libPaths(c(library_dir, .libPaths()))

manifest_lines <- readLines(manifest_path, warn = FALSE)
manifest_lines <- manifest_lines[nzchar(manifest_lines) & !startsWith(manifest_lines, "#")]
manifest_rows <- lapply(manifest_lines, function(line) strsplit(line, "\t", fixed = TRUE)[[1L]])
manifest_rows <- Filter(function(fields) identical(fields[[1L]], profile), manifest_rows)
for (fields in manifest_rows) {
  package <- fields[[2L]]
  expected_version <- fields[[3L]]
  package_path <- find.package(package, lib.loc = library_dir, quiet = TRUE)
  if (length(package_path) != 1L) stop("package is not installed in library: ", package)
  package_path <- normalizePath(package_path, mustWork = TRUE)
  installed_path <- normalizePath(file.path(library_dir, package), mustWork = TRUE)
  if (!identical(package_path, installed_path)) {
    stop("installed path mismatch for ", package, ": expected ", installed_path,
         ", got ", package_path)
  }
  description <- packageDescription(package, lib.loc = library_dir)
  actual_version <- description$Version
  parsed_actual_version <- packageVersion(package, lib.loc = library_dir)
  if (expected_version != "record-at-run" &&
      (!identical(actual_version, expected_version) ||
       !identical(parsed_actual_version, package_version(expected_version)))) {
    stop("installed version mismatch for ", package, ": expected ", expected_version,
         ", got ", actual_version)
  }
  built <- if (is.null(description$Built) || is.na(description$Built)) "" else description$Built
  fields <- c("installed_package", profile, expected_version, actual_version, built,
              R.version$version.string, R.version$platform, Sys.info()[["machine"]],
              Sys.getlocale("LC_CTYPE"), package_path)
  cat(paste(fields, collapse = "\t"), "\n", sep = "")
}
RS
