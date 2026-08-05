# Nocter v0.4.0 Handoff

## Baseline

- branch: `develop`
- released baseline: v0.3.0
- completed milestone: v0.4.0 Phase 1 — Deterministic Package Graph
- next milestone: v0.4.0 Phase 2 — Immutable Package-wide LSP Snapshot
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

## Qualification State

Phase 1 passes path, Git, archive, generated-lock transaction, offline reuse, cache-miss,
source/lock mismatch, cycle, package compilation, LSP, formatter, full repository, optimized
distribution, installed-home, native packaged-home, and archive acceptance gates.

## Next Work

Phase 2 should replace per-request LSP package reconstruction with one immutable package snapshot
shared by diagnostics, hover, completion, definition, and references. Snapshot invalidation must
use package/module identity and open-document versions; it must not add network or manifest writes
to LSP request handling.
