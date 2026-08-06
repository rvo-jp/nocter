# Nocter Development Handoff

## Current Task

The example ownership migration is complete. Runnable user packages live under the repository-root
`examples/`; compiler-only valid and invalid inputs live under
`compiler/tests/fixtures/source_corpus/`.

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

## Next Work

No example-migration task remains. Select the next compiler or standard-library objective from the
qualified candidate record; do not publish, tag, or merge as part of this handoff.
