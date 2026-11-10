# nocter-dependency-query

## Responsibility

Own construction-scoped dependency state, shared immutable memoization and explicit request
progress without owning compiler semantics, source evaluation or revision tracking.

## Contract

Domain owners supply hashable keys, completed values, computations and authored/integrity failure
classification. The query owns absent/active/complete transitions and cycle paths; it cannot infer
names, types, eligibility, source causes or execution limits. A construction retains one authority
and pairs keys with its own domain. Cross-revision inputs, fingerprints and invalidation belong to
[nocter-computation](../nocter-computation/README.md).

## Invariants

One `DependencyQuery` owns absent, active and completed states and the exact ordered active frames.
`resolve` supports re-entrant computations. `resolve_steps` instead advances drafts through
`DependencyProgress`, scheduling each returned dependency group in order with explicit frames.
Both entries share memoization and cycle detection, including when iterative work runs inside a
re-entrant computation. Deep iterative dependencies do not consume the host call stack.

A draft returns a complete value or a group containing at least one unfinished key. Completed
dependencies in a group are skipped. Empty or wholly completed groups invoke the consumer's
`no_progress` failure, preventing an invalid draft from spinning. The query alone constructs
active-cycle paths; consumers classify their own authored and integrity failures. Failure unwinds
only the invocation's unfinished frames, retaining completed dependencies and outer active work.

The read-only active-key path lets consumers classify request roles for an execution-depth budget
without another stack; the query does not interpret those roles. Each completed value is allocated
once and shared across requests. The query has no `Clone` implementation, so a construction cannot
fork a second memoization authority or copy a large frozen aggregate on each edge.
