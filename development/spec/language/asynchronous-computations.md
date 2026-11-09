# Asynchronous Computations

## Scope

This chapter owns the adopted v2.0.0 lazy future, producer, await, dependency and cancellation
contracts. [Execution effects](execution-effects.md) own inferred waiting and other effect facts;
[origins](memory-and-regions.md#edited-state-origins) own post-state bounds. These rules do not
admit async syntax to the bounded private compiler or establish an executor, physical ABI,
native lifecycle, package entry adapter or standard-library task protocol.

## Future Values and Producers

`future T` is one owned lazy computation delivering an ordinary T value. It is move-only for all
T, including void and Copy outputs. Transfer carries its sole drive, output and cancellation
responsibility. It cannot be driven or destroyed concurrently through aliases. Creating it
neither starts a detached task nor executes the producer body.

```text
async func add_later(take value: i32): i32 {
    value + 1
}

async func example(): i32 {
    let pending: future i32 = add_later(41)
    await pending
}
```

`async` is permitted on named module functions, type-owned construction functions and methods,
including interface requirements/defaults. A construction function's deferred payload still
constructs Self. Complete matched implementations and interface bindings retain execution kind.
Type aliases, result types and generic substitution cannot make an immediate declaration async.
`const async`, async literals/operators/coercions/destructors/tests, anonymous async closures
and async primitives are not introduced. An immediate callable can return an existing future.

The declared result is the body's output: `async func load(): File!` has call type
`future (File!)`. `future` consumes a complete operand; `(future File)!` instead means an immediate
fallible value containing a future. An async body returning future T creates nested futures,
without flattening. Structural `func(Input): future Output` describes immediate invocation that
returns a future, without a second async flag on its callable type.

Every produced future has checked non-waiting drive and cancellation/release evidence. Laziness
or an intention to drop it unused cannot justify unknown callback/backend effects. Conditional
generic/static-witness evidence follows the effect contract, without authored blocking qualifiers.

Equal output types do not equate producer identities, captures or state transitions. Any future
join, container or erased boundary needs a sound combined dependency/effect/cleanup schema;
future T is not permission to select code from T or assume empty footprints. General joining/
representation rules need their own admission evidence, just as forwarding must preserve an
already established schema.

## Construction and Input Commit

Evaluate/prepare receiver and inputs in ordinary source order under their read/edit/take
operations. Complete required backing before the producer's invocation transfer/activation
commits. Interrupted preparation cleans completed snapshots/temporaries and releases reservations;
it does not restore transfers already performed by nested argument computations.

At commit, a take input receives its snapshot/value, while read/edit inputs retain their outside
accesses in the future. Edit delegation begins at creation, not first await. The caller cannot
conflict with retained roots while needed. Storing, forwarding and cancelling preserve the same
dependency/cleanup schema; plain future T syntax does not erase its concrete input transitions.

A read input is not snapshotted merely because its data is Copy. A fresh read operand belongs
to the caller's evaluation frame under the ordinary temporary rule. Immediate await can retain
that frame through suspension; storing a future beyond a needed temporary's evaluation extent
is invalid. Name an owner with sufficient duration or declare intentional value receipt with
take. Do not transfer a hidden temporary owner into the future to repair a read call.
Physical scalar transport may use registers or copies while preserving that source access
contract; it does not grant snapshot semantics or remove the required logical root.

Automatic backing cannot add an unadvertised outer fallible layer or move a required construction
request into the producer body. Its managed-storage failure policy belongs to storage/platform
contracts. Body throw fails the driven output, not construction. An explicitly recoverable
immediate factory declares a result such as `(future T)!` and follows its own ordinary commit/
resource rules. The compiler cannot invent that API or restore its already committed take inputs.

## Await and Suspension

Await is valid in the nearest async function/method/default body; a nested ordinary closure does
not inherit permission. The operand supplies one owned future T. A read/edit alias cannot
surrender another owner's future. No transfer marker is added. Await consumes that instance and
delivers T on normal completion, preserving outside dependencies.

One await eliminates one future layer. `await operation()?` awaits first and propagates its
output outcome through the current async result. An immediate outcome containing a future instead
needs grouping, as in `await (make_future()?)`. Each additional future/outcome layer retains its
own boundary.

A drive step computes, completes, or suspends under the checked wait-interest protocol. Calls,
argument computation, child construction, normal cleanup and backing/release operations all
satisfy the external-progress rule. Allocation alone is neither a wait proof nor a non-waiting
certificate. Readiness permits retry; it does not mean completion. No hidden worker thread or
polling executor repairs a failed proof. Exact interests, scheduling, entry and task composition
require their own platform/library contracts.

Outside storage may remain accessed across suspension when its roots survive. Producer locals/
captures accessed by an awaited child need stable storage while needed. Preserve logical identity
and cancellation order, without a universal heap/pinning requirement. A child depending on
parent-owned storage cannot escape that owner because of its future type or field-free layout.

## Completion and Cancellation

Normal completion stages output, completes remaining producer cleanup and transfers that output
exactly once when consumed. A ready future owns unconsumed output and its dependencies. Retirement
cannot invalidate outside roots needed by delivered output; output into destroyed producer-owned
storage is invalid.

Dropping an unstarted/pending future cancels it, without executing unstarted body statements or
synthesizing a result. Pending parent cancellation first releases an active child and its loans,
then parent resources they required. Ready-output disposal destroys the output before releasing
its needed roots. Follow checked cleanup/dependency order within each phase, without duplicate
owners/destructors.

Unstarted cancellation still destroys received owned inputs/captures. Their own cleanup may
update outside state even though no producer-body statement ran. Preserve those schema-provided
transitions and check affected edit bounds; unstarted does not mean effect-free.

Cancellation occurs at a quiescent state, not concurrent preemption inside a drive step. Every
actual suspension state must admit cancellation cleanup and leave edited outside places fully
initialized. Completed edits/effects are not rolled back. Trap/divergence remain separate exits;
no unwinding guarantee is invented.

Apply the [origin contract's phase relations](memory-and-regions.md#deferred-completion-and-cancellation-bounds)
at their actual lifecycle boundaries: completion after producer cleanup, pending cancellation
after child/local cleanup, and ready disposal through output cleanup. That chapter owns the
exact dependency envelopes and their defaults. Check every cancellable state and both pending/
ready branches; output ownership alone supplies no empty outside-write proof. No transition may
leave access into destroyed frame, child or guard storage.

```text
// Assume Holder has checked dependency slots and an immediate, non-failing clear operation.
async func forget(edit holder: Holder from static): void {
    holder.clear()
}
```

The selected clear implementation must remove dependencies and satisfy effect requirements;
its name grants none of those properties.

Completion establishes static-independent state. Unstarted cancellation preserves the original
state. After await use the completion bound with the delivered output's live obligations; after
dropping the future use its composed cancellation envelope. This void-output example has no
additional output-write footprint. Do not assert completion on unstarted cancellation, run clear
eagerly or introduce a hidden stronger incoming precondition.

Output from describes only the delivered value, separately from retained inputs and edited-state
transitions. An independent numeric output can detach after completion even though its pending
future retained its input. The future schema carries completion/cancellation write footprints
through wrappers and forwarding. A bodyless/erased boundary needs that checked evidence before
admission, without a second body lookup.

## Outcomes and Resource Completion

Throw, optional/fallible propagation and recovery in an async body operate on its driven output.
Await delivers a source outcome whose later loss follows
[outcome diagnostics](outcomes.md#failure-decisions-and-resource-completion). Creating an unstarted
future produces no body failure to handle.

Cancellation relinquishes the requested output under its lifecycle contract, including ready
output disposal. It is not plain discard of an already delivered source outcome. It does not
report resource-protocol success, propagate abandoned output through its cancelling caller or
run the remaining body to finish work. Task/library composition must document which child
outputs it consumes or abandons. It cannot discard a delivered failure as ordinary cleanup to
evade the warning. Exact combinator APIs remain separate library migration work.

## Frame Storage

The future [storage rules](memory-and-regions.md#storage-optimization)
permit register, embedded, stack, or heap representations when they preserve the computation's
logical ownership, access, dependencies, stable observations, and cleanup. There is no universal
heap-frame or user-authored pinning requirement. This does not change the released representation
and effect contracts or qualify an executable future representation.

Eliminating physical frame or capture fields does not release an input dependency retained by the
selected public contract, erase a value output's outside dependencies, change source availability, or make
access into a destroyed local valid. Pending and ready computations retain the dependencies needed
by their output and cleanup. Cancellation preserves child-before-owner cleanup; an unconsumed ready
output is destroyed before its dependencies are released.

Complete actual required construction storage before invocation-time transfer commits. A private
optional heap request may disappear under the storage contract, but a required recoverable/resource
failure cannot move into drive or cancellation. Infer waiting and allocation through the actual
chosen backing strategy; no authored `blocking`, `noalloc`, or `notrap` substitute is introduced.
An unproved drive-safety obligation remains a validity error rather than a hidden worker thread
or an assumption that an unknown backend is safe.

Combining backing blocks does not itself combine tasks, change poll/tie order, omit cancellation,
or turn cancellation into successful completion of a resource protocol. Task composition's
observable schedule and effects remain governed by its selected contract. Stronger coroutine fusion
needs a separate effect/cleanup argument, not just a proof that frame bytes can be reused.
