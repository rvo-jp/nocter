# Asynchronous Computation Boundary

This document defines the cross-responsibility contract for Nocter's asynchronous computation
model. Public syntax and observable behavior belong in `spec/` after the implementation supports
them. The v0.41.0 milestone owns delivery order and qualification evidence.

## Outcome

`async T` is an owning structural type for one deferred computation that will eventually produce
`T`. It is not a callable modifier, interface, hidden thread, or synonym for nonblocking execution.
An asynchronous producer is written by returning that type:

```nct
func fetch(url: Url): async String!
```

Calling `fetch` creates a lazy computation and returns immediately. Moving the value transfers the
only ownership of that computation. `await` consumes it and produces `String!`. Destroying an
unfinished value cancels its work and releases every captured or acquired resource exactly once.

The initial model is deliberately single-use and structured. It has no implicit copying, detached
task, hidden global executor, or background execution merely because an `async T` value exists.

## Type and Precedence

`async` is a unary type constructor whose operand is a complete type. Therefore:

- `async String!` means `async (String!)`;
- `(async String)!` means a fallible operation that immediately returns an asynchronous
  computation;
- `&async T` borrows the computation value;
- `async &T` produces a borrow when awaited.

The distinction between the last two forms is preserved in syntax, checked types, presentation,
serialization, and source maps. No later stage may recover it from source text.

Type aliases are transparent during declaration checking. A declaration whose normalized result
has `async` as its outer constructor is an asynchronous producer. Generic substitution does not
reclassify a declaration later. For example, `identity<T>(value: T): T` remains an immediate
callable when instantiated with `T = async U`; it transfers an existing computation rather than
creating a new one.

## Callable Execution Authority

Declaration checking assigns every callable body exactly one execution kind:

- **immediate** — invocation executes the body and returns its declared result;
- **deferred** — invocation captures the arguments and creates a computation, while awaiting or
  scheduling that computation executes the body and produces the inner result.

The checked callable declaration is the sole authority for this choice. It determines `deferred`
once from the normalized outer result constructor in the declaration's generic domain. Body
checking, call checking, state-machine lowering, diagnostics, and semantic presentation consume
that decision. They cannot independently inspect syntax or substitute call-site types to infer it.

A deferred body is checked against the inner result type. Thus a body declared as
`async String!` returns or tail-produces `String!`, not another `async String!`. A future
tail-forwarding optimization may reuse or fuse computations, but cannot change this source-level
rule.

Constructors, literals, coercions, operators, and destruction declarations do not acquire
asynchronous execution accidentally from a nested type. Each declaration category must explicitly
admit the checked execution kind before it can produce a deferred body.

## Ownership and Lifecycle

An `async T` value has one lifecycle authority. Its abstract states are created, scheduled,
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

The existing `from` contract continues to describe only the produced value. For example, an
asynchronous view may return `async &str from text`; awaiting it yields a borrow derived from
`text`. `from` does not describe scheduler storage, frame allocation, or the mere fact that the
computation captured `text` while pending.

These two provenance products have separate owners and consumers:

| Fact | Sole authority | Consumers |
|---|---|---|
| Pending computation captures | checked callable execution contract | call checking, region checking, frame lowering |
| Awaited result provenance | existing checked result contract | call checking, region checking, semantic presentation |
| Frame field liveness | suspension-aware executable lowering | MIR validation, cancellation cleanup |

## Suspension and State Machines

`await` is a consuming expression valid only in a body whose checked execution kind admits
suspension. Its checked result is the inner type of the consumed `async T`. The checker decides
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
plan. For `async T!`, both success and recoverable failure are completed values of type `T!`;
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

The allocation-backed frame header stores the resume entry, cancellation entry, lifecycle state
tag, incoming allocation context, any required process context, and any transferred pack pointer.
The completed output has one separate stable field when its representation occupies bytes.
Initial, suspension, and completed tags are assigned deterministically from the closed Machine
state list. A running state is not externally cancellable in the single-threaded executor, so it
does not require a second concurrently observable tag.

Ordinary stack frames and asynchronous heap frames use the same target object-placement authority
for alignment and overflow. The two layouts remain distinct products because a stack frame also
owns outgoing-call storage, saved registers, and an ABI frame record, while an asynchronous frame
must survive a return to the executor.

## Structured Execution

The first task API is scope-owned. A scope cannot finish while its child work remains unconsumed;
normal exit joins it and exceptional exit cancels it. A task handle is an ownership value, not a
detached observation token. Detached execution is excluded until the language has an explicit
process-lifetime ownership and failure-reporting contract.

Creating an `async T` value is lazy. It does not run until consumed by `await` or transferred to a
structured scheduling operation. This keeps argument capture, cancellation, and start order
observable and deterministic.

## Independent Guarantees

`async` and a future `noblock` guarantee answer different questions:

- `async T` says that producing `T` may suspend and is represented as an owned computation;
- `noblock` will say that executing a callable cannot block its operating-system thread.

An asynchronous computation may initially contain blocking work, although doing so can stall a
single-threaded executor. A synchronous callable may be nonblocking. Neither fact implies the
other, and the type checker must not synthesize one guarantee from the other.

Under the initial uniform allocation-backed representation, a deferred producer cannot satisfy
`noalloc`: invocation must create its computation. Allocation performed later by the deferred body
remains governed by the body's checked operations. This restriction may be relaxed only after a
representation-independent allocation contract exists; it must not be hidden behind an optimizer.

## Responsibility Matrix

| Decision | Sole authority | Consumers |
|---|---|---|
| `async T` syntax and precedence | syntax tree | declaration lowering, formatter, source projection |
| Structural async type identity | type store | checking, presentation, executable closure |
| Immediate or deferred callable execution | checked declaration | body checking, call checking, lowering, tooling |
| Captured argument origins | checked invocation contract | region checking, frame lowering |
| Suspension legality and consumed result | checked body | executable lowering, diagnostics, tooling |
| Frame fields and continuation liveness | MIR async-frame derivation | MIR validation, Machine projection |
| Cancellation order and initialization conditions | checked ownership cleanup | MIR async-frame derivation, Machine projection |
| Machine state and destruction identities | Machine projection | target backend, executor runtime |
| Task lifecycle and runnable order | executor | reactor adapter, runtime entry |
| Readiness and timer registration | reactor | executor wakeups |
| Native readiness mechanism | selected target adapter | reactor contract |

## Rejected Shortcuts

- `async func` is not retained as an alternate spelling. It would make asynchronous values look
  like a declaration-only effect and create two surface authorities.
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

The language and compiler first establish one lossless async type, execution-kind fact, consuming
`await`, capture provenance, and explicit diagnostics. State-machine lowering follows only after
the checked product is closed. Executor and reactor implementation follows only after the
executable state contract is closed. Public asynchronous time and networking APIs follow only
after deterministic cancellation and stale-event tests pass.

This order prevents runtime constraints from leaking backward into source semantics and prevents
the editor from implementing a partial asynchronous language independently of the compiler.
