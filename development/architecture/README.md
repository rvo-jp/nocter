# Compiler Architecture

This directory contains current contracts that span compiler crates, generated runtime services,
target adapters, or standard-library implementation modules. It does not define public language,
platform, tooling, or standard-library behavior.

Public language, platform, CLI, diagnostic, and editor behavior belongs to
[`spec/`](../../spec/README.md). Exact standard-library declarations and their observable behavior
belong under [`std/`](../../std/README.md). A workspace crate's private mechanism belongs in that
crate's colocated `README.md`. Work order, rejected migrations, qualification evidence, and release
history belong under [`development/history/`](../history/README.md).

## Reading Order

1. [Architecture Overview](overview.md) for the program pipeline, side authorities, and dependency
   rules.
2. The relevant [workspace crate README](../compiler/README.md#crate-documentation) for one crate's
   exported contract and private responsibilities.
3. One cross-responsibility contract below when a decision crosses those local boundaries.
4. Historical milestones or reviews only when the reason or qualification evidence is needed.

## Pipeline

These documents own handoffs between semantic products and tooling projections:

- [Checked Program](pipeline/checked-program.md)
- [Declaration Diagnostics](pipeline/declaration-diagnostics.md)
- [Target, Executable, and MIR Programs](pipeline/target-program.md)
- [Machine Program and Native Target](pipeline/machine-program.md)
- [Incremental Computation](pipeline/incremental-computation.md)
- [Semantic Presentation](pipeline/semantic-presentation.md)

## Semantic Contracts

These documents own compiler-wide relations that more than one pipeline stage consumes:

- [Execution Contracts](contracts/execution.md)
- [Value Provenance](contracts/value-provenance.md)
- [Compile-Time Callables](contracts/compile-time-callables.md)
- [Erased Callables](contracts/erased-callables.md)

## Representations

These documents own one representation as it travels from syntax or checked semantics to stored
layout and native execution:

- [Tuples](representations/tuples.md)
- [Floating-Point Values](representations/floating-point.md)
- [Unicode Scalars](representations/unicode-scalars.md)
- [Static Unicode Data](representations/static-unicode-data.md)

## Runtime Integration

These documents own boundaries that cross standard source, compiler products, runtime services,
target adapters, and resource lifecycles:

- [Asynchronous Computation](runtime/asynchronous-computation.md)
- [Blocking Jobs](runtime/blocking-jobs.md)
- [Asynchronous Byte Streams](runtime/byte-streams.md)
- [Filesystem and File I/O](runtime/filesystem.md)
- [Processes](runtime/processes.md)
- [Networking](runtime/networking.md)
- [HTTP Client](runtime/http-client.md)
- [Secure Transport](runtime/secure-transport.md)
- [HTTP Service](runtime/http-service.md)

## Standard-Library Implementation

These are internal implementation boundaries, not public API guides:

- [Source Layout](standard-library/source-layout.md)
- [Associative Collections](standard-library/associative-collections.md)
- [JSON](standard-library/json.md)

Grammar-derived test coverage belongs to
[`development/compiler/tests/grammar.md`](../compiler/tests/grammar.md). Performance methodology
belongs to [`development/benchmarks/`](../benchmarks/README.md). Repository maintenance workflow
belongs to [`development/README.md`](../README.md).

## Information Ownership

| Information | Sole owner |
|---|---|
| Public language, platform, CLI, diagnostic, and editor behavior | `spec/` |
| Exact standard-library declarations and declaration documentation | each `std/**/index.nct` |
| Longer observable standard-library behavior | the guide assigned by `std/README.md` |
| Compiler-wide dependency direction and cross-responsibility contracts | this directory |
| One crate's inputs, outputs, private split, and local invariants | that crate's `README.md` |
| Exact Rust APIs and dependencies | source, rustdoc, and Cargo manifests |
| Current work and blockers | `development/TODO.md` |
| Work order, design findings, and qualification evidence | `development/history/` |

Architecture documents state current authorities, information flow, lifecycle, and invariants.
They do not retain delivery phases, completed gates, or superseded alternatives as current design.
