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
- [The milestone catalog](../milestones/README.md) links each scope and its completion gates;
  [publication records](../releases/README.md) own frozen release evidence; [the handoff](../TODO.md)
  owns only the next concrete work and current blockers.

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

Run the complete gate from the repository root:

```sh
development/verification/verify-compiler.sh
```

The gate shares one temporary external Cargo target across formatting, warnings-denied Clippy,
workspace tests, no-default-features checking, and Rust documentation, then removes it. Complete
verification therefore cannot accumulate obsolete hash generations in the workspace.

Focused Cargo commands may use `development/compiler/target/` for a fast inner loop. The directory
is ignored by Git and is a disposable cache, not repository state. Development builds retain line
tables, test executables omit debug payload, and both profiles disable incremental object graphs.
Reclaim stale local generations without touching source or release artifacts with:

```sh
cargo clean --manifest-path development/compiler/Cargo.toml
```

The [development verification contract](../verification/README.md) owns this cache lifecycle.
Release qualification independently uses fresh external targets and removes them after verification
so clean-build evidence does not become a persistent workspace cache.
