#!/usr/bin/env bash
# Prepare the pinned package library used by the installed-code corpus jobs.
# The downloaded archives are temporary CI inputs; only their provenance is
# retained in the uploaded report.
set -euo pipefail

if [[ "$#" -ne 4 ]]; then
  echo "usage: $0 PROFILE LIBRARY MANIFEST PROVENANCE" >&2
  exit 2
fi

profile="$1"
library="$2"
manifest="$3"
provenance="$4"
artifact_dir="${RUNNER_TEMP:-/tmp}/installed-code-corpus-artifacts-${profile}"
mkdir -p "$library" "$artifact_dir"

case "$profile" in
  r-4.6.1|r-4.5.3) require_fixed_checksums=true ;;
  r-devel) require_fixed_checksums=false ;;
  *) echo "unknown corpus profile: $profile" >&2; exit 2 ;;
esac

printf 'profile\tpackage\tpackage_version\tsource\tsnapshot\tr_version\tos\tarch\tlocale\tartifact_url\tartifact_sha256\tacquired_sha256\tinstalled_path\n' > "$provenance"

# Validate the complete manifest before any dependency installation or network
# access. This keeps an unmeasured blocking profile from doing partial work.
declare -A manifest_packages
while IFS=$'\t' read -r row_profile package version source snapshot r_version os arch locale url expected; do
  [[ -z "$row_profile" || "$row_profile" == \#* || "$row_profile" != "$profile" ]] && continue
  [[ -z "${manifest_packages[$package]+present}" ]] || {
    echo "manifest repeats package $package for $profile" >&2
    exit 1
  }
  manifest_packages[$package]=1
  if [[ "$require_fixed_checksums" == true && "$expected" == "record-at-run" ]]; then
    echo "blocking profile $profile has an unfixed artifact checksum for $package" >&2
    exit 1
  fi
  if [[ "$expected" == "UNVERIFIED" ]]; then
    echo "artifact checksum for $profile/$package has not been measured; refusing to run" >&2
    exit 1
  fi
  if [[ "$source" == CRAN-archive && "$url" != https://cloud.r-project.org/src/contrib/Archive/* ]]; then
    echo "fixed CRAN package $package must use a non-disappearing Archive URL" >&2
    exit 1
  fi
  if [[ "$expected" != record-at-run && ! "$expected" =~ ^[0-9a-f]{64}$ ]]; then
    echo "invalid artifact checksum for $package: $expected" >&2
    exit 1
  fi
done < "$manifest"
for package in base stats MASS Matrix dplyr rlang R6; do
  [[ -n "${manifest_packages[$package]+present}" ]] || {
    echo "manifest is missing $package for $profile" >&2
    exit 1
  }
done

download() {
  local url="$1"
  local destination="$2"
  curl --fail --location --retry 3 --silent --show-error \
    --output "$destination" "$url" \
  || curl --fail --location --retry 3 --silent --show-error \
    --output "$destination" "${url/cloud.r-project.org/cran.r-project.org}"
}

Rscript --vanilla - "$library" <<'RS'
args <- commandArgs(trailingOnly = TRUE)
library_dir <- args[[1L]]
options(repos = c(CRAN = "https://packagemanager.posit.co/cran/2026-09-01"))
.libPaths(c(library_dir, .libPaths()))

# Install dependencies from the pinned snapshot before the target archives
# are installed by the shell loop. The target archives are installed again
# below, so their versions remain controlled by the manifest.
install.packages(c("MASS", "Matrix", "dplyr", "rlang", "R6"),
                 lib = library_dir,
                 dependencies = c("Depends", "Imports", "LinkingTo"), quiet = TRUE)
RS

declare -A artifacts
declare -A seen_packages
while IFS=$'\t' read -r row_profile package version source snapshot r_version os arch locale url expected; do
  [[ -z "$row_profile" || "$row_profile" == \#* ]] && continue
  [[ "$row_profile" == "$profile" ]] || continue
  [[ -z "${seen_packages[$package]+present}" ]] || {
    echo "manifest repeats package $package for $profile" >&2
    exit 1
  }
  seen_packages[$package]=1
  if [[ "$require_fixed_checksums" == true && "$expected" == "record-at-run" ]]; then
    echo "blocking profile $profile has an unfixed artifact checksum for $package" >&2
    exit 1
  fi
  if [[ "$expected" == "UNVERIFIED" ]]; then
    echo "artifact checksum for $profile/$package has not been measured; refusing to run" >&2
    exit 1
  fi
  if [[ "$source" == CRAN-archive && "$url" != https://cloud.r-project.org/src/contrib/Archive/* ]]; then
    echo "fixed CRAN package $package must use a non-disappearing Archive URL" >&2
    exit 1
  fi
  if [[ "$expected" != record-at-run && ! "$expected" =~ ^[0-9a-f]{64}$ ]]; then
    echo "invalid artifact checksum for $package: $expected" >&2
    exit 1
  fi
  key="$url"
  if [[ -z "${artifacts[$key]+present}" ]]; then
    archive="$artifact_dir/$(basename "$url")"
    download "$url" "$archive"
    actual="$(sha256sum "$archive" | awk '{print $1}')"
    if [[ "$expected" != record-at-run ]]; then
      [[ "$actual" == "$expected" ]] || {
        echo "checksum mismatch for $url: expected $expected, got $actual" >&2
        exit 1
      }
    fi
    artifacts[$key]="$archive|$actual"
  fi
  archive="${artifacts[$key]%%|*}"
  actual="${artifacts[$key]##*|}"
  installed_path="$library/$package"
  if [[ "$source" == R-source ]]; then
    # Base and stats are supplied by setup-r. The source archive is still
    # acquired and checksummed so the R provenance is reproducible.
    system_path="$(Rscript --vanilla -e 'cat(system.file(package=commandArgs(TRUE)[[1]]))' "$package")"
    [[ -d "$system_path" ]] || {
      echo "R installation has no package $package" >&2
      exit 1
    }
    if [[ ! -e "$installed_path" ]]; then
      ln -s "$system_path" "$installed_path"
    fi
  else
    R CMD INSTALL --no-multiarch --no-test-load \
      -l "$library" "$archive" >/dev/null
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$row_profile" "$package" "$version" "$source" "$snapshot" "$r_version" \
    "$os" "$arch" "$locale" "$url" "$expected" "$actual" "$installed_path" >> "$provenance"
done < "$manifest"

for package in base stats MASS Matrix dplyr rlang R6; do
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
