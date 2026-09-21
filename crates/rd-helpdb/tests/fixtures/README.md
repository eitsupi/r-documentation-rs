# Help database fixtures

The `data/help_topics*` fixtures are independently authored inputs generated
with R 4.6.1 by
[`tests/fixtures/generate_help_topic_metadata.R`](../../../../../tests/fixtures/generate_help_topic_metadata.R).
Run it from the repository root:

```sh
Rscript tests/fixtures/generate_help_topic_metadata.R
```

The script writes directly to `crates/rd-helpdb/tests/fixtures/data/`. Only
`rd-helpdb` consumes these fixtures, and keeping them in the crate lets the
published crate's tests work independently of the workspace. The metadata
fixtures use serialization versions 2 and 3, gzip compression, and ASCII
text. They cover reordered columns, duplicate aliases within and across
rows, NA values, empty alias groups, missing optional columns, and zero
rows. Tests build malformed schemas from the decoded objects.

The raw-record `help_topics.rdx`/`.rdb` pair contains the repository's
independently authored `minimal.Rd` topic under `first-topic`. The generator
disables system macros and removes source references to avoid embedding
external source text, paths, or timestamps. Consumer tests corrupt a temporary
copy of the record to test title fallback after a decoding failure.

Other top-level fixtures are copied from `tests/fixtures/data/` and generated
by `tests/fixtures/generate_fixtures.R`. See the
[lazy-load fixture README](data/lazyload/README.md) for the separate adapter
fixtures.
