# Nocter Compiler Architecture

This document defines stable responsibility boundaries in the Rust bootstrap compiler. See the
[specification](../../spec/README.md) for public language rules and the
[v0.2.0 contract](v0.2.0.md) for the completed release criteria.

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
external runtime library. The v0.2.0 native target is `arm64-darwin`.

## Phase Ownership

| Area | Owns |
|---|---|
| `source` | canonical file identity, byte spans, line mapping |
| `lexer` | tokens and lexical diagnostics |
| `parser` | AST construction, syntax recovery, removed-syntax diagnostics |
| `ast` | syntax data, AST JSON, documentation extraction |
| `frontend` | compile-unit loading, prelude, frontend orchestration |
| `resolve` | imports, visibility, scopes, symbols, declaration identity |
| `typecheck` | types, generic specialization, places, ownership, borrows, drop semantics |
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

- IR represents ownership transfer, drop obligations, and fallible exits as explicit operations.
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

## LSP Boundary

`driver/lsp` owns transport, document state, and protocol conversion. `analysis` derives semantic
data for hover, completion, definition, references, and signature help from resolver/typechecker
facts. See [LSP](lsp.md).

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
