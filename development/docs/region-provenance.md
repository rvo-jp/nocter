# Region, Provenance, and Allocation Context

This document owns the implementation design for the v0.3.0 region and allocation-context
foundation. Public
semantics belong to [Memory, Regions, and Allocators](../../spec/06-memory-region-allocator.md), and
the completed gate belongs to the [v0.3.0 Release Record](../releases/v0.3.0.md).

## Separate Concepts

Do not collapse these concepts into a single lifetime label:

| Concept | Meaning |
|---|---|
| storage origin | the storage whose lifetime constrains a value |
| loan | a readonly or readwrite access restriction over a source place |
| value provenance | storage origins carried through a value and its projections |
| allocator backend | runtime mechanism that allocates and releases bytes |
| allocation context | statically propagated capability selecting an allocator and region |
| lexical region | child lifetime boundary that owns an allocation context |
| drop obligation | runtime record of initialized owned values that require cleanup |

Allocator backend identity remains runtime data where deallocation needs it. Storage origins and
outlives constraints are compile-time facts. Drop obligations remain IR/runtime control-flow state.

## Provenance Data Flow

```text
AST places and calls
  -> resolver declaration/place identity
  -> typecheck storage origins and callable summaries
  -> ownership loan ranges + escape diagnostics
  -> analysis facts                    -> LSP
  -> IR allocation/region/drop facts
  -> backend execution
```

The resolver owns identity but not lifetime policy. Typecheck owns origin construction, joins,
outlives constraints, and diagnostics. Ownership consumes those facts to calculate NLL loans; it
does not reconstruct call behavior. IR receives only validated region and cleanup operations.

## Internal Model

The exact Rust representation may evolve, but the semantic model must distinguish:

```text
StorageOrigin
  Static
  Scope(ScopeId)
  Input(CallableId, InputId)
  Region(RegionId)
  CurrentAllocationContext
  Allocated(StorageOrigin)
  Unknown

ValueProvenance
  Independent
  Origins(set<StorageOrigin>)
  Aggregate { fallback, fields, elements }
  Optional { present }
  Fallible { success, error }
```

`InputId` is declaration identity, not a parameter name string. A method receiver has its own input
identity. Renaming a parameter must not change callable-summary behavior.

`CurrentAllocationContext` is a symbolic summary origin. At a call site it is concretized to the
innermost lexical `RegionId`, or to `Static` in the root context. This lets an allocating helper
retain storage provenance without baking a caller's region identity into its callable summary.

`Allocated(origin)` records that storage newly created by an allocation operation survives in the
value while preserving the underlying lifetime domain. It is not an execution effect: an
allocation discarded before return does not appear in result provenance. Summary instantiation
substitutes the underlying domain and retains the allocation marker across aggregate fields,
outcome branches, calls, and literal construction.

Origin joins are conservative unions. Escape succeeds only when every possible origin outlives the
destination. A known static alternative does not erase a shorter alternative.

## Callable Summaries

The existing return-only summary becomes a shared `CallableProvenanceSummary` keyed by declaration
identity. It contains shaped result provenance, including retained allocation markers, plus the
separate execution-time current-context requirement.

Summary construction must:

- preserve field, element, optional, success, and error distinctions
- converge across recursive and mutually recursive source call graphs
- map callee input identities to caller expressions at each call site
- fall back to `Unknown`, never to a guessed parameter, when analysis is incomplete
- expose a stable owned query API to analysis and LSP

Return checking rejects local, owned-parameter, temporary, region, and unknown escapes. NLL maps
returned origins back to source places and keeps those loans active through the result's last use.

Phase 4 exposes the inferred relationship as an optional identity-resolved `from` contract at API
boundaries. A concrete body is still inferred and then checked against that upper bound. A
bodyless interface method seeds its callable summary from the contract. Call sites substitute
receiver and parameter identities exactly as they do for inferred summaries; source syntax never
introduces lifetime arithmetic or a parallel provenance graph.

Phase 10 closure environments enter this model as compiler-generated aggregates. Each explicit
capture field retains the captured place's exact provenance and loan. Generated closure call
targets use receiver and parameter identities in the same callable-summary fixed point as source
functions. An adapter calling a closure therefore propagates result origins and allocation effects
without a closure-specific provenance graph.

## Execution Allocation Requirements

An allocating source callable receives the current allocation context as a hidden capability. The
effect is inferred through calls to a fixed point and is part of compiler semantic facts. It is not
implemented as a mutable global or thread-local lookup.

This fact answers whether execution needs the ambient allocation context. It does not answer
whether allocated storage survives in the result. Explicit allocator operations can produce an
`Allocated(Input(...))` result without needing the ambient context; scratch allocation can require
the ambient context without adding `Allocated(...)` to the callable result.

The root driver supplies a program-lifetime aborting system context. Entering a lexical region
derives a child context from its parent and installs it for the region body's allocating calls.

Trusted allocation declarations carry metadata such as:

```text
allocation operation
result storage comes from allocator input N
failure policy: aborting | recoverable
```

The metadata belongs to trusted declaration identity. No analysis searches for names such as
`alloc`, `String`, `Vec`, or `reserve`.

## Failure Policies

`TryAllocator` is the fundamental fallible capability. Its failure-atomic operations return
`error`. `Allocator` is an aborting adapter over the same backend.

Normal and `try_*` collection operations share one implementation path:

```text
try operation
  -> checked layout/growth
  -> backend attempt
  -> publish only on success

normal operation
  -> try operation
  -> allocation error becomes non-allocating process termination
```

Failure policy is not stored as value provenance. `RawBuffer` stores the backend and region identity
needed for release. An operation chooses whether failure is returned or terminates.

## Region Lowering

Conceptually:

```text
evaluate parent context
enter child RegionId and runtime region
install child allocation context
execute body
for every exiting edge:
  drop live body values in reverse ownership order
  release child region
  transfer control
```

The exiting-edge transformation covers fallthrough, `return`, `break`, `continue`, and propagation.
Calling a `never` function does not synthesize cleanup because the process may terminate without
unwinding.

Dropping a region-backed buffer and releasing the region must not free the same storage twice. The
child backend owns whether individual deallocation reclaims a block immediately or records it as no
longer live before bulk release; collection code uses only the ordinary buffer release contract.

Nested regions form lexical outlives edges. A child origin can flow into a shorter nested child but
cannot flow into its parent or another region whose lifetime is not proven shorter.

## Diagnostics and Tooling

Diagnostics identify both the value being escaped and the origin that is too short. They should
name source bindings or lexical regions where possible and avoid Rust lifetime terminology.

Compiler analysis exposes:

- region declaration and binding spans
- parent/current allocation context identity
- allocating-call effect
- value origin summary suitable for hover
- the source range responsible for an escape

LSP converts these facts to hover, semantic tokens, completion, and diagnostics without maintaining
a second provenance graph. The retained aggregate fact is an internal compiler value, not a public
description of nominal representation. Analysis derives a bounded storage summary that omits
storage-independent and scalar-only branches, coalesces equivalent origins, and does not reveal
private field names.

## Completed Migration Boundary

Phase 0 retains the v0.2.0 explicit fallible behavior through the shared fallible core and exposes
ordinary allocation through its aborting adapter. Default allocation uses statically propagated
context facts; it does not call `page_allocator()` at each use site and does not depend on a mutable
global or thread-local allocator.
