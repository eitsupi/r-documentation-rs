# Installed lazy-load fixture

`lazyfixture.rdx` and `lazyfixture.rdb` are copied from a package installed
with `R CMD INSTALL` from the fixture source directory
`crates/rd-rds/tests/fixtures/lazyload-package`. The checked-in files were
generated with R 4.6.1 in the development environment; output can vary across
R versions and platforms. Regenerate them with:

```text
Rscript crates/rd-rds/tests/fixtures/generate_installed_lazyload_fixture.R \
  crates/rd-rds/tests/fixtures/data/lazyload/installed
```

The generated pair uses the normal installed-package profile: a gzip-enveloped
`.rdx` index and a zlib `.rdb` record database. The smoke test opens the
committed pair directly and reads and parses a known binding.
