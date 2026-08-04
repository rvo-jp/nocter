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

Type-member presentation has one shared renderer across declaration hover, reference hover, and
completion detail. Fields, variants, methods, and associated functions always include their visible
owner as `Type.member`; generic completion substitutes the concrete owner and member types when
typecheck establishes them. Canonical declaration identity remains internal and is not exposed as a
repository or module path.

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

## Completed v0.3.0 Phase 3 Integration

Interpolation tooling uses `analysis/interpolation`, which reads the typecheck semantic plan rather
than recognizing string syntax or standard-library names in protocol code.

- hover on a complete interpolation reports owned `String`, the current-allocation effect, and the
  result origin
- hover on an interpolation part reports its accepted concrete input type while nested expressions
  retain ordinary declaration and expression hover
- completion and signature help inside incomplete `${...` expressions use a cursor-preserving
  syntax overlay and then run the ordinary compiler query
- the recovery scanner respects escapes, comments, nested delimiters, and nested string forms
- unresolved capabilities or expression types produce no invented interpolation fact
- JSON-RPC tests cover complete hover plus incomplete completion and nested-call signature help

## Completed v0.3.0 Phase 4 Integration

Provenance and generic-bound tooling reads resolved callable signatures and typecheck facts. The
protocol layer does not parse `from`, inspect interface names, or repeat conformance lookup.

- hover and signature help append normalized `from` origins to callable signatures
- completion after `from` offers only eligible receiver, parameter, `static`, and `current` origins
- completion on a bounded generic receiver lists only methods declared by its canonical interface
- definition and references group a bounded call with the interface method declaration
- semantic tokens classify generic parameters as types and provenance inputs as readonly parameters
- recovery for incomplete `T: ` and `from ... |` edits preserves the cursor while requiring the
  recovered compiler run to establish every identity
- JSON-RPC tests verify exact hover and definition ranges for a bound method call

## Body-Bearing Interface Implementation Integration

Concrete interface calls expose the selected conformance member rather than an unrelated inherent
method or a method-name reconstruction. Hover and signature help present the specialized concrete
receiver; completion and definition retain the implementation member span. Generic-bound calls
continue to define to the interface contract while specialization facts carry the concrete dispatch
member into buildability and lowering.

The compiler candidate model keeps contract ownership separate from dispatch ownership. Contract
ownership supplies visibility and interface generic arguments. Dispatch ownership supplies impl
generic inference and the static method target. This distinction prevents same-spelled interface
and impl generic parameters from corrupting editor types such as `ViewIter<T>` and `Iterator<&T>`.
The protocol layer consumes these identities and never repeats conformance selection.

## Completed v0.3.0 Phase 5 Integration

Nested-outcome and process tooling uses normalized callable signatures and provenance facts from
analysis. Protocol code does not recognize `env`, reinterpret `?!`, or synthesize a `from` clause.

- hover on an imported or aliased process lookup shows the normalized `(&str)?!` return
- callable hover includes the exact `from static` contract and static result provenance
- the hover range covers the imported alias or callable name, not an enclosing module/type prefix
- malformed and incomplete consumers continue through the ordinary recovery pipeline
- JSON-RPC tests verify exact normalized hover content and alias source range

## Completed v0.3.0 Phase 6 Integration

Stored outcome tooling reads normalized local and expression types from typecheck facts. Protocol
code does not inspect tags, reconstruct outcome layers, or special-case a consumer expression.

- hover on a saved optional, fallible, or supported composed value preserves its normalized type
- completion detail reports the same stored type as hover
- later consumption does not replace the saved binding's declaration fact with its payload type
- generic aliases and specializations retain their concrete outcome layers
- JSON-RPC tests cover stored `T!?` hover and completion through the protocol boundary

## Completed v0.3.0 Phase 7 Integration

Collection-iteration tooling consumes the immutable typecheck plan and callable semantic summaries.
It does not search for `iter`, `into_iter`, or `next` spellings.

- hover on the loop binding or source reports readonly, owned, or direct mode plus the concrete
  iterator and element types
- hover reports the statically selected conversion and step targets, including an implicit
  allocation effect when either target uses the current allocation context
- completion inside the body reports the exact collection-element type
- incomplete `for item in`, `for item in &`, and partial-source headers use a cursor-preserving
  syntax overlay without creating a conformance fact
- semantic-token recovery removes placeholder identifiers and remaps every later source range to
  the original document
- JSON-RPC tests require parser diagnostics for incomplete input and reject an invented
  ownership-ambiguity diagnostic

## Completed v0.3.0 Phase 9 Integration

Capability-set tooling consumes the ordered bound list and resolved interface declaration
identities retained by the compiler. It does not repeat conformance matching in the protocol layer.

- declaration hover preserves normalized bound order such as `T: Readable + Measurable`
- unresolved generic signature help preserves every bound; concrete calls continue to show their
  specialized type arguments
- member completion combines every interface in the capability set and deduplicates by declaration
  identity
- distinct interfaces that declare the same member name produce no arbitrarily selected completion
  target; an actual ambiguous call receives the typechecker diagnostic
- definition and references retain the selected interface method declaration
- recovery after an incomplete `T: A +` inserts only a temporary syntax placeholder and accepts
  results only after the preceding bound identities resolve again
- JSON-RPC tests cover two-bound completion, hover ranges, definition identity, and normalized
  provenance together

## Completed v0.3.0 Phase 10 Integration

Closure and default-method tooling consumes normalized typecheck and declaration-identity facts. The
protocol layer does not infer captures, rediscover conformances, or recognize iterator method names.

- hover presents closure bindings, parameters, and explicit capture modes from normalized anonymous
  types, including `closure mut` and `closure once` capability distinctions
- hover, completion detail, and signature help present specialized interface defaults with their
  concrete receiver and method-level generic arguments
- direct callable signature help presents the normalized `&func`, `&+func`, or `func` contract,
  declared parameter names, result provenance, and the source binding being invoked
- definition, references, and semantic tokens retain closure-local parameter/capture identity and
  the selected default declaration
- one delimiter-recovery scanner closes incomplete blocks while respecting strings and comments;
  hover, completion, signature help, and region recovery reuse that scanner
- incomplete closure bodies retain established capture, parameter, receiver, and field facts without
  creating an erased callable or interface conformance identity
- JSON-RPC tests cover closure hover, specialized default signature help, and field completion inside
  an unclosed closure body

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
incomplete zero-argument iterator calls through direct and JSON-RPC queries. Phase 3 tests cover
owned result/effect/origin hover, interpolation-part types, and cursor-preserving completion and
signature recovery through direct and JSON-RPC queries. Phase 4 tests cover normalized provenance
labels, eligible-origin and bound-method completion, specialized signature help, interface-targeted
definition/references, semantic classification, recovery, and protocol source ranges. Phase 5
tests cover nested outcome normalization, exact `from static` display, static result provenance,
and aliased callable hover ranges. Phase 6 tests cover normalized stored composed values through
direct analysis and JSON-RPC hover/completion queries. Phase 7 tests cover exact source modes,
concrete iterator/item facts, body completion, implicit allocation effects, incomplete-header
diagnostics, and range-safe semantic recovery. Phase 9 tests cover complete capability-set hover and
signature help, unambiguous/ambiguous member completion, incomplete additional-bound recovery, and
JSON-RPC agreement. Phase 10 tests cover normalized closure/capture presentation, default-method
specialization, semantic identity, shared delimiter recovery, and direct plus JSON-RPC agreement.

## Deferred Features

Rename, code actions, formatting requests, a workspace-wide package index, and inlay hints remain
later v0.3.0 work. Add them after the semantic facts and recovery APIs used by hover, completion,
and signature help remain stable.
