# Asynchronous Computation Boundary

This document defines the cross-responsibility contract for Nocter's asynchronous computation
model. Public syntax and observable behavior belong in `spec/`; milestone records own delivery
order and qualification evidence.

## Outcome

`async` is the explicit execution modifier for a deferred function or method. `future T` is the
separate owning structural type for one deferred computation that will eventually produce `T`.
Neither is an interface or hidden thread. Every structural future is safe to drive without a
synchronous external wait; the separate
[blocking-effect boundary](blocking-effect-design.md) owns how checking proves that invariant. An
asynchronous producer is written explicitly:

```nct
async func fetch(url: Url): String!
```

Calling `fetch` creates a lazy computation and returns immediately. Moving the value transfers the
only ownership of that computation. `await` consumes it and produces `String!`. Destroying an
unfinished value cancels its work and releases every captured or acquired resource exactly once.

The initial model is deliberately single-use and structured. It has no implicit copying, detached
task, hidden global executor, or background execution merely because a `future T` value exists.

## Type and Precedence

`future` is a unary type constructor whose operand is a complete type. Therefore:

- `future String!` means `future (String!)`;
- `(future String)!` means a fallible operation that immediately returns an asynchronous
  computation;
- `&future T` borrows the computation value;
- `future &T` produces a borrow when awaited.

The distinction between the last two forms is preserved in syntax, checked types, presentation,
serialization, and source maps. No later stage may recover it from source text.

The parsed declaration modifier is the sole input to callable execution classification. Result
types, aliases, and generic substitution never reclassify a declaration. For example,
`identity<T>(value: T): T` remains immediate when instantiated with `T = future U`; it transfers
an existing computation rather than creating a new one.

## Callable Execution Authority

Declaration checking assigns every callable body exactly one execution kind:

- **immediate** — invocation executes the body and returns its declared result;
- **deferred** — invocation captures the arguments and creates a computation, while awaiting or
  scheduling that computation executes the body and produces the inner result.

Declaration lowering records this choice once from the syntax-owned modifier. The checked callable
declaration is the sole downstream authority. Body checking, call checking, state-machine lowering,
diagnostics, and semantic presentation consume that decision. They cannot independently inspect
syntax, result shapes, aliases, or call-site substitutions to infer it.

A deferred body is checked against its declared result type. Thus `async func load(): String!`
returns or tail-produces `String!`, while calling it produces `future String!`. An explicit result
of `future String!` instead creates a nested `future future String!` call type. A future
tail-forwarding optimization may reuse or fuse computations only when these source-level types
remain observably identical.

Constructors, literals, coercions, operators, destruction declarations, tests, anonymous closures,
and primitives do not admit `async` in the initial model. Their result types never imply deferred
execution.

A compiler-authorized primitive may return a `future T` value immediately. Such a primitive
constructs an opaque computation through its target lowering and has no deferred Nocter body.
Callable execution therefore belongs to the declaration contract; it is not equivalent to the
outer shape of every callable result.

## Ownership and Lifecycle

A `future T` value has one lifecycle authority. Its abstract states are created, scheduled,
running, suspended, completed, cancelled, and consumed. Legal transitions are closed in the
checked and executable representations rather than inferred by the runtime.

- The value is not implicitly copyable, irrespective of `T`.
- `await` requires ownership and consumes the value.
- Storing or returning the value moves it under the ordinary ownership rules.
- Scheduling transfers execution responsibility to a scope-owned task while preserving one
  cancellation owner.
- Destroying unfinished work requests cancellation, removes reactor registrations, and destroys
  initialized frame fields exactly once.
- Destroying completed but unconsumed work destroys the stored result exactly once.

The initial runtime may represent every computation with an allocation-backed erased handle. That
is an implementation choice, not source semantics. Later escape analysis or frame placement may
remove allocations only when ownership, cancellation, address stability, and destruction remain
observationally identical.

## Captures and Result Provenance

The lifetime of the pending computation and the provenance of its eventual result are separate
facts.

A deferred invocation captures every argument needed to begin its body. Any borrowed argument
therefore constrains how long the pending computation may live, even when the awaited result is
storage-independent. Declaration checking records this **computation capture provenance** from the
checked parameter and capture model. Call checking maps it to argument origins. No source-visible
annotation is required because the capture follows mechanically from invocation ownership.

The existing `from` contract continues to describe only the produced value. For example,
`async func view(text: &Text): &str from text` produces a future whose awaited value is a borrow
derived from `text`. `from` does not describe scheduler storage, frame allocation, or the mere fact
that the computation captured `text` while pending.

These two provenance products have separate owners and consumers:

| Fact | Sole authority | Consumers |
|---|---|---|
| Pending computation captures | checked callable execution contract | call checking, region checking, frame lowering |
| Awaited result provenance | existing checked result contract | call checking, region checking, semantic presentation |
| Frame field liveness | suspension-aware executable lowering | MIR validation, cancellation cleanup |

## Suspension and State Machines

`await` is a consuming expression valid only in a body whose checked execution kind admits
suspension. Its checked result is the inner type of the consumed `future T`. The checker decides
ownership, result type, error and optional structure, live borrows, and cleanup obligations before
lowering.

Executable lowering receives explicit suspension points and produces a state machine. It may not
re-run name resolution, overload selection, interface solving, provenance inference, or liveness
from source. MIR derives representation-level liveness once from its own closed CFG; this is not a
second semantic liveness decision. A frame stores only values needed by a continuation or its
cancellation actions, together with checked conditional-initialization flags. Machine lowering
maps that exact product to machine identities and layouts without recomputing it.

The initial state owns captured inputs before the first resume. Each suspension publishes one
runtime-provided output edge and one ordered cancellation plan. Normal return completes with the
body result, and destruction of a completed but unconsumed computation uses its frozen output
plan. For `future T!`, both success and recoverable failure are completed values of type `T!`;
there is no parallel hidden scheduler-error channel. Runtime cancellation remains distinct from a
completed language-level failure.

Scheduler and reactor identities are stable values with generations. The scheduler owns task
lifecycle and runnable order; the reactor owns readiness registration. Neither may inspect the
other's private representation, and neither owns a pointer into movable task storage.

## Executor and Reactor Boundary

The target-independent scheduler owns generation-qualified task and registration identities. A
task is exactly one of runnable, running, waiting, completed, or cancelling. The scheduler is the
only component that changes these states or appends to the FIFO runnable queue. Computation frame
bytes and resume/cancel functions remain executor-owned payload; the scheduler never inspects
their layout.

The reactor accepts opaque descriptor-readiness and monotonic-deadline interests paired with a
registration identity. It may return stale native events, but an event can wake a task only while
the exact registration generation remains active. Waking one member consumes and deregisters the
complete wait set before enqueueing the task. Multiple events for the same wait set therefore
produce one resume. Operation code reattempts its nonblocking action after resume and decides the
observable readiness-versus-deadline outcome; the scheduler does not duplicate that policy.

Cancellation first invalidates and deregisters every wait identity, then publishes either pending-
frame cleanup or completed-output cleanup. Reactor deregistration is infallible at this boundary:
the target adapter normalizes interruption and already-removed native registrations internally.
Orderly shutdown uses the same cancellation transition for every retained task rather than
implementing a second cleanup path.

The Darwin host adapter maps these logical registrations to kqueue-backed events. One native
descriptor has one never-reused native token and may retain multiple logical readable or writable
registrations. Removing one logical waiter updates the native interest without hiding another.
Native closure and error observations wake the relevant logical direction so the resumed operation
can obtain the authoritative I/O result. A separate monotonic clock contract projects the earliest
fixed deadline to each blocking poll; spurious wakeups cannot restart it.

This Rust adapter is executable conformance evidence for the reactor boundary, not a library linked
into generated Nocter programs. The generated ARM64 runtime must satisfy the same observable tests;
host conformance cannot substitute for that release requirement.

## Native Frame Placement

Machine decides the exact union of stack objects, SSA values, initialization flags, and incoming
pack ownership that survives any suspension. ARM64 placement consumes that union once and assigns
each retained identity one stable byte range for the complete computation lifetime. Individual
suspension states select subsets of those identities; they cannot request a second layout or
overlay storage according to backend-recomputed liveness.

The allocation-backed frame header stores the resume entry, cancellation entry, completed-output
consume entry, lifecycle state tag, incoming allocation context, any required process context, and
any transferred pack pointer. The completed output has one separate stable field when its
representation occupies bytes.
Initial, suspension, and completed tags are assigned deterministically from the closed Machine
state list. A running state is not externally cancellable in the single-threaded executor, so it
does not require a second concurrently observable tag.

Ordinary stack frames and asynchronous heap frames use the same target object-placement authority
for alignment and overflow. The two layouts remain distinct products because a stack frame also
owns outgoing-call storage, saved registers, and an ABI frame record, while an asynchronous frame
must survive a return to the executor.

The runtime ABI schema is the numeric authority for the owning handle and the fixed header prefix:
handle size/alignment, resume/cancellation/consume entry offsets, state-tag offset,
allocation-context offset, header extent/alignment, and complete lifecycle-state tag sequences. A
computation supplies its suspension-state count; the schema assigns the initial, each suspension,
and completed tag. It also owns the uniform poll status and wait-interest record ABI. ARM64
placement begins after that prefix. It cannot redeclare these values or their arithmetic from its
own assumptions.

Resume receives only the opaque frame pointer. A pending result returns a pointer and count for
frame-owned wait-interest records; a completed result returns no interests. A descriptor interest
contains its descriptor and readable/writable direction, while a timer interest contains its fixed
monotonic deadline. Every record also retains a pointer to a computation-owned readiness cell.
The reactor signals only records that actually became ready; copying or forwarding a record keeps
the same cell identity. A computation may therefore be resumed after an unrelated member of a
composed wait set wakes without completing early. Nested `await` forwards the child's pending
records unchanged. On completion, the parent calls the child's consume entry with typed
destination storage. That entry moves the output and retires the child frame. Cancellation
similarly calls only the child's cancellation entry. Neither parent, executor, nor scheduler can
inspect a child's output offset or cancellation state.

## Structured Execution

The selected process entry is the first concrete execution owner. If `main` is `async` with
declared result `R`, the
compiler-generated process adapter invokes its constructor, owns the resulting root computation,
drives it to completion, consumes `R`, and applies the ordinary process-result policy. Entry
selection freezes immediate versus deferred execution once; MIR and Machine receive that fact and
cannot recover it from the result type. This special process boundary does not make ordinary
synchronous calls start an executor.

On ARM64 Darwin, the process adapter converts each pending ABI slice into one temporary `pollfd`
array and one relative timeout derived from the earliest fixed monotonic deadline. A single
`poll(2)` wait therefore preserves the wait set's OR semantics for descriptors and timers. An
interrupted call retries against the same interests and recalculates the relative timeout. After a
successful wait, the adapter writes only through the readiness pointers belonging to returned
descriptor events or elapsed timers; the temporary mapping is released before the computation
resumes. A target timeout narrower than the monotonic domain is only one wait segment: a zero-event
return rechecks the absolute deadline and repeats the wait while it remains in the future.
Deadlines use half-domain wrapping comparison, so a near-future deadline remains ordered across one
counter wrap. Invalid record tags, an empty pending set, native wait failure, and release failure
terminate through distinct compiler-owned trap reasons. The adapter never interprets computation
frame layout.

The initial compiler-owned descriptor-readiness and monotonic-deadline computations use the same
opaque header and interest-record schema as a generated deferred function. Each constructor writes
its distinct record payload, while both use one resume/cancel/consume lifecycle. The first resume
publishes the frame-owned interest; later resumes complete only after the reactor has signaled its
shared readiness cell. Cancellation and completed-output consumption retire the frame through
separate lifecycle entries. Cancellation is the destruction entry for every unconsumed owning
handle, including a completed handle. A constructor is declared only when the frozen Machine
program contains its primitive role; later lowering does not rediscover the dependency from source
spelling.

`std/task.join` and `std/task.race` use one compiler-owned two-child composition substrate rather
than a second executor. Each owns two child handles and polls them left to right. Pending records
from both children are copied into one dynamically sized contiguous set; their readiness pointers
still name the original child cells, so nested compositions preserve exact wake identity. Machine
lowering freezes only the two structural result offsets. ARM64 lowering consumes those offsets
without reopening semantic types or recomputing layout.

Join keeps both completed outputs inside their child frames until its tuple is consumed. Race
selects the first child that completes during ordered polling, immediately cancels the loser, and
keeps the winner output inside its child frame until consumption. The raw race result is the
structural tuple `(bool, T)`; ordinary standard-library code maps that private representation to
the public `Race<T>` enum, so the backend does not depend on a standard-library nominal type.
Cancellation accepts both pending and completed-but-unconsumed composition states and calls every
still-owned child cancellation entry exactly once.

The first task API is scope-owned. A scope cannot finish while its child work remains unconsumed;
normal exit joins it and exceptional exit cancels it. A task handle is an ownership value, not a
detached observation token. Detached execution is excluded until the language has an explicit
process-lifetime ownership and failure-reporting contract.

Creating a `future T` value is lazy. It does not run until consumed by `await` or transferred to a
structured scheduling operation. This keeps argument capture, cancellation, and start order
observable and deterministic.

## Independent Guarantees

`async` says that invocation creates deferred work whose body may suspend. `future T` owns that
work and guarantees that driving it does not synchronously wait for external progress. An immediate
callable may instead expose the positive `blocking` effect. The effect proof and primitive
classification belong to the [blocking-effect boundary](blocking-effect-design.md), not this frame
and executor contract.

This drive invariant does not imply bounded work, allocation freedom, or real-time suitability. A
computation may allocate or perform long CPU work without synchronously waiting. The checker must
keep execution, allocation, blocking behavior, and provenance as independent facts even though it
rejects a blocking edge inside deferred execution.

Under the initial uniform allocation-backed representation, a deferred producer cannot satisfy
`noalloc`: invocation must create its computation. Allocation performed later by the deferred body
remains governed by the body's checked operations. This restriction may be relaxed only after a
representation-independent allocation contract exists; it must not be hidden behind an optimizer.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| `async` declaration modifier | syntax tree | declaration lowering |
| `future T` syntax and precedence | syntax tree | declaration lowering, formatter, source projection |
| Structural future type identity | type store | checking, presentation, executable closure |
| Immediate or deferred callable execution | checked declaration | body checking, call checking, lowering, tooling |
| Callable blocking effect and future drive proof | checked effect authority | validation, semantic queries |
| Compiler-owned computation construction | selected primitive role and target lowering | native lifecycle helper |
| Immediate or deferred process entry | executable entry selection | process-root MIR, native process adapter |
| Captured argument origins | checked invocation contract | region checking, frame lowering |
| Suspension legality and consumed result | checked body | executable lowering, diagnostics, tooling |
| Frame fields and continuation liveness | MIR async-frame derivation | MIR validation, Machine projection |
| Cancellation order and initialization conditions | checked ownership cleanup | MIR async-frame derivation, Machine projection |
| Machine state and destruction identities | Machine projection | target backend, executor runtime |
| Wait-interest structure and numeric runtime ABI | runtime contract | scheduler, reactor, target backend |
| Task lifecycle and runnable order | executor | reactor adapter, runtime entry |
| Readiness and timer registration | reactor | executor wakeups |
| Native readiness mechanism | selected target adapter | reactor contract |

## Rejected Shortcuts

- `async T` is not retained as an alternate type spelling. It would let result syntax silently
  decide callable execution and recreate two authorities for the same fact.
- Callable execution is not inferred from call-site substitution, source spelling, or the presence
  of `await` in a body.
- `from current` is not exposed to describe frame or executor storage.
- A shared global executor is not started by ordinary synchronous calls.
- Literal-pack callbacks and trusted target-service calls are not reused as a general callback ABI.
- Blocking system calls are not relabeled as asynchronous merely because they run inside a
  deferred body.
- Cancellation cleanup is not implemented by dropping an opaque frame and hoping its private state
  matches the scheduler or reactor.

## Delivery Order

The language and compiler first establish one lossless future type, execution-kind fact, consuming
`await`, capture provenance, and explicit diagnostics. State-machine lowering follows only after
the checked product is closed. Executor and reactor implementation follows only after the
executable state contract is closed. The generated Darwin process adapter, descriptor-readiness
producer, and monotonic-deadline producer now consume the same wait-interest ABI. Native pipe and
elapsed-deadline conformance qualify both pending paths end to end. A public asynchronous time
contract uses the timer through bounded duration segments and a copied input value. Concurrent
TCP loopback conformance now qualifies descriptor readiness on a real socket. Public asynchronous
networking can now borrow receiver and buffer storage from a directly awaiting parent: loan
analysis freezes the stable source roots, target-independent frames preserve those roots, and
deferred ARM64 code accesses retained local storage at its persistent heap address. Escaping child
computations remain rejected by the ordinary provenance contract.

The first structured composition operations join two heterogeneous computations or race two
same-output computations without a detached task or global executor. Reactor-signaled readiness
cells prevent one child from completing merely because the other child's descriptor or deadline
woke the shared process wait. Native coverage exercises immediate completion, deterministic race
selection, different concurrent deadlines, cancellation before polling, cancellation of a nested
composition with one completed child, and concurrent public TCP connection and acceptance.

This order prevents runtime constraints from leaking backward into source semantics and prevents
the editor from implementing a partial asynchronous language independently of the compiler.
