# Nocter Compiler Architecture

This document defines stable responsibility boundaries in the Rust bootstrap compiler. See the
[specification](../../spec/README.md) for public language rules and the
[v0.4.0 release record](v0.4.0.md) for the released package and editor-snapshot boundaries.

## Pipeline

```text
.nct source
  -> SourceMap
  -> lexer / parser
  -> module loading / resolution
  -> type checking / ownership facts
  -> buildability preflight
  -> IR lowering
  -> ABI classification
  -> ARM64 code generation
  -> Mach-O image
```

Normal user builds do not require LLVM, `clang`, `as`, `ld`, the Xcode Command Line Tools, or an
external runtime library. The released and active-development native target is `arm64-darwin`.

## Phase Ownership

| Area | Owns |
|---|---|
| `source` | canonical file identity, byte spans, line mapping |
| `lexer` | tokens and lexical diagnostics |
| `parser` | AST construction, syntax recovery, removed-syntax diagnostics |
| `ast` | syntax data, AST JSON, documentation extraction |
| `frontend` | compile-unit loading, prelude, frontend orchestration |
| `resolve` | imports, visibility, scopes, symbols, declaration identity |
| `typecheck` | types, generic specialization, places, ownership, storage provenance, regions, allocation effects, drop semantics |
| `analysis` | owned editor/query results derived from compiler facts |
| `driver/buildability` | preflight rejection of checked forms not supported by the runtime |
| `ir` | conversion from typed facts to explicit lower-level operations |
| `abi` | data layout and argument/return classification |
| `backend` | IR validation, ARM64 emission, Mach-O output |
| `target` | machine encoding and target-specific output details |
| `diagnostics` | structured diagnostics and text/JSON rendering |
| `driver` | CLI, pipeline, and LSP protocol orchestration |

Later phases consume facts from earlier phases; they do not reimplement earlier decisions. When a
new responsibility does not fit an existing area, introduce a focused module and narrow API before
adding a broad helper.

## Compile-unit and Source Identity

- `SourceMap` owns source identity across the compiler.
- Every file in an import graph has one canonical identity.
- Diagnostics carry source-backed spans whenever a location is known.
- An LSP open document may overlay disk content without creating a second identity.
- Resolver and typechecker distinguish synthetic recovery nodes from real declarations.

## Semantic Boundary

Every form accepted by the parser must reach one of two outcomes:

1. Resolver and typechecker produce compiler-owned facts.
2. Parser, resolver, typechecker, or buildability rejects it with a source-backed diagnostic.

There is no third path in which the backend guesses missing language semantics from raw AST. Types,
symbols, ownership state, variants, and drop shapes required by lowering belong in resolver or
typechecker output.

## Buildability Boundary

The checkable language and native runtime subset are not identical. `driver/buildability` rejects
forms that are valid in the frontend but cannot be executed safely by IR and the backend, before
they become machine-code errors.

Promoting a feature to buildable requires checking the complete parser → resolver → typecheck →
ownership → IR → ABI → backend → CLI/std/LSP path. Pure AST shape classification may be shared, but
phase-specific facts such as symbol identity and type compatibility remain separate.

## IR, ABI, and Backend

- IR represents ownership transfer, drop obligations, and outcome exits as explicit operations.
- Optional and fallible layers retain distinct IR type identities even when they share a callable
  ABI shape. Shared operations use outcome terminology; error payload operations remain
  fallible-only.
- ABI classification is centralized in `abi` and shared by lowering and backend validation.
- Unsupported user source stops at buildability; backend validation remains a guard against drifted
  or hand-built IR.
- Target-specific syscalls and encoding stay inside backend/target and target-gated standard-library
  internals.
- Update [ABI and Layout](../../spec/09-abi-layout.md) when public layout or ABI behavior changes.

## Allocator and Drop Boundary

The Allocator is an ordinary standard-library API, but the compiler must represent runtime drop for
owned values. Keep immutable per-type drop shapes, mutable per-path drop obligations, and allocator
provenance separate. See [Allocator and Ownership](allocator-ownership.md).

The compiler does not special-case public names such as `Allocator`, `String`, or `Vec`. Required
primitives are confined to the `pub(nocter)` trust boundary and explicit IR operations.

Phase 0 established a compiler-owned provenance boundary between typecheck and ownership. Callable
provenance summaries, lexical outlives constraints, and allocation-effect facts are derived by the
same typecheck implementation. Return checking, NLL, region escape validation, analysis, and IR
consume those facts instead of maintaining separate origin models. See
[Region, Provenance, and Allocation Context](region-provenance.md).

Phase 3 interpolation uses a validated runtime-capability bundle. Typecheck produces a semantic
plan containing declaration identities, result type, allocation effect, provenance, and per-part
evaluation mode. Dedicated IR lowering consumes that plan; it does not resolve standard-library
names or repeat type dispatch. See [Owned String Interpolation and Formatting](interpolation.md).

Phase 4 keeps public result-origin contracts and generic dispatch in separate layers. Resolver
preserves origin, bound, conformance, and source-module identities; typecheck validates contracts
and records bound calls; analysis expands only reachable concrete specializations; IR selects a
target from that callable index without searching interface or method spellings. See
[Public Provenance Contracts and Generic Interface Bounds](provenance-contracts.md).

Phase 5 normalizes callable optional/fallible layers in the shared `outcomes` model. Buildability,
IR type conversion, backend ABI validation, and analysis consume that structure instead of module
or declaration spellings. The Darwin entry shim owns the process-context registers; narrow IR
values expose argument and environment views, while ordinary `std/process` source owns UTF-8,
matching, allocation policy, and public errors. See
[Nested Outcomes and Executable Process Context](outcomes-process-context.md).

Phase 10 keeps interface capability/default behavior and closure storage separate. Resolver
preserves required/default method and anonymous closure identities. Typecheck produces specialized
default-method calls, closure plans, and dedicated structural callable-call facts. Ownership and
provenance consume capture fields as ordinary aggregate state, while IR materializes anonymous
environments and invokes generated static targets. Built-in callable invocation never enters
interface or method lookup, and no later phase reconstructs its capability from a standard-library
declaration. See
[Callable Values and Interface Default Methods](callable-default-methods.md).

## LSP Boundary

`driver/lsp` owns transport, document state, and protocol conversion. `analysis` derives semantic
data for hover, completion, definition, references, and signature help from resolver/typechecker
facts. See [LSP](lsp.md).

Editor analysis has two shared internal boundaries. `SemanticOccurrenceIndex` maps source focus
spans to resolver/typechecker identities, roles, kinds, readonly state, and contextual type
applications. `analysis/presentation` maps those semantic values to normalized user-facing
declarations. Navigation and semantic tokens consume the former; hover, completion detail, and
signature help share the latter. A feature must not add its own name-resolution pass or render a
canonical signature by slicing source text.

Recovery overlays are temporary source inputs, not an alternate analysis system. A recovered file
must pass through the ordinary compile-unit frontend and the same occurrence/presentation
boundaries. Syntax-only fallback may attach documentation or provide a degraded result when no
semantic identity exists, but it cannot override an established semantic result.

## Diagnostics

- Malformed user source must not panic.
- Text diagnostics include file, line, column, snippet, primary marker, and help when applicable.
- JSON and LSP diagnostics retain stable machine-readable spans.
- Ordinary user diagnostics do not expose backend implementation terminology.
- The same semantic error follows the same diagnostic path in check, build, run, and LSP.

## Testing Layers

| Layer | Proves |
|---|---|
| lexer/parser | syntax shape, recovery, removed syntax |
| resolver | imports, visibility, symbol identity, source loading |
| typecheck | types, generics, ownership, borrows, drop, diagnostics |
| buildability | early rejection of runtime-unsupported forms |
| IR | operation shape, ownership/drop transitions, ABI handoff |
| backend/target | frame/layout assumptions, instruction encoding, emission |
| CLI build/run | user-visible native behavior |
| distributed home | packaged standard-library visibility and runtime behavior |
| analysis/LSP | agreement between compiler facts and protocol responses |

A user-visible promotion needs a focused phase test plus at least one CLI, distributed-home, or LSP
integration test.
