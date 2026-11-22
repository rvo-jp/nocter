# Inferred Execution Effects

## Source Contract

Nocter v2.0.0 has no authored `noalloc`, `notrap` or `blocking` modifier on declarations,
callable types, interface operations, generic requirements, primitives or destruction. Equivalent
attributes, effect parameters and renamed source guarantees are not substitutes. `async` and
`const` remain staging contracts, not claims of allocation freedom, purity or bounded work.

The compiler derives effects from selected operations and cleanup, independently of input
operations, Copy and nominal names. Platform facts, capability schemas, erasure and executable
realization require their own checked evidence before admission. The
[compiler pipeline](../../architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
records implemented scope.

## Facts and Phases

Keep resource requests, runtime-managed backing requests, recoverable failure, safety traps,
immediate termination, synchronous external waiting, divergence, observable reads/writes,
dependencies and cleanup separate. Absence of one effect proves none of the others.

Private error/frame/environment storage can allocate without requesting a Nocter allocator;
include that storage in actual backing facts. A move-only type may have allocation-free,
non-waiting cleanup; Copy is not a substitute for analyzing it. Trap freedom proves neither
termination nor purity. A name cannot justify removing callbacks, resource release or termination.

For future-producing operations distinguish immediate construction, drive, normal completion
cleanup, pending cancellation and ready-output disposal. Async construction does not execute
the body, but argument evaluation and required backing execute in the caller. Constructing a
child future inside an async body is part of its parent's drive: deferring the child's body does
not hide a waiting constructor. Returning an existing future does not give an immediate helper
the producer's body effects. Retiring a frame/output still has cleanup effects.

Allocation latency alone does not classify an external wait. Conversely, a locking allocator,
release backend or callback cannot be assumed non-waiting merely because it is called allocation
or cleanup. The [storage contract](memory-and-regions.md#storage-optimization) preserves observable
requests/failure checkpoints and requires revalidation after changing physical backing/release.

## Conditional Evidence and Admission

Analyze checked bodies over their selected operations, transitions and cleanup. Preserve generic/
callback conditions on type schemas, witnesses, adapters and established control/value facts
in the compiler product; discharge them for every admitted concrete instance. Do not retype a
body, reselect an operation or recover
generic Copy from a concrete caller to obtain a favorable effect.

An async template with a formal callback can retain a conditional drive obligation without a
new source qualifier. Every actual produced future must establish that obligation before
admission, even when never awaited. A known waiting operation that cannot satisfy the producer
contract is invalid at its source; uncertainty at an actual witness identifies that use and
the originating operation. Interrupted/unproduced results acquire no fictitious environment or
effect certificate. Structural errors remain separate from actual value production.

Unknown is not effect-free. Erasure needs a sound retained target/effect/cleanup envelope;
interface calls use frozen bindings or conditional requirements; primitives need exact authorized
declaration/backend certificates. A spelling, package path, empty representation or bodyless
signature grants no trusted fact. Changed bodies, witnesses, destructors and backends invalidate
dependent summaries and proofs.

Not proving an optimization normally retains runtime work/checks; it does not reject an otherwise
valid immediate program. Required validity obligations instead must hold: memory, initialization,
origins, cleanup, const eligibility and future drive/cancellation safety cannot be replaced with
a runtime fallback that changes semantics. No hidden thread, scheduling, clone or changed input
operation repairs a failed proof.

## External Progress and Cycles

Synchronous waiting requires an external actor, resource, clock, thread or contended lock to make
progress before returning control. A valid suspension instead returns control and publishes its
resume interests. Future drive and cancellation/release paths must not synchronously wait.

The proof is not a union of known blocking primitive names. A loop or recursive cycle can poll
externally changing state without suspension even when each observation returns immediately.
Such a cycle needs an argument that it returns control independently of external progress, or
explicit suspension through its protocol. A finite bounded observation/computation sequence can
establish that argument; a zero-allocation/call count cannot. Unknown dependence remains a
possible wait.

A CPU-only loop/recursion can diverge without an external wait. Non-waiting promises no
termination, fairness, deadline, bounded CPU work or real-time allocation latency. Keep these
facts separate rather than banning all loops or accepting all polling. Conservative rejection
must identify the uncertain cycle/target; no complete decision procedure is promised.

## Authority and Presentation

Compose checked summaries through recursion, substitution, invocation adapters, retained state
and cleanup. Storage/target choices can refine physical operations under the same semantic
contract but cannot introduce a forbidden wait or defer an observable resource failure. Typing
and dispatch remain frozen; physical validation does not repeat them.
Precomputation/inlining can remove physical work only while preserving logical staging, source
availability, observable effects and failure timing. It cannot advance a body trap to lazy
construction or recategorize a delivered output as undelivered to suppress its handling obligation.

Inferred reports are separate from authored signatures. Removing authored effect promises means
a body change can invalidate a safety-sensitive caller without changing the source header.
Relate that caller, selected operation and cause; do not manufacture source modifiers as repairs.
Hard build-budget policy, solver qualification and exact platform certificates remain separate
work without mandatory function annotations.
