# Incremental Computation Boundary

This document owns the current cross-crate contract that turns immutable source revisions into
reusable compiler and editor products. Compiler stages remain the sole semantic authorities;
computation schedules, memoizes, validates, and reuses their products without implementing a
language rule.

## Query Graph

```text
admitted source revision
        |
        v
source text -> parse -> declaration surface -> module surface
        |                                            |
        +---------------- discovery topology --------+
                                                     |
                                                     v
                                      atomic semantic-scope inputs
                                                     |
declarations -> preparation -> body names -> typed bodies -> finalization
       |              |             |              |             |
       +--------------+-------------+--------------+-------------+
                                      program analysis
                                             |
                              complete or incomplete unit analysis
                                             |
                                             v
                                    session semantic evidence
```

Commands and workspace analysis both enter through `nocter-compiler-computation`. They differ in
owner lifetime and requested downstream product, not in syntax providers, semantic stage order, or
recovery policy. Intermediate semantic queries are private to that owner. A caller can publish one
admitted revision, discover through its syntax provider, and demand one closed unit result; it
cannot demand or reorder a declaration, preparation, body, or finalization query.

## Responsibility Boundaries

- `nocter-workspace-revision` owns accepted document events and immutable source overlays.
- package resolution and workspace orchestration own physical roots, package topology, compilation
  demand, and the frozen discovery request for one revision.
- `nocter-computation` owns typed inputs and queries, dependency recording, invalidation, cycle
  detection, memoization, retention, and execution accounting without knowing compiler semantics.
- `nocter-compiler-computation` owns compiler-domain query keys, input publication, stage order,
  and the sole complete-or-incomplete analysis demand.
- declaration lowering and checking remain the sole owners of the semantic values computed by
  their queries.
- session translates one closed result into target construction or explicit recovery evidence. It
  cannot restart a compiler stage.
- analysis joins semantic evidence with source projection. Protocol code consumes only its typed
  results.

No side authority may become a second semantic pipeline. Workspace code cannot maintain a manual
invalidation graph, session cannot reconstruct a missing intermediate result, and an editor feature
cannot choose a fallback compiler phase.

## Revision and Identity

One `CompilerComputation` admits an atomic source view and returns an owner-bound revision token.
The token is required for syntax access and discovery. A token from another owner or an older
source view is rejected before semantic inputs can be published.

Physical paths are canonicalized before they become query keys. A semantic scope key identifies
the selected target and canonical root package identities. A body key identifies one canonical
source path plus its declaration-owned syntax locator. Generation-local `SourceId`, `NodeId`,
token, span, symbol, type-arena, and closure-arena identities never become cross-revision keys.

An immutable product may retain dense local identities only while it retains their exact owner.
Reusable body products encode lexical, type, closure, and source references as body-local recipes;
they do not claim that one generation's arena identities are valid in another.

## Input Views and Fingerprints

One discovered unit publishes inseparable but differently invalidated input views:

- the declaration view combines source-neutral discovery topology with canonical module surfaces;
- the exact-current view additionally includes every reached source's normalized bytes and current
  source-identity layout;
- each body view contains the exact normalized bytes below one stable declaration locator.

Syntax owns canonical declaration and body surfaces. Discovery owns canonical topology and current
source surfaces. Compiler computation composes their fingerprints once; consumers cannot assemble
parallel fingerprints from paths, rendered names, or semantic storage.

Accepted declaration, preparation, lexical, and typed-body products use source-neutral
fingerprints where their contracts permit reuse. Authored rejection, source projection, incomplete
syntax, canonical replay, and the final unit outcome remain exact-current. An unchanged result
fingerprint stops invalidation propagation even when an upstream query had to be re-evaluated.

A fingerprint is cache validation, not semantic authority. Its owning product still carries the
identities and decisions consumed downstream. Warm incremental results are compared with a fresh
computation of the same final source in conformance tests.

## Body Independence and Finalization

Program preparation freezes the declaration-owned semantic prefix before body work. Each lexical
and typed-body query opens from that same prefix and an empty body-local extension domain, so one
body cannot observe types, closures, symbols, or memoized facts allocated by a preceding sibling.
Preparation and the declaration recipe needed to reopen it are one checking-owned query product.
Compiler computation schedules its transition but cannot split the product and manually rejoin
generation-local bindings or source projection.

A successful body publishes source-neutral recipes for its lexical identities, inferred
structural types, closures, checked operations, and source occurrences. Finalization replays all
successful recipes once in canonical `BodyId` order. It then computes ownership, provenance,
loans, opaque witnesses, cleanup, and completed checked semantics once for the whole program.
Workspace and session receive only the finalized branch and cannot repeat replay or checking.

## Failure and Recovery

Recoverable authored failure is a query value, not a missing cache entry. Declaration,
preparation, lexical, and typed-body rejection retain the exact diagnostic and only the recovery
capabilities justified by that diagnostic. Successful sibling bodies may remain reusable, but a
rejected branch cannot commit semantic mutations.

Incomplete syntax uses one exact-current child query. The top-level unit query selects complete or
incomplete analysis before the result leaves compiler computation. Command, workspace, session,
analysis, and LSP layers therefore cannot assign different meanings to the same syntax hole.

Authored rejection and compiler-domain integrity failure are distinct query outcomes. A rejected
stage prevents its downstream query from being demanded, so "not reached" is control flow rather
than a stored semantic state. Integrity failures retain their original typed cause through the unit
product and session boundary; they never collapse into missing authority or become an authored
diagnostic.

Every semantic query also validates the capability required by its stage. The unit query remains
the sole scheduler, but correctness does not depend on that caller selecting a valid complete,
incomplete, declaration, preparation, body, or finalization transition.

## Source Projection

Reusable semantic products contain source-independent locator recipes. Materialization resolves
those recipes against the exact current syntax generation and constructs frontend bindings and
`SourceIndex` together. Documentation is read from current syntax during materialization, so a
trivia-only edit cannot retain stale presentation.

Source projection travels beside semantics and cannot participate in lookup, typing, dispatch,
ownership, reachability, ABI, or code generation. A failed semantic/source join is an integrity
error rather than permission to guess a range or publish partial editor data.

An exact-current body context owns current bindings and projection, but its successful equivalence
fingerprint remains tied to source-neutral preparation. Each body query separately depends on its
exact body source, and finalization separately depends on the exact-current scope. An unrelated
source edit can therefore reuse unaffected body recipes without permitting a stale final source
projection.

## Current Execution Contract

The computation database is in-memory and revision-pinned. It records actual query reads rather
than caller-maintained invalidation lists, detects recursive cycles, and retains a bounded window
of source revisions with each retained query's complete dependency closure. Background execution,
cancellation, persistent disk caches, remote indexes, and parallel backend scheduling are not part
of this boundary.

The completed migration and qualification evidence is preserved in the
[v0.20.0 Phase 1 review](../reviews/v0.20.0-phase-1.md). That review records historical findings;
this document defines only the resulting current contract.
