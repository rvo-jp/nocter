# Nocter Compiler Architecture

This document describes the Rust bootstrap compiler architecture. It is for
compiler engineers and AI coding agents. It is not the source of truth for
Nocter source-language rules; those live in [../../spec](../../spec/README.md).

## Design Goal

The compiler should remain self-contained:

```text
.nct source
  -> source map
  -> lexer
  -> parser
  -> import loading and resolution
  -> type checking, ownership, and buildability checks
  -> IR lowering
  -> ABI classification
  -> ARM64 Darwin code generation
  -> Mach-O executable image
```

Normal user builds must not depend on LLVM, `clang`, `as`, `ld`, Xcode Command
Line Tools, or an external runtime library.

## Phase Ownership

Each phase owns one kind of fact. Later phases may consume facts from earlier
phases, but they should not reimplement earlier decisions.

| Area | Owns |
|---|---|
| `source` | loaded source files, canonical file identity, byte spans, line mapping |
| `lexer` | tokenization and lexical diagnostics |
| `parser` | AST construction, parser recovery, removed-syntax diagnostics |
| `ast` | syntax tree data, AST JSON, syntax documentation extraction helpers |
| `resolve` | use graph loading, visibility, scopes, symbols, declaration identity |
| `typecheck` | expression types, signatures, ownership, borrow, drop, interface conformance |
| `driver/buildability` | source-backed rejection of checked forms outside the runtime subset |
| `ir` | compiler IR and source-to-IR lowering |
| `abi` | layout, ABI values, call/return classification |
| `backend` | ARM64 Darwin code generation and Mach-O output |
| `diagnostics` | structured diagnostic data and rendering |
| `driver` | CLI orchestration, JSON output, LSP server integration |

The LSP server belongs in `driver/lsp`, but language facts must come from
`resolve` and `typecheck`. LSP code may translate facts into protocol objects;
it must not define separate lookup, type, ownership, or visibility rules.

## Source And Compile Unit Model

The source root, import resolution rules, synthetic prelude rules, and Nocter
home rules are specified in
[Modules and Use Declarations](../../spec/01-modules-use.md) and
[Targets and Distribution](../../spec/10-targets-distribution.md).

Compiler implementation rules:

- `SourceMap` is the shared source identity layer.
- Diagnostics should carry source-backed spans when the source location is
  known.
- Import loading must produce one canonical source identity per loaded file.
- Open LSP documents may override on-disk text for compile-unit analysis, but
  must still follow the same source identity rules.

## Frontend Boundary

The parser may accept syntax that is not yet runtime-buildable, but accepted
syntax must eventually reach one of these states:

- resolved and typechecked with compiler-owned facts
- rejected by parser, resolver, typechecker, or buildability with a stable
  user-facing diagnostic
- explicitly deferred in the v0 closure document

The backend must not infer missing language semantics from raw AST structure.
If lowering needs a fact, that fact belongs in resolver or typechecker output.

## Buildability Boundary

The current compiler intentionally separates "checkable" from "buildable".
`driver/buildability` is the source-backed preflight for code that has valid
frontend facts but is outside the v0 runtime subset.

This boundary exists to avoid accidental backend errors such as:

- unsupported IR instruction paths reached by normal user source
- backend diagnostics phrased in machine-code implementation terms
- implicit semantic decisions made after type checking

When a feature is promoted to runtime support:

1. update the relevant `spec/` chapter if user behavior changes
2. update `v0-closure.md` when a `ship` or `reject` decision changes
3. update `implementation-status.md`
4. add parser, resolver, typecheck, buildability, lowering, backend, std, CLI,
   or LSP tests at the narrowest sufficient layers

## IR And ABI Boundary

IR lowering should consume typed facts and emit explicit operations. ABI
classification should be centralized in `abi` and reused by lowering and
backend validation.

Backend rules:

- validate IR shape before emission when drift would otherwise corrupt codegen
- use source-backed buildability diagnostics for ordinary user source outside
  the runtime subset
- keep target-specific machine details in backend and target-gated std
  internals
- keep Nocter ABI updates synchronized with
  [ABI and Layout](../../spec/09-abi-layout.md) and
  [Backend V0](backend-v0.md)

## Standard Library Boundary

The public standard-library specification lives in
[Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md).

Compiler docs may describe:

- which public std APIs currently build and run
- which APIs are check-only or recoverably unsupported
- which primitives are trusted active-home boundaries
- which target-gated declarations are required by the current backend

Compiler docs must not become a second public std specification. The runtime
status is tracked in [Std Runtime Status](std-runtime-status.md).

## Diagnostics

Diagnostic text should describe Nocter source concepts, not compiler internals.

Required behavior:

- malformed user source must not panic the compiler
- source-backed diagnostics should include file, line, column, source snippet,
  primary marker, and help when useful
- JSON diagnostics must preserve stable machine-readable spans
- LSP diagnostics must be derived from the same diagnostics pipeline
- backend validation diagnostics are acceptable for hand-built or drifted IR,
  but ordinary user source should be stopped earlier when possible

## Testing Strategy

Use the narrowest test layer that proves the behavior:

- parser tests for syntax shape and removed syntax
- resolver tests for use, visibility, symbol identity, and source loading
- typecheck tests for typing, ownership, borrowing, interfaces, and diagnostics
- buildability tests for stable rejection before IR/backend
- IR lowering tests for instruction shape and ABI handoff
- backend unit tests for instruction encoding, frame layout, and emission
- CLI integration tests for user-visible build/run/check behavior
- distributed-home tests for packaged `std/` visibility and public API behavior
- LSP tests for protocol behavior backed by compiler facts

Broad language promotions should include at least one CLI or distributed-home
test when they are user-visible.
