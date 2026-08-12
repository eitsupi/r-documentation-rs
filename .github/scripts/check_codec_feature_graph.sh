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
  for codec in lzma-rust2 bzip2 ruzstd; do
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

all_features_output="$work_dir/rd-rds-all-features.tree"
if ! cargo tree -p rd-rds --all-features -e normal,build --locked >"$all_features_output"; then
  printf 'FAIL: cargo tree could not inspect rd-rds with all features\n' >&2
  failed=1
elif grep -Eq '(^|[[:space:]])lzma-sys v' "$all_features_output"; then
  printf 'FAIL: rd-rds all-features graph contains forbidden native dependency lzma-sys\n' >&2
  failed=1
else
  printf 'OK: rd-rds all-features graph contains no lzma-sys.\n'
fi

if (( failed )); then
  exit 1
fi
