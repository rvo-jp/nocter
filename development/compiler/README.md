# Nocter Compiler Workspace

This directory contains the active Rust workspace for the Nocter compiler, language server, native
backend, package tooling, and conformance suite.

## Authorities

This file is only the workspace entry point. It does not duplicate language rules, compiler-stage
contracts, or milestone status.

- [`spec/`](../../spec/README.md) owns published language, platform, CLI, diagnostic and editor behavior.
- [Development specifications](../spec/README.md) own adopted unreleased contracts. Their stated
  scope is authoritative for v2.0.0 work; absent productions have no released fallback.
- [`std`](../../std/README.md) owns compiler-checked standard-library declarations and
  declaration documentation, and assigns longer observable subjects to behavior guides.
- [Compiler architecture](../architecture/overview.md) owns the pipeline, dependency direction, and
  cross-stage authority rules.
- [Checked program design](../architecture/pipeline/checked-program.md),
  [target and executable program design](../architecture/pipeline/target-program.md), and
  [machine program design](../architecture/pipeline/machine-program.md) own their detailed stage contracts.
- [Execution contract design](../architecture/contracts/execution.md) owns the boundary between
  authored callable guarantees, checked execution facts, primitive facts, and their consumers.
- [Semantic presentation design](../architecture/pipeline/semantic-presentation.md) owns the compiler-to-editor
  presentation boundary.
- [`Cargo.toml`](Cargo.toml) is the canonical workspace-member list. Crate manifests and public Rust
  APIs are the canonical dependency and implementation surface.
- Each workspace crate's `README.md` owns that crate's responsibility, input/output contract,
  internal responsibility split, and local invariants. It may name another crate only through that
  crate's exported contract.
- [Current work](../TODO.md) owns next actions and blockers; [development design](../design/README.md)
  owns unresolved design and realization obligations.

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

Reusable source corpora are documented under [`tests/`](tests/README.md). Test assertions remain in
the consuming crate; that directory owns only physical non-Rust inputs shared across those tests.

## Verification

The [development verification contract](../verification/README.md) owns command invocation,
evidence selection, full-pipeline requirements and cache lifecycle. Select checks by the actual
changed behavior and consumers; source admission and native execution are separate claims.

Focused Cargo commands may use the ignored `development/compiler/target/` cache. Development
builds retain line tables, test executables omit debug payload, and both profiles disable
incremental object graphs. Release qualification independently uses fresh external targets;
current local changes do not replace a retained qualified artifact.
