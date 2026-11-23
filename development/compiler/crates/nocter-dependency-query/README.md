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
A step may instead request `Compute`, delegating the already-active key to `compute_current`.
That bounded computation can recursively resolve other roles, including iterative requests,
through the same query; it must not reopen its current key. The default callback rejects an
unsupported computation role through `no_progress`. Both entries share memoization and cycle
detection across these transitions. `Require` groups use no host recursion; consumers must bound
recursive computations themselves.

A draft returns a complete value or a group containing at least one unfinished key. Completed
dependencies in a group are skipped. Empty or wholly completed groups invoke the consumer's
`no_progress` failure, preventing an invalid draft from spinning. The query alone constructs
active-cycle paths; consumers classify their own authored and integrity failures. Failure unwinds
only the invocation's unfinished frames, retaining completed dependencies and outer active work.
`dependency_failed` projects each failed child at the parent boundary as those frames unwind,
including the edge that closes a cycle. The default preserves the exact error. Domain consumers
may attach authored causes or classify that cycle without maintaining their own stack; the query
still owns detection, frame removal and successful dependency evidence. A failure of the requested
root has no parent callback inside that invocation.

A `RequireOne` step asks for one completed prerequisite before the draft advances again.
Within the same iterative invocation, a cyclic candidate can give way to the next candidate.
The query removes that candidate's unfinished frames, calls `dependency_abandoned` for their
bookkeeping and keeps all completed facts. A successful child returns control to the draft for
semantic selection. Computation failures propagate through the normal failure callbacks, and
the last active cycle propagates when the group runs out of candidates.

The read-only active-key path lets consumers classify request roles for an execution-depth budget
without another stack; the query does not interpret those roles. Each completed value is allocated
once and shared across requests. The query has no `Clone` implementation, so a construction cannot
fork a second memoization authority or copy a large frozen aggregate on each edge.

Successful resolutions and reads of `completed` record the selected fact as a prerequisite of the
current active computation, including cache hits and read-only progress. Incomplete reads grant no
fact. Consumers must request missing required work rather than complete from its absence. Failed
frames discard their edges; completed siblings remain independent and a retry records its own
requirements. One completion-order arena owns values and direct prerequisite indices. Every edge
points backwards, duplicates are collapsed, and consuming `into_dependencies` freezes that flat
DAG without another lookup/cache authority or recursive destruction. `into_completed` projects the
same values when a consumer does not need prerequisite evidence. Tracking uses construction-local
interior mutation only; published nodes and values are immutable.
