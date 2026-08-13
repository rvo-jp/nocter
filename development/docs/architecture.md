# Nocter Compiler Architecture

This document defines stable responsibility boundaries in the Rust bootstrap compiler. See the
[specification](../../spec/README.md) for public language rules and the
[v0.8.0 release record](../releases/v0.8.0.md) for the released conversion, ownership, and
editor-presentation boundaries.

## Pipeline

```text
.nct source
  -> SourceMap
  -> lexer / parser
  -> semantic definition index / resolution
  -> typed HIR / ownership and provenance
  -> control-flow MIR
  -> machine IR lowering
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
| `semantic` | typed compile-unit identities, definition/body/type records, source locations |
| `frontend` | compile-unit loading, prelude, frontend orchestration |
| `resolve` | imports, visibility, scopes, symbols, declaration identity |
| `typecheck` | types, generic specialization, places, ownership, storage provenance, regions, execution allocation requirements, drop semantics |
| `analysis` | owned editor/query results derived from compiler facts |
| `mir` | checked control flow, places, operands, loans, initialization, and explicit drops |
| `driver/buildability` | temporary pre-MIR rejection while Phase 3 migrates runtime coverage |
| `ir` | conversion from MIR to explicit machine-independent operations |
| `abi` | data layout and argument/return classification |
| `backend` | IR validation, ARM64 emission, Mach-O output |
| `target` | machine encoding and target-specific output details |
| `diagnostics` | structured diagnostics and text/JSON rendering |
| `driver` | CLI, pipeline, and LSP protocol orchestration |

The pipeline above is the v0.14.0 target architecture. Phase 0 established definition and body
identity; Phase 1 established checker-owned partial typed HIR; Phase 2 completed indexed editor
projection and semantic identity for type occurrences and bindings. Phase 3 removes buildability
preflight and AST-driven IR lowering as their MIR replacements become authoritative. See
[Semantic Identity and Typed Model](semantic-model.md).

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
ownership → MIR → IR → ABI → backend → CLI/std/LSP path. During Phase 3, an explicitly routed
construct may use either its old preflight/lowering family or MIR, never both and never fallback
after a MIR error. The old family is deleted when that construct migrates. At Phase 3 completion,
buildability is MIR validation rather than a parallel AST model.

## IR, ABI, and Backend

- Phase 3 MIR currently owns the migrated scalar subset: identity-backed parameters and bindings,
  assignments, arithmetic and comparison temporaries, conditional diamonds, and direct scalar
  calls. Calls retain canonical `DefId` targets and explicit returning or non-returning CFG
  continuations. Buildability and machine-IR lowering consume the same checked body from one
  compile-unit cache. A construction-only control-flow builder may own open blocks, but checked MIR
  never contains placeholder terminators. Straight-line calls split expression evaluation into
  ordered blocks and explicit result places. Structural MIR-to-IR conversion follows linear branch
  paths to a common join. Trapping fallible calls use separate success and trap successors rather
  than an AST-only failure flag. Unsupported body families are selected before MIR construction and
  remain on the named migration route; a selected MIR body never falls back.
- MIR locals keep semantic type, runtime representation, ownership behavior, logical storage,
  source identity, and lexical scope as separate checked contracts. Basic blocks retain the same
  `ScopeId` tree, so cleanup construction can derive every exited scope from CFG edges.
  Parameter storage retains source ordinal rather than an ABI word index. Parameter ABI words and
  aggregate staging slots are selected by one backend parameter projection from the validated ABI
  layout, including preceding multiword parameters. Machine-local slots are another backend
  projection and may omit only proven single-definition/single-use values.
- Aggregate places are a base `LocalId` plus an optional validated `ProjectionPathId`. Projection
  records retain parent identity, checked result type and ownership, and field offset or checked
  index layout; backend lowering never interprets an AST member or index expression to recover a
  storage path.
- Definite initialization is a MIR fixed point over CFG edges. Its place-state domain separates
  whole locals, explicitly initialized projections, and projections invalidated by partial moves.
  A field move therefore preserves available siblings without leaving the aggregate root movable.
  Joins intersect normalized place availability, ordinary calls initialize their destination on
  the return edge, and fallible calls do so only on the success edge. Machine IR therefore never
  guesses whether a reachable operand or return place contains a value.
- Owned cleanup and borrows use separate path-sensitive MIR analyses over the same typed dense-set
  substrate. Drop obligations retain may-live and must-live state across joins; loans have explicit
  identities and begin/end points. Loan overlap follows projected places: a root overlaps every
  child, ancestors overlap descendants, and distinct field projections are disjoint. Neither
  cleanup nor alias validity is inferred from AST nesting or a single ambiguous runtime-live flag.
- MIR construction emits `BeginLoan` when a borrow binding acquires stored scalar storage.
  Cleanup inserts `EndLoan` on every CFG edge that exits the loan's lexical scope, and machine-IR
  projection turns only the begin point into pointer materialization. The end point remains a
  checked lifetime boundary with no runtime instruction.
- Cleanup materialization consumes retained definite-initialization edge states and lexical scope
  transitions. It inserts explicit reverse-order drop chains for each concrete CFG successor and
  callable exit, including distinct success and failure states for outcome calls. A wholly
  available owned local produces one root drop; after a partial move, maximal remaining owned
  projections produce separate drops, so cleanup neither destroys moved storage nor leaks sibling
  fields.
- MIR calls carry each argument's checked semantic type and value representation. Scalar, borrow,
  and aggregate arguments therefore share call identity and continuation semantics; conversion to
  ABI-specific machine-IR argument forms is a backend projection rather than a separate call model.
  Borrow parameter forwarding projects to its ABI pointer word without inventing a scalar local.
  Borrow and indirect aggregate arguments share one IR-owned predicate that prohibits tail-call
  frame teardown while an argument still depends on the caller frame.
- Recoverable scalar outcome calls represent `otherwise` as an explicit failure block that writes
  the call destination and rejoins success. Backend `Recover` mode is a projection of that CFG;
  `Trap` and `Propagate` remain distinct terminal failure edges.
- A discarded scalar catch uses the same failure branch without an error-payload local. The CFG
  rejoin determines recovery semantics, while a named catch will require an explicit failure
  payload place rather than a backend-only binding convention.
- Copy aggregate parameters are represented as aggregate MIR locals rather than ABI word lists.
  Checked field selections become typed MIR projection paths; the backend maps those paths to the
  aggregate staging slot established by the same parameter projection used at function entry.
- MIR construction takes one immutable semantic input bundle containing the semantic database,
  current resolver, compile-unit resolver map, and checked HIR. Layout of imported and nested
  aggregate fields therefore uses the same cross-source authority as ABI signature projection.
- Whole copy aggregate parameter places may become MIR call arguments. Machine-IR projection uses
  the parameter's already validated direct/indirect ABI classification. An indirect argument points
  into the current stack frame and therefore makes tail-call frame reuse ineligible.
- Nested aggregate members remain parent-linked MIR projection paths. Each segment carries checked
  type, representation, ownership, and relative layout offset; machine-IR lowering folds those
  offsets only after selecting the aggregate's physical storage.
- Built-in integers use canonical `IntegerType` in MIR. Non-legacy widths share the word-based
  machine-IR integer operations while preserving width and signedness through arithmetic,
  shifts, comparison, call, field-load, and return projection.
- `u8` is intentionally a dedicated MIR scalar while machine IR and ABI retain specialized byte
  locations. Its source parameter ordinal maps to a validated `U8` ABI slot; it is not mislabeled
  as a generic integer and recovered by a backend exception.
- Boolean short-circuit operations are CFG, not binary rvalues. MIR selects either the right-hand
  evaluation block or a constant-result block and joins at the destination place, so calls,
  traps, and future cleanup on the skipped edge remain unreachable by construction.
- Expression-valued conditionals use the same MIR builder at function tails and nested operand
  positions. Both branch blocks carry child `ScopeId`s, assign one destination, and join in the
  parent scope; route selection does not depend on where the source expression is written.
- Value-producing branch and outcome-recovery blocks share one statement-plus-tail MIR builder.
  Machine-IR projection structures nested diamonds by their nearest common reachable MIR block,
  so bindings and nested conditionals inside recovery do not require a syntax-shaped backend path.
- Numeric negation and boolean inversion are distinct MIR unary operations. The verifier checks
  their scalar domain, while machine-IR projection alone chooses subtraction-from-zero or boolean
  inversion; lowering does not erase the authored operation before validation.
- Integer casts enter MIR only from exact or lossless conversion decisions retained by typed HIR.
  The MIR cast records both semantic types and scalar representations, validation checks range
  inclusion again, and backend projection alone materializes sign or zero extension.
- Scalar compound assignments are represented as ordinary MIR read-modify-write assignments.
  Their operators share the same closed `BinaryOperator` domain as expression arithmetic, so
  machine lowering does not infer compound semantics from an AST assignment token.
- Primitive declarations are recognized as a closed `IntrinsicId` at the resolution-to-lowering
  boundary. Machine-IR selection dispatches on that identity; the source spelling is retained only
  for diagnostics and source-boundary recognition.
- MIR builders produce a construction-only body and pass it through one `finalize` boundary.
  A single `LoweringContext` owns all mutable construction state; nested statement, expression,
  conditional, and outcome builders cannot substitute independent local, projection, scope, or CFG
  collections.
  Finalization validates definite initialization, materializes cleanup edges, and validates the
  completed representation before it can enter the compile-unit cache.
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
primitives are confined to the exact implicit toolchain standard-library package and explicit IR
operations. Their `pub(/)` visibility controls access within that package but does not grant
primitive authority.

Phase 0 established a compiler-owned provenance boundary between typecheck and ownership. Callable
provenance summaries, lexical outlives constraints, and allocation-effect facts are derived by the
same typecheck implementation. Return checking, NLL, region escape validation, analysis, and IR
consume those facts instead of maintaining separate origin models. See
[Region, Provenance, and Allocation Context](region-provenance.md).

Callable provenance is a fixed point over a complete compile unit. `TypecheckCompileUnitContext`
owns that immutable result, and every source-level provenance, ownership, and return check borrows
the same context. A source checker must not reconstruct the compile-unit fixed point.

Interpolation uses a validated runtime-capability bundle containing `String` construction and the
exact `Format` contract identities. Typecheck resolves every part to the shared protocol-method
plan used by generic static dispatch. Dedicated IR lowering consumes that plan; it does not resolve
standard-library names, repeat conformance lookup, or classify input types. See
[Owned String Interpolation and Formatting](interpolation.md).

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
