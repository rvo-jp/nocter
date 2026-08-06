# Nocter v0.5.0 Handoff

## Baseline

- branch: `develop`
- released baseline: v0.4.0
- completed milestone: v0.5.0 Phase 2 native test declarations and assertions
- active milestone: v0.5.0 Phase 3 package-wide editor index and refactoring
- target: `arm64-darwin`

The normative plan is [v0.5.0.md](docs/v0.5.0.md). Package/compiler boundaries are defined in
[packages.md](docs/packages.md). Public behavior belongs in `spec/`.

## Implemented Scope

- composite `nocter.nct` package file with separate manifest and root-module AST responsibilities
- removal of package responsibility from `index.nct`
- typed package-root and path module identities; omitted executable `entry` selects `nocter.nct`,
  while explicit `entry: "."` selects `index.nct`
- removal of source-root and entry-file-parent import discovery
- module-relative, package-absolute, dependency, and standard-library import namespaces
- `#dependencies` for path, Git revision, and archive sources
- generated format-1 `#lock` with exact Git commits and SHA-256 archive identities
- digest-backed `PackageId`, scoped dependency aliases, graph cycles, and transitive loading
- package-local and exact-identity Nocter-home stores
- isolated Git/archive fetcher with archive and canonical-path safety validation
- `nocter fetch`, `--locked`, and `--offline`
- CLI dependency compilation and locked-offline LSP graph loading
- nearest-package LSP ownership, nested package navigation, manifest semantic ranges, and dependency
  import completion
- package-aware formatter and JSON AST
- immutable generation-numbered LSP snapshots shared by diagnostics and semantic requests
- locked-offline package graphs reused per package, including read-only unsaved `nocter.nct`
  overlays
- source-dependency invalidation, reverse-import rebuilding, unrelated-analysis reuse, watched-file
  updates, and nested-package isolation
- versioned diagnostics and semantic-token generation identifiers
- failure-stable frontend dependency traces for missing, malformed, deleted, and symlinked imports
- failure-stable transitive package-manifest traces and deterministic graph recovery
- dynamic `.nct` file-watcher registration, saved-text synchronization, and an explicit LSP
  lifecycle state machine
- typed `#test` package targets with exact entry modules and separate executable/test namespaces
- isolated `nocter test` execution with declaration-order continuation and stable human/JSON reports
- package test formatter, JSON AST, semantic ranges, manifest completion, and definition navigation
- native `test name { ... }` declarations with fixed `void!` contracts and module-local uniqueness
- source-backed test discovery, `--case`, per-declaration compiler entry identities, and isolated
  process execution without source rewriting or synthetic `main`
- ordinary `std/testing` assertions, native test LSP presentation, and same-module/private versus
  separate-module/public visibility coverage

## Qualification State

Phase 1 passes path, Git, archive, generated-lock transaction, offline reuse, cache-miss,
source/lock mismatch, cycle, package compilation, LSP, formatter, full repository, optimized
distribution, installed-home, native packaged-home, and archive acceptance gates. Phase 2 passes
focused snapshot, LSP, package, compiler-library, formatter, warnings-denied Clippy, full repository,
optimized distribution, installed-home `doctor`, and packaged LSP acceptance gates. The subsequent
stabilization audit adds malformed root/transitive manifest recovery, missing-import creation,
symlink deletion, dynamic watcher registration, `didSave`, lifecycle, and a deterministic
49-document invalidation partition. The complete verification script, public documentation build,
optimized package build, installed-home `doctor`, archive inspection, and packaged LSP acceptance
all pass after the audit.

## Publication State

v0.4.0 is published at <https://github.com/rvo-jp/nocter/releases/tag/v0.4.0>. The annotated tag
targets qualification commit `d878ff69`; `main` contains the identical tree through release merge
`aa1d8c60`. The uploaded `nocter-v0.4.0-arm64-darwin.tar.gz` digest matches the qualified local
artifact.

## Next Work

Begin Phase 3 with one generation-numbered package-wide declaration index built from the existing
locked package graph and immutable LSP snapshot. Define source-backed cross-module reference and
rename plans before adding edit-producing requests; do not introduce ambient directory scanning or
a mutable editor-only symbol cache.
