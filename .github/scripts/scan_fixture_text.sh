#!/usr/bin/env bash
# Scan tracked R binary fixtures for text copied from GPL-licensed R sources.
#
# .rds compression is identified from its magic bytes and decoded with the
# corresponding command-line tool. If zstd is unavailable, the local
# development container's system libzstd is used through Python ctypes
# (zstd_decompress.py). .rdbentry files have a four-byte size prefix followed
# by a raw zlib stream; Python 3's standard-library zlib decoder
# (zlib_decompress.py) is used because it is available on ubuntu-latest and in
# the local development container without requiring an extra decompression
# utility.
#
# For an isolated test copy, set SCAN_FIXTURE_FILES to a newline-separated list
# of files. Normal invocations leave it unset and enumerate tracked fixtures
# with git ls-files.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

forbidden_patterns=(
  'Rd/macros/system.Rd'
  '\newcommand'
  'Rd_expr_doi'
  'CRAN.R-project.org/package='
  'ifelse{latex}'
  '/opt/R/'
  'R.home'
  'Ripley'
  'Venables'
)

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

decompressed="$work_dir/decompressed"

decode_rds() {
  local file=$1
  local magic
  magic=$(od -An -tx1 -N4 "$file" | tr -d ' \n')

  case "$magic" in
    1f8b*) gzip -dc -- "$file" >"$decompressed" || return 1 ;;
    fd377a58*) xz -dc -- "$file" >"$decompressed" || return 1 ;;
    425a68*) bzip2 -dc -- "$file" >"$decompressed" || return 1 ;;
    28b52ffd*)
      if command -v zstd >/dev/null 2>&1; then
        zstd -dc -- "$file" >"$decompressed" || return 1
      else
        python3 "$script_dir/zstd_decompress.py" "$file" >"$decompressed" || return 1
      fi
      ;;
    *) cat -- "$file" >"$decompressed" || return 1 ;;
  esac
}

decode_rdbentry() {
  local file=$1
  tail -c +5 -- "$file" | python3 "$script_dir/zlib_decompress.py" \
    >"$decompressed" || return 1
}

scan_file() {
  local file=$1
  local pattern

  case "$file" in
    *.rdbentry) decode_rdbentry "$file" || return 1 ;;
    *.rds) decode_rds "$file" || return 1 ;;
    *)
      echo "unexpected fixture extension: $file" >&2
      return 2
      ;;
  esac

  for pattern in "${forbidden_patterns[@]}"; do
    if grep -aFq -- "$pattern" "$decompressed"; then
      printf 'forbidden pattern %s in %s\n' "$pattern" "$file"
      return 1
    fi
  done
}

fixture_files=()
if [[ -n "${SCAN_FIXTURE_FILES:-}" ]]; then
  while IFS= read -r file; do
    [[ -n "$file" ]] && fixture_files+=("$file")
  done <<<"$SCAN_FIXTURE_FILES"
else
  while IFS= read -r -d '' file; do
    fixture_files+=("$file")
  done < <(git ls-files -z -- '*.rds' '*.rdbentry')
fi

failed=0
for file in "${fixture_files[@]}"; do
  if ! scan_file "$file"; then
    failed=1
  fi
done

if (( failed )); then
  exit 1
fi

printf 'OK: scanned %d tracked binary fixtures; no forbidden text found.\n' "${#fixture_files[@]}"
