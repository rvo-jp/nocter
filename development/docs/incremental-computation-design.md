# Incremental Computation Boundary

This document owns the cross-crate computation contract that turns immutable source revisions into
compiler and editor products. Compiler stages remain the sole semantic authorities; the computation
layer may schedule, memoize, validate, and reuse their products but cannot implement a language
rule.

## Target Model

```text
workspace source revision
        |
        v
frozen workspace plan
        |
        v
revision-pinned computation snapshot
        |
        +--> parse(source)
        +--> module surface(module)
        +--> declaration graph(scope)
        +--> resolve body(body)
        +--> check body(body)
        `--> validate target(scope)
                    |
                    v
          semantic evidence + source projection
                    |
                    v
              analysis queries
```

The computation snapshot is a lazy view over one accepted input revision, not a requirement to
materialize every compiler product eagerly. A command may demand a target product; an editor query
may demand only the semantic capability it needs. Both paths invoke the same stage providers.

## Authority Boundaries

- `nocter-workspace-revision` owns open documents, monotonic revisions, and immutable overlays.
- workspace analysis owns physical path interpretation, package/module/single-file scope selection,
  and the frozen plan for one revision.
- the computation owner owns query keys, dependency recording, memoization, invalidation, cycle
  reporting, and reuse accounting.
- declaration lowering, checking, target construction, and later backend stages own semantic rules
  and return immutable products.
- session composition owns the only production and recovery stage order. One analyzed unit owns its
  discovery snapshot and exact session result inseparably.
- analysis owns the validated join between semantic evidence and source projection. It does not
  invoke compiler stages or inspect computation storage.
- protocol layers consume typed analysis results only.

## Identity

Physical paths are resolved before the computation boundary. Queries use opaque source, package,
module, declaration, and body keys. A query key is stable only in the domain stated by its owner;
generation-local arena IDs never become cross-revision cache keys.

Reusable module products may retain their own dense IDs. A dependent product can reuse those IDs
only while it retains the exact immutable owner product. Pointer identity alone is not a semantic
fingerprint.

## Dependency and Reuse

Dependencies are recorded by query evaluation. Feature code and workspace orchestration do not
maintain parallel downstream invalidation lists. A changed input makes its direct query dirty. A
dirty query is reevaluated only when demanded; if its deterministic output fingerprint remains
unchanged, invalidation does not propagate to dependents.

Examples:

- changing a function body invalidates that source parse and the affected body computations;
  unchanged module surfaces and unrelated bodies remain reusable;
- changing a public declaration invalidates the module surface and every computation that actually
  consumed it;
- changing an ordinary comment can update source projection without invalidating semantic
  dependents when their product fingerprints remain equal.

Every reuse decision is observable in tests through computation counters. Correctness tests compare
a warm incremental result with a fresh computation from the same final source revision.

## Source Projection

A stage product carries its semantic result, diagnostics, deterministic fingerprint, and source
projection contribution together. The semantic model remains source-independent, but stage APIs do
not provide a success path that silently drops the corresponding authored projection. The finished
`SourceIndex` remains an independent immutable relation and retains its whole-generation integrity
seal as defense in depth.

## Recovery

Complete and incomplete syntax are explicit query inputs to the same session stage graph. Recovery
does not create a second semantic database or feature-local fallback order. Rejected name and body
domains, typed interruptions, diagnostics, and complete/partial/unavailable coverage remain ordinary
cacheable products when their source justification is complete. Internal inconsistency is never
cached as authored recovery.

## Deferred Facilities

The first computation phase does not require background execution, request cancellation, persistent
disk caches, remote indexes, or parallel backend work. Those facilities may be added only after the
dependency graph and snapshot contract are proven. They must not introduce another semantic
authority.
