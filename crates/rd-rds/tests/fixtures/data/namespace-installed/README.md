# Installed namespace metadata fixture

`namespace-installed.rds` is copied from `Meta/nsInfo.rds` after installing
the minimal package in
[`namespace-package`](../../namespace-package) with `R CMD INSTALL`. The
package's `NAMESPACE` includes a named export, an ordinary export, an
`import(..., except = ...)`, a mixed unaliased/aliased `importFrom`, and an S3
declaration.
The checked-in artifact was generated with R 4.6.1; it is not copied from an
installed system package or another repository.

Regenerate it with:

```text
Rscript crates/rd-rds/tests/fixtures/generate_installed_namespace_fixture.R \
  crates/rd-rds/tests/fixtures/data/namespace-installed
```
