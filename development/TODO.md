# Nocter Development Handoff

## Current Task

v0.6.0 Phase 2 is complete. The candidate is qualified locally; publication requires a separate
explicit user authorization.

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
- callable source anchors, normalized declarations, and bounded semantic-detail projection now form
  one `analysis/presentation` boundary shared by hover, completion, signature help, and inlay hints
- result provenance no longer exposes private aggregate layout or copy-only dataflow, semantic hints
  attach after the complete callable signature, and normalized recovery never slices a signature
  from raw source
- the v0.6.0 Phase 0 full verification record is owned by `milestones/v0.6.0.md`
- result allocation provenance, external result provenance, and the execution allocation
  requirement have separate compiler owners
- contextual `alloc` is parsed, formatted, serialized, resolved, typechecked, presented, and
  source-edited for every supported callable declaration and structural callable type
- `from current`, inferred allocation-effect signature text, and private aggregate provenance prose
  have been removed from current source and editor contracts
- standard `String`, `Vec<T>`, iterator, process, I/O, typed-literal, and allocator APIs carry
  explicit result contracts without blanket modifiers
- type-directed contract projection excludes scalar and discarded local allocation facts while
  retained-input mutation summaries preserve allocator and lexical-region origins through wrappers
- neutral empty buffers select an allocation domain only on first growth; return checking and
  summary inference share the same mutation effects
- the v0.6.0 Phase 1 completion record is owned by `milestones/v0.6.0.md`
- recursive allocation summaries start from body evidence rather than written contracts
- E0462 retains a deterministic returned-expression witness while edits remain declaration-owned
- closure callable bounds enforce allocation variance across methods and imported modules
- every distributed standard-library module participates in one automated contract audit
- generic type parameters retain storage capability, protocol `for` bindings instantiate resolved
  conversion and step provenance, and ownership-transfer primitives preserve element origins
- the exact v0.6.0 candidate archive and Phase 2 verification record are owned by
  `milestones/v0.6.0.md`

## Next Work

Do not publish, tag, or rewrite the v0.5.0 release records without explicit authorization. The next
development milestone should be planned independently after publication status is decided; do not
reopen the completed v0.6.0 contract boundary for unrelated syntax or execution effects.
