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
- Each workspace crate's `README.md` owns that crate's responsibility, input/output contract,
  internal responsibility split, and local invariants. It may name another crate only through that
  crate's exported contract.
- [The latest completed implementation milestone](../milestones/v0.24.0.md) owns its scope and
  completion gates; [the latest publication record](../releases/v0.22.0.md) owns frozen release
  evidence; [the
  handoff](../TODO.md) owns only the next concrete work and current blockers.

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

## Crate Documentation

Every workspace member has a colocated `README.md`. Start with
[`crates/nocter-session/README.md`](crates/nocter-session/README.md) for compiler orchestration,
[`crates/nocter-checking/README.md`](crates/nocter-checking/README.md) for semantic checking,
[`crates/nocter-analysis/README.md`](crates/nocter-analysis/README.md) for editor queries, or
[`crates/nocter-machine/README.md`](crates/nocter-machine/README.md) for native lowering. The
workspace manifest, rather than a copied crate list, remains the membership authority.

## Verification

Run from the repository root:

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
```

Cargo writes build artifacts to `development/compiler/target/`. The directory is ignored by Git and
is a disposable cache, not repository state. Workspace-owned development and test profiles retain
line-table debugging while disabling incremental object graphs, which removes the largest
edit-history-dependent part of the many-crate cache. Reclaim stale build generations without
touching source or release artifacts with:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml
```

Release qualification uses a fresh external `CARGO_TARGET_DIR` and removes it after verification so
clean-build evidence does not become a second persistent workspace cache.
