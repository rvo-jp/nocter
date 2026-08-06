# Nocter Development Handoff

## Current Task

The v0.5.0 release candidate is fully qualified and awaits explicit publication authorization. Do
not add features, rebuild or replace the qualified archive, merge to `main`, create a tag, push, or
publish a GitHub release before that authorization.

## Completed Checkpoint

- public release notes moved to repository-root `releases/`
- published-release qualification moved to `development/releases/`
- the active candidate record moved to `development/milestones/`
- specification indexes no longer delegate normative behavior to development records
- developer website instructions moved out of generated `docs/` Markdown
- standard-library and LSP development documents now describe implementation architecture rather
  than public API inventories
- current specification chapters describe one unqualified candidate contract instead of mixing
  released-version and phase-specific APIs
- lexical keywords, construction examples, allocator use, iteration, and primitive-boundary text
  have been reconciled with the current compiler and distributed standard library
- the root `example.nct` and mixed-audience `spec/examples/` tree have been removed
- `examples/hello` and `examples/file-summary` are formatter-checked and verified through the
  distributed standard-library boundary
- source-corpus fixtures and public package verification now have separate integration tests
- the documentation generator publishes Markdown and Nocter source as one collision-checked site,
  with public examples reachable through website navigation
- the complete v0.5.0 verification suite, fresh installed-home authoring workflow, public examples,
  packaged LSP lifecycle, archive structure, and release identity pass final qualification
- the exact upload candidate is `dist/nocter-v0.5.0-arm64-darwin.tar.gz` (3,254,546 bytes), with
  SHA-256 `61560090d1be6a802900e254c9666d4b60be9623257f1a62b87c1532b0636aa1`

## Next Work

After explicit publication authorization, publish the already qualified v0.5.0 candidate. Preserve
the exact archive above, merge `develop` to `main`, create and push `v0.5.0`, attach the archive to
the GitHub release using the English public notes, then download the published asset and repeat the
digest and fresh-install audit. Only after that audit should documentation identify v0.5.0 as the
latest published release and the candidate record move to `development/releases/v0.5.0.md`.
