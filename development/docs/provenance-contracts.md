# Public Provenance Contracts and Compiler-Owned Result Storage

This document owns the compiler implementation boundary between source-visible `from` contracts
and internal result-storage inference. Public language semantics belong in the specification. The
frozen v0.7.0 implementation evidence belongs to the
[v0.7.0 release record](../releases/v0.7.0.md).

## Design Boundary

Source contracts name only caller-managed origins retained by a result:

```text
source `from` clause
  -> resolved receiver and parameter identities
  -> external-origin contract validation
  -> call-site identity substitution
  -> loans, ownership, and region escape
```

Fresh result storage and execution allocation remain compiler-owned:

```text
trusted allocation roles and callable bodies
  -> lossless provenance summaries
  -> recursive fixed-point convergence
  -> type-directed abstract-boundary fallback
  -> active-region instantiation at each call
```

These flows share `ValueProvenance`, but neither reconstructs the other from spelling or a
standard-library name.

## Source Contract Rules

- An omitted clause expands to no caller-managed origin when the successful result is independent,
  fresh, or static, and to one inferred origin when exactly one typed receiver, parameter,
  allocator, or literal pack is eligible.
- A public body with multiple eligible inputs may omit the clause only when exact body evidence
  retains none of them. Retaining any candidate requires an explicit upper bound.
- A bodyless or structural callable with multiple eligible inputs is ambiguous and invalid without
  an explicit clause.
- An explicit clause remains legal as a wider documented upper bound. It is never reconstructed in
  formatter or editor output when source omitted it.
- For a fallible return, the public contract constrains only the success branch. Error provenance
  remains compiler-owned escape evidence.
- Absence of `from` does not promise allocation-free execution or independence from the active
  lexical region.
- `static` is the only non-input public origin. `from current` is invalid because the current
  allocation context is a compiler capability.
- Private body-backed callables may keep exact inferred origins without repeating an API contract.
- Body-backed summaries retain their exact outcome and aggregate shape after the public upper
  bound is validated. A coarse `from left | right` clause must not erase path-sensitive evidence.

## Internal Ownership

- `ast/provenance` owns accepted source clauses and source spans.
- `resolve/body` binds origin spans to receiver and parameter declarations.
- `typecheck/provenance/contracts` converts clauses into semantic origins.
- `typecheck/provenance/elision` is the sole zero/one/ambiguous candidate classifier and expands
  source omission for every semantic consumer.
- `typecheck/provenance/result_allocation` owns fresh-result projections independently of external
  contracts and the execution allocation requirement.
- `typecheck/provenance/storage_capability` distinguishes general storage provenance from the
  narrower type-directed capability used at abstract result boundaries.
- `typecheck/provenance/storage_projection` filters scalar-only dataflow while validating public
  contracts.
- `typecheck/returns/borrow_returns/summaries` keeps exact body evidence, seeds bodyless declared
  origins, and supplies conservative fresh storage only when an abstract result needs it.
- `typecheck/returns/borrow_returns/mutation_effects` preserves storage retained through readwrite
  inputs.
- `typecheck/allocation` separately infers whether execution needs the hidden current context.
- `target/trusted` attaches allocator, process, iteration, and ownership-transfer semantics to
  validated declaration identities.
- `analysis/presentation` renders normalized accepted declarations. It never appends inferred
  allocation prose or private aggregate provenance.

## Abstract Callable Boundaries

A bodyless or structural callable with an explicit clause uses that success-result contract. With
no clause, the shared classifier expands zero or one eligible input and rejects an ambiguous
storage-bearing result. Scalar branches remain independent. Owned fresh storage receives the
current allocation context internally, and fallible error storage receives a separate shaped
summary. These rules are type-directed and contain no `String`, `Vec`, or standard-library
allowlist.

Trusted allocation operations override the abstract summary with their exact source: the active
context or a resolved allocator input. Body-backed direct calls use their exact converged summary.
Structural callable calls instantiate the same expanded contract and type-shaped fresh/error
storage summary used by declaration-backed calls.

## Generic and Interface Dispatch

Generic calls resolve canonical interface identity and type arguments before provenance
substitution. Conformance checks compare receiver capability, generics, parameters, result types,
outcome layers, and external `from` contracts. Fresh result storage is not callable variance.

Resolved iteration conversion and step declarations feed the same summary-instantiation path as
ordinary calls. Closure captures are compiler-generated aggregate fields, so callback results keep
capture and argument origins without a closure-specific lifetime graph.

## Editor Boundary

Hover, completion, and signature help show normalized declarations containing only accepted
source contracts. Semantic tokens treat `alloc` as an ordinary identifier. Inlay hints may show
inferred binding types and bounded source provenance, but not execution-allocation requirements,
fresh-result markers, or phrases such as `from inferred storage`.

The removed result modifier has no AST, resolver field, formatter mode, semantic-token category,
or source-edit action. Parser recovery may emit one focused obsolete-syntax diagnostic, but all
downstream compiler phases see only the current model.

## Historical Foundation

v0.3.0 established identity-resolved `from` contracts, generic substitution, closure provenance,
and region checking. v0.6.0 established lossless fresh-result dataflow across aggregates, outcomes,
recursion, mutation, iterators, and ownership transfer. v0.7.0 removes the public allocation
modifier while preserving that semantic foundation behind the source boundary.
