# Nocter Compiler Architecture

This document defines stable responsibility boundaries in the Rust bootstrap compiler. Public
language behavior belongs in the [language specification](../../spec/README.md); milestone scope
and qualification belong in [development milestones](../milestones/README.md).

## Pipeline

```text
.nct source
  -> SourceMap
  -> lexer / parser
  -> semantic definition index / resolution
  -> checker-owned typed HIR
  -> checked control-flow MIR
  -> machine IR
  -> ABI classification
  -> ARM64 code generation
  -> Mach-O image
```

Normal user builds do not require LLVM, `clang`, `as`, `ld`, the Xcode Command Line Tools, or an
external runtime library. The released and active-development native target is `arm64-darwin`.

## Responsibility Boundaries

| Area | Owns |
|---|---|
| `source` | canonical file identity, byte spans, line mapping |
| `lexer` | tokens and lexical diagnostics |
| `parser` | authored AST, syntax recovery, removed-syntax diagnostics |
| `ast` | syntax data, AST JSON, documentation extraction |
| `semantic` | compile-unit definition, body, expression, and type identities |
| `frontend` | package loading, prelude, compile-unit orchestration |
| `resolve` | imports, visibility, scopes, symbols, declaration identity |
| `typecheck` | types, specialization, ownership, provenance, regions, and execution plans |
| `analysis` | retained compiler and editor projections |
| `mir` | executable control flow, places, values, calls, loans, initialization, and cleanup |
| `driver/buildability` | reachable MIR construction and validation diagnostics |
| `ir` | projection of checked MIR into explicit machine-independent operations |
| `abi` | data layout and argument/return classification |
| `backend` | IR validation, ARM64 emission, Mach-O output |
| `target` | machine encoding and target-specific output details |
| `driver` | CLI and LSP transport orchestration |

Later stages consume earlier facts. They do not repeat selection, type checking, ownership, or
source-pattern classification. A new responsibility receives a focused module and narrow API; it
does not become an exception in an unrelated lowering path.

## Semantic Authority

Every accepted source form reaches one of two outcomes:

1. Resolution and type checking produce compiler-owned semantic facts, followed by valid MIR.
2. The frontend or MIR construction rejects it with a source-backed diagnostic.

There is no backend path that guesses missing semantics from raw AST. Spans locate source; they do
not identify declarations or decide semantic equality. Definitions, bodies, expressions, and
types use their distinct typed ID domains. See
[Semantic Identity and Typed Model](semantic-model.md).

AST remains the authored input to typed-HIR-to-MIR construction. Once a body is finalized, both
buildability and machine-IR projection consume the same cached MIR body. Machine projection never
inspects `Expr`, `Stmt`, or `Block` to recover execution meaning.

## Checked MIR Boundary

MIR is the single executable model for the native compiler. It contains:

- body-local identities for locals, blocks, scopes, projections, loans, regions, and drop plans;
- `TyId`, representation, ownership, storage role, lexical scope, and source origin for each local;
- typed places and projection paths for fields, indexes, dereferences, and logical error fields;
- explicit operands, rvalues, calls, outcome inspection, loops, branches, returns, and traps;
- semantic callable instances built from canonical `DefId`, receiver type, and type arguments;
- explicit begin/end loans, aggregate construction boundaries, region boundaries, moves, drops,
  and return preservation.

Construction uses one mutable context. Finalization then validates definite initialization,
materializes owned-value replacement, cleanup, and return-preservation edges, and validates the
completed graph. Only the finalized result enters the compile-unit cache.

Initialization, ownership obligations, and loans are separate path-sensitive dataflow domains over
the same CFG. A partial move invalidates only the selected projection. Cleanup drops a live root or
the maximal remaining live projections, in reverse lexical order, on every scope-exiting edge.
Borrow conflicts compare place overlap, so unrelated fields remain independent while ancestors,
descendants, and conservatively aliased indexes conflict.

Outcome values remain logical in MIR. Optional/fallible layer order, success payloads, and logical
error code/message operands are semantic data; byte offsets and register layouts are not. The ABI
and machine-IR projector derive physical layout after MIR validation.

Zero-argument static error helpers are also classified from checked MIR. Their native inline
projection accepts exactly one static `error` assignment to the return place followed by `Return`.
This is a MIR capability query, not an AST body-pattern exemption.

## Buildability

The checkable language may be wider than the native runtime subset. Buildability walks reachable
semantic call instances, constructs or reads their cached MIR bodies, and reports MIR construction
or validation failures at source locations. It may validate signature ABI support before body
construction, but it does not maintain statement or expression semantics in parallel with MIR.

Adding native support therefore requires the complete parser → resolver → typecheck → MIR → IR →
ABI → backend path. A construct is not buildable merely because an old AST lowerer can emit code;
the production compiler contains no such fallback route.

## Machine IR, ABI, and Backend

Machine IR is an explicit, target-independent storage and operation model projected from MIR.
Projection assigns ABI words, aggregate slots, offsets, direct or indirect passing, outcome layout,
and tail-call eligibility. It may use typed HIR and resolver records to project a retained `TyId`
into layout, but it may not revisit source expressions.

`abi` is the shared layout and calling-convention authority. Backend validation guards against
compiler drift or hand-built invalid IR; it is not a substitute for source diagnostics.
Target-specific syscalls, instruction encoding, and executable layout stay in `backend` and
`target`. Update [ABI and Layout](../../spec/09-abi-layout.md) whenever public ABI behavior changes.

## Allocator and Drop Boundary

`Allocator`, `String`, and `Vec` are ordinary standard-library surfaces. The compiler recognizes
only explicit intrinsic identities from the exact implicit toolchain standard-library package; a
public name alone never grants primitive authority.

Immutable per-type drop plans, mutable per-path drop obligations, and allocator provenance remain
separate. Allocation regions are MIR scope resources, and cleanup inserts their exits after owned
values on every leaving edge. See [Allocator and Ownership](allocator-ownership.md) and
[Region, Provenance, and Allocation Context](region-provenance.md).

## LSP Boundary

`driver/lsp` owns transport, document state, and protocol conversion. `analysis` retains semantic
occurrences, syntax cursor sites, and lexical scopes. Navigation and tokens select semantic
identities first; hover, completion, and signature help use shared normalized presentation rather
than source slicing.

Incomplete-source recovery is isolated. It may supply degraded syntax-only results when no
semantic identity exists, but it cannot override a successful semantic result. See [LSP](lsp.md).

## Diagnostics

- Malformed user source must not panic.
- Text diagnostics include file, line, column, snippet, primary marker, and actionable help when
  the compiler knows a remedy.
- JSON and LSP diagnostics retain stable machine-readable spans.
- Ordinary user diagnostics do not expose internal MIR or backend details.
- The same semantic error follows the same diagnostic path in check, build, run, and LSP.

## Verification Layers

| Layer | Proves |
|---|---|
| lexer/parser | syntax shape, recovery, removed syntax |
| resolver/typecheck | identity, types, generics, ownership, provenance, diagnostics |
| MIR | control flow, initialization, moves, loans, cleanup, call identity |
| machine IR | storage projection, explicit operations, ABI handoff |
| backend/target | frame/layout assumptions, encoding, emission |
| CLI build/run | user-visible native behavior |
| distributed home | packaged standard-library visibility and runtime behavior |
| analysis/LSP | agreement between compiler facts and protocol responses |

A user-visible promotion needs focused semantic or MIR coverage plus at least one CLI,
distributed-home, or LSP integration test. Exact temporary numbering and incidental instruction
shape are not contracts.
