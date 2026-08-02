# Language Server

The Nocter LSP is a protocol view of compiler facts. It does not create a separate resolver or type
system for editors.

## Architecture

```text
open documents + filesystem
  -> compile-unit frontend
  -> resolver/typecheck facts
  -> feature-specific analysis result
  -> LSP protocol conversion
```

`driver/lsp` owns JSON-RPC, document state, URI/range conversion, and capability routing.
`analysis` provides compiler-owned result types for hover, completion, definition, and references.
Resolver and typechecker decide visibility, type normalization, generic specialization, and
ownership capability.

## Current Baseline

The server supports document sync, diagnostic publication, semantic tokens, hover, definition,
references, document symbols, global/member/enum-pattern/struct-field completion, and signature
help. A shared call-site result combines resolved target, generic specialization, active parameter,
and documentation for hover and signature help.

Completion derives lexical scope and shadowing, generic member specialization, receiver capability,
signature detail, documentation, and insertion text from compiler facts. Import paths share the
frontend module layout and workspace/source roots; imported symbols come from resolved import
identity and visibility. Call-argument candidates use typechecker assignability for ranking.
Incomplete calls, member expressions, imports, regions, and typed literals use temporary
compile-unit recovery overlays separate from the authoritative document.

## Released v0.2.0 Capabilities

### Hover

Hover presentation is built from typechecked facts rather than reconstructed from AST text.

| Target | Required contents |
|---|---|
| local / parameter | mutability, borrow capability, resolved type |
| function / method | full signature, generic parameters or specialization, fallibility |
| struct / enum / interface | declaration kind, type parameters, documentation |
| field / variant | owner type, field or payload type, documentation |
| imported symbol | resolved declaration, module path, visibility |
| expression | normalized result type when no declaration target is sufficient |

Responses include a source-backed range. During incomplete edits, the server never invents a type:
it returns only established declaration information or `null`.

### Completion

A completion request classifies cursor context before collecting candidates:

- expression / statement: visible locals, parameters, functions, types, and keywords
- import: modules and public symbols reachable from the current module
- member: fields and methods of the receiver type, excluding candidates that violate borrow
  capability
- enum pattern: variants and payload fields of the target enum
- struct literal: fields not yet specified
- call argument: visible values compatible with the expected type and active parameter
- typed literal target: visible sequence or string shapes declared for the resolved nominal type
- typed literal element: visible values ranked against the specialized element-pack type

Candidates include at least `label`, `kind`, a type or signature `detail`, a documentation summary,
and required `insertText`. Completion respects visibility and shadowing and deduplicates semantic
symbols. Ranking prioritizes exact prefix, locality, and expected-type compatibility, with ordering
fixed by tests.

### Signature Help

The resolved call target and argument index provide:

- the full callable signature
- parameters and parameter documentation
- the active parameter
- return type and fallibility
- concrete generic types when specialization is known

The server does not guess overload-like candidates with string matching. Only when resolver or
typechecker cannot establish a target may recovery analysis return an explicitly incomplete result.

## Completed v0.3.0 Phase 1 Integration

Typed literal tooling uses `analysis/literals` as its semantic query boundary. The query joins the
expression's resolver identity, typecheck result, declaration shape, generic substitutions, and
documentation without depending on `Vec`, `String`, or hidden lowering target names.

- hover on a literal target or delimiter reports the specialized literal signature and declaration
  documentation
- signature help inside `[]` or `""` reports the element pack or string parameter
- go-to-definition on a delimiter targets the declaring shape
- completion after a nominal target offers only its accessible `[]` and `""` definitions
- element completion uses the specialized pack element type for expected-type ranking
- recovery closes missing delimiters and unclosed literal bodies in a temporary overlay
- generic source bodies retain editor facts such as `T` even when no concrete code-generation
  specialization exists yet

The recovery path is validated before it replaces ordinary or region recovery, so an identifier
followed by whitespace cannot become a synthetic literal fact unless resolver and typechecker
establish a literal definition.

## Completed v0.3.0 Phase 2 Integration

Iteration and collection tooling continues to use ordinary generic method facts. It does not
recognize `ViewIter`, `VecIntoIter`, `next`, `get`, or any standard-library module name.

- completion specializes readonly and consuming iterator methods from the concrete receiver
- receiver filtering offers `&+self.next()` only for a writable iterator place
- hover and signature help distinguish `(&T)?` from `T?` and include callable result provenance
- optional borrow formatting retains parentheses, avoiding the ambiguous `&T?` presentation
- incomplete zero-argument member calls try both a closed empty call and an argument-placeholder
  overlay through the general call recovery pipeline
- JSON-RPC tests verify that an incomplete `.next(` edit resolves the same compiler-owned method
  signature as the complete call

## Reliability Requirements

- Leave no stale diagnostics after didOpen, didChange, or didClose.
- Centralize conversion between UTF-16 LSP positions and UTF-8 source byte spans.
- Do not panic on malformed or incomplete source, unknown imports, or missing receivers.
- Imports see open-document overlays and never mix disk text under a second identity.
- Hover, completion, and signature help do not report contradictory types at one cursor.
- Protocol response tests are backed by unit tests for compiler analysis results.

## v0.3.0 Phase 0 Integration

Region and allocation tooling consumes compiler analysis facts rather than inspecting keywords or
standard-library names. The completed Phase 0 integration provides:

- semantic identity and definition for a lexical region binding
- hover detail for the region's parent and current allocation context
- allocating-call effect in callable hover detail
- source-backed escape diagnostics that identify the value and shorter origin
- completion for the required `using` position from typechecked allocator/context candidates
- recovery for incomplete region headers and bodies without stale diagnostics

LSP does not build a second region graph or infer provenance from `String`, `Vec`, or allocator
method names.

## Acceptance Tests

The released v0.2.0 integration tests cover:

1. hover and specialized signature help for an imported generic function
2. `Vec<String>` method completion and receiver borrow capability
3. payload enum pattern completion and missing struct-literal fields
4. hover/completion detail for documented standard-library symbols
5. consecutive didChange operations containing incomplete calls, member access, and imports
6. definition/reference/diagnostic consistency under a multi-file open-document overlay

Phase 0 integration tests additionally cover region binding definition and semantic tokens,
parent/current-context hover, allocating-call effects, allocator-aware completion, storage-origin
presentation, source-backed escape diagnostics, and cursor-preserving recovery for incomplete
region headers. Phase 1 tests cover sequence and string shape suggestions, imported definitions,
specialized hover and signature help, expected element ranking, delimiter definition, duplicate
expression facts, and incomplete expression/declaration recovery. Phase 2 tests cover concrete
readonly/owned result types, receiver capability, returned-borrow provenance, and complete plus
incomplete zero-argument iterator calls through direct and JSON-RPC queries.

## Deferred Features

Rename, code actions, formatting requests, a workspace-wide package index, and inlay hints remain
later v0.3.0 work. Add them after the semantic facts and recovery APIs used by hover, completion,
and signature help remain stable.
