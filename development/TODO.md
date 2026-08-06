# Nocter Development Handoff

## Current Task

Nocter v0.5.0 is published and its GitHub asset has passed the post-publication digest and
fresh-install audit. No release repair remains. The next task is to define one coherent v0.6.0
milestone before beginning feature work.

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
- `develop` and `main` contain the qualified tree, annotated tag `v0.5.0` identifies release merge
  `aae1f5bf9982637413ed31f436cb651c1c1a1301`, and the English GitHub Release is public
- the published asset reports the qualified size and digest on GitHub; a fresh download passes
  `doctor`, `init`, locked/offline check and test, run, graph, explicit build, and direct Mach-O
  execution without repository configuration

## Next Work

Define the v0.6.0 purpose, completion definition, explicit non-goals, and Phase 0 foundation in a
new `development/milestones/v0.6.0.md`. Keep v0.5.0 behavior and qualification frozen under
`development/releases/`; do not append new design work to that release record.
