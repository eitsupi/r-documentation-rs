# Releasing

Releases are performed from CI. Pushing a version tag runs the [Publish to crates.io workflow](.github/workflows/publish-crate.yml), which publishes all workspace crates in lockstep with `cargo publish --workspace` via crates.io Trusted Publishing; `cargo` resolves the inter-crate publish order itself. Do not run `cargo publish` locally.

## Preconditions

- [ ] Confirm the Check workflow is green on the release commit.
- [ ] Confirm the most recent R Corpus run succeeded; that workflow is weekly/manual and is not a PR gate.
- [ ] Update `CHANGELOG.md`: date the release entry and leave `Unreleased` empty.
- [ ] Bump the workspace version in lockstep.
- [ ] When changing the release series, grep tracked Markdown for the previous series string and update every remaining prose reference.

## Pre-tag verification

- [ ] Run `cargo deny check` and the repo-wide fixture scan (`.github/scripts/scan_fixture_text.sh`).
- [ ] For each crate, run `cargo package --list -p <crate>` and confirm the archive contents include `LICENSE`, `README.md`, and every fixture required by its tests; published versions cannot be deleted.
- [ ] Run `cargo package --workspace --locked` on the clean release commit; it packages and verifies the crates in dependency order.
- [ ] Review `cargo package --list -p <crate>` for each crate: no unexpected files; the README and the fixtures its tests need are present; explain any file over 100 KiB.
- [ ] Record the compressed and unpacked sizes of each `.crate`.
- [ ] Never use `--allow-dirty` or `--no-verify`.

## Publish

- [ ] Push the version tag (`v<version>`, e.g. `v0.1.0`) to run the publish workflow.
- [ ] Verify the docs.rs builds and the crates.io README rendering afterwards; crate READMEs link the stability policy by absolute URL for this reason.
