#!/usr/bin/env bash
# Assert that gzip-only consumer configurations do not pull other codecs.

set -euo pipefail

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

failed=0

check_graph() {
  local crate=$1
  shift
  local output="$work_dir/$crate.tree"
  local bad=0

  printf 'Checking %s gzip-only production graph\n' "$crate"
  if ! cargo tree -p "$crate" "$@" -e normal,build --locked >"$output"; then
    printf 'FAIL: cargo tree could not inspect %s\n' "$crate" >&2
    failed=1
    return
  fi

  if ! grep -Eq '(^|[[:space:]])flate2 v' "$output"; then
    printf 'FAIL: %s gzip-only graph is missing flate2\n' "$crate" >&2
    bad=1
  fi

  local codec
  for codec in xz2 lzma-sys bzip2 ruzstd; do
    if grep -Eq "(^|[[:space:]])${codec} v" "$output"; then
      printf 'FAIL: %s gzip-only graph contains unwanted codec %s\n' \
        "$crate" "$codec" >&2
      bad=1
    fi
  done

  if (( bad )); then
    failed=1
  else
    printf 'OK: %s contains flate2 and no other codec.\n' "$crate"
  fi
}

check_graph rd-helpdb --no-default-features --features gzip
check_graph rd-ast --no-default-features --features rds,gzip

if (( failed )); then
  exit 1
fi
