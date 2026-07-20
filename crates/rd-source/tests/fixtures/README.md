# Fixture oracle policy

The committed oracle set is created or rewritten only by `generate.R --update`,
and every change is gated by `generate.R --check`. The script pins and enforces
R 4.6.1.

Changes that do not intend regeneration must leave every committed `.rds` byte-
identical; reviewers verify this with `git diff`. A version bump changes the
single version constant, regenerates the entire oracle set, and lands as an
isolated commit recording the old and new versions and the semantic differences.

The reproducibility invariant is: the same source, pinned R version, and either
synthetic source path produce the same sanitized object, the same bytes, and the
same committed oracle.

For mass-corpus findings, reproduce one file, manually minimize it to a
warning-free fragment, probe it with `Rscript`, add a fixture with its oracle
first, demonstrate the failing Rust test, fix the parser, then rerun the curated
and mass suites.
