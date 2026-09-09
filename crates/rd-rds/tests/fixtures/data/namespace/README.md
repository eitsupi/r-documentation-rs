# Namespace metadata fixtures

`namespace-v2.rds` and `namespace-v3.rds` are deterministic `Meta/nsInfo.rds`
shapes generated from hand-authored declarations by
[`generate_namespace_metadata_fixture.R`](../../generate_namespace_metadata_fixture.R)
with R 4.6.1. They do not come from an installed package or another
repository. Regenerate them with:

```text
Rscript crates/rd-rds/tests/fixtures/generate_namespace_metadata_fixture.R \
  crates/rd-rds/tests/fixtures/data/namespace
```

The fixture contains duplicate declarations, an all-package import, an
aliased `importFrom`, an `except` import, four-column S3 registrations, S4
declarations, empty export patterns, and an unknown field.
