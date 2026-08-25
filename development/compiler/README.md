# Nocter Compiler Workspace

This directory contains the active Rust workspace for the Nocter compiler, language server, native
backend, package tooling, and conformance suite.

## Authorities

This file is only the workspace entry point. It does not duplicate language rules, compiler-stage
contracts, or milestone status.

- [`spec/`](../../spec/README.md) is the sole authority for public language, standard-library, CLI,
  diagnostic, and editor behavior.
- [Compiler architecture](../docs/architecture.md) owns the pipeline, dependency direction, and
  cross-stage authority rules.
- [Checked program design](../docs/checked-program-design.md),
  [target and executable program design](../docs/target-program-design.md), and
  [machine program design](../docs/machine-program-design.md) own their detailed stage contracts.
- [Semantic presentation design](../docs/semantic-presentation-design.md) owns the compiler-to-editor
  presentation boundary.
- [`Cargo.toml`](Cargo.toml) is the canonical workspace-member list. Crate manifests and public Rust
  APIs are the canonical dependency and implementation surface.
- [The active milestone](../milestones/v0.17.0.md) owns the completed v0.17.0 Phase 0 scope;
  [v0.17.0 release preparation](../milestones/v0.17.0-release-preparation.md) owns the active
  candidate qualification; [the handoff](../TODO.md) owns only the next concrete work
  and current blockers.

The compiler derives behavior from the current specification. The implementation removed before
v0.14.0 remains available through Git history only and is not a behavioral or architectural input.

## Dependency Direction

```text
source and discovery
  -> syntax and declarations
  -> checking
  -> target validation and executable specialization
  -> MIR
  -> machine program
  -> ARM64 and Mach-O emission
```

Source projection, diagnostics, editor analysis, command composition, and package acquisition are
side authorities with explicit inputs. They cannot create a second semantic pipeline or let a later
stage reinterpret an earlier stage's private representation. The architecture document owns the
complete boundary rules.

## Verification

Run from the repository root:

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
```

Cargo writes incremental build artifacts to `development/compiler/target/`. The directory is
ignored by Git and is a disposable cache, not repository state. Reclaim it without touching source
or release artifacts with:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml
```

Release qualification uses a fresh external `CARGO_TARGET_DIR` and removes it after verification so
clean-build evidence does not become a second persistent workspace cache.
