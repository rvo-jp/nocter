# Nocter v0.4.0 Handoff

## Baseline

- branch: `develop`
- release candidate: v0.4.0
- completed milestone: v0.4.0 release-readiness implementation and documentation
- qualification: complete for the exact v0.4.0 distribution candidate
- next milestone: publication after explicit user authorization
- target: `arm64-darwin`

The normative plan is [v0.4.0.md](docs/v0.4.0.md). Package/compiler boundaries are defined in
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

## Next Work

Publish the already-qualified v0.4.0 candidate only after explicit user authorization. Preserve
the frozen feature scope and exact release identity; do not add a Phase 3 feature or rebuild the
archive as part of publication unless an external release check finds a blocker.
