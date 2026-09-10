# Asynchronous Computations

This chapter defines asynchronous producer declarations, future values, suspension, ownership, and
storage lifetime.

## Future Type

`future T` is an owning structural type for one lazy computation that eventually produces `T`.
The future is a value: it may be bound, stored in another sized value, moved, passed to a call, or
returned by an immediate callable.

```nct
let pending: future String! = fetch(url)
let response = await pending?
```

Every `future T` value is move-only, irrespective of `T`. Copying it would create two authorities
for starting, cancelling, or consuming the same work. `await` consumes the value directly, so its
canonical operand spelling does not add `move`. Using the binding again after `await` is an
uninitialized-place error.

An unfinished future is lazy. Creating it captures its invocation inputs but does not execute the
producer body. Awaiting it starts or resumes execution. Destroying an unfinished future cancels it
and releases its initialized captures exactly once. Destroying a completed but unconsumed future
destroys its stored output exactly once.

## Nonblocking Drive Invariant

Every `future T` is safe to drive on an executor thread. One drive step may compute, complete, or
publish readiness and deadline interests before suspending. It cannot synchronously wait for an
external actor, resource, clock, process, thread, or contended lock to make progress. This
invariant belongs to the structural future type, so storing, returning, joining, or otherwise
moving a future cannot erase it.

Suspension is not blocking. `await` transfers control after the awaited future publishes its
interests; a nonblocking syscall may report that it would wait and cause the computation to publish
the corresponding readiness interest. By contrast, a blocking descriptor operation, synchronous
name lookup, file operation, sleep, process wait, thread join, or waiting lock acquisition cannot
run while a future is being driven.

The invariant does not promise a time bound. Allocation, CPU-intensive work, and a loop with no
suspension are not synchronous external waits, although each may still make an application
unsuitable for latency-sensitive use. `noalloc` remains the independent allocation guarantee. A
future real-time contract would additionally require bounded work and target-specific latency
evidence and is not implied here.

## Producer Declarations

`async` is a declaration modifier. It makes an ordinary function or method a deferred producer:

```nct
async func fetch(url: Url): String! {
    // This body produces String!.
}
```

Calling `fetch` captures its arguments and returns immediately with `future String!`. The declared
result annotation is the output produced by the body, not the call expression's outer future type.
The body runs only when its future is driven.

Execution kind never follows from a result type, alias expansion, or generic substitution:

```nct
type PendingCount = future usize

func forward<T>(value: T): T { move value }        // always immediate
func forward_named(value: PendingCount): PendingCount { move value }
async func count_later(): usize { 0 }               // always deferred
async func nested(): future usize { count_later() } // call type: future future usize
```

Instantiating `forward` with `T = future usize` transfers an existing future. It does not turn
`forward` into an asynchronous producer.

`async` is admitted on ordinary function and method declarations, including interface method
contracts and default method bodies. A matching interface implementation must have the same
execution kind. Constructors, literals, coercions, operators, drop declarations, tests, anonymous
closures, and primitive functions do not admit the modifier in the initial model. A primitive or
ordinary immediate function may return `future T` when it constructs or transfers a future value
directly.

An asynchronous body is checked against the nonblocking drive invariant. It may call any helper
whose callable contract is nonblocking, but it cannot reach a `blocking`
callable directly, through interface dispatch, through a callback, or through implicit
destruction. The `blocking async` spelling is invalid rather than creating a second kind of future.

A structural callable type describes invocation behavior through its result type. For example,
`func(Input): future Output` is an immediately invoked callable that returns a future. It does not
imply that the callable declaration used `async`, and callable values do not expose a second
execution-kind dimension.

## Type Layering

`future` consumes a complete type operand:

```nct
future String!    // future (String!): awaiting produces String!
(future String)!  // immediate fallible value containing future String on success
future &T         // awaiting produces a readonly borrow
&future T         // readonly borrow of the future value
```

Outcome propagation follows `await`. `await operation()?` first consumes `operation()`'s future,
obtains its fallible output, and then propagates failure through the current asynchronous body.

## Suspension

`await` is valid only inside an `async` function or method body. Its operand must be an owned
`future T` value, and its expression type is `T`. A borrowed future cannot be awaited because the
borrower does not own its consumption authority.

Nested futures require one `await` per layer:

```nct
async func flatten(): i32! {
    await await nested()?
}
```

Control flow does not weaken this rule. Branches and loops use the same ownership joins as other
move-only values, so a future consumed on only one path is maybe initialized afterward.

## Process Entry

The selected top-level `main` may carry `async` when its declared result is one of the ordinary
accepted process results: `void`, `void!`, `i32`, `i32!`, `usize`, or `usize!`.

```nct
async func main(): i32! {
    let response = await fetch()?
    drop response
    return 0
}
```

The compiler-generated process adapter invokes the lazy producer, becomes the owner of that one
root future, drives it until completion, and then applies the same exit-status and error-reporting
rules as the corresponding immediate result. This is a process-boundary rule, not an alternate
calling convention visible to source code. Calling the same function normally still returns an
unstarted `future R` value.

A synchronous output, filesystem, resolver, process, or stream operation carries `blocking` and
therefore cannot be called from an asynchronous body. An asynchronous body must use an
executor-safe operation or finish its work before handing the result to synchronous code.

When the root future suspends, it supplies one or more descriptor-readiness or absolute
monotonic-deadline interests. The process adapter waits for any interest to become eligible and
then resumes the computation; it does not repeatedly poll a pending future. Readiness is only
permission to retry the suspended operation. The operation remains responsible for reporting
success, closure, timeout, or failure.

Monotonic deadlines use a wrapping counter and may name a future point at most half a counter
domain away. A narrower native timeout may split that wait internally but cannot make the deadline
eligible early.

## Captures and Result Provenance

A pending future retains every receiver and argument needed to begin its producer body. A borrowed
input therefore constrains how long the future may remain alive even when its eventual output is
storage-independent.

The `from` clause has a separate meaning: it constrains only the value produced by `await`.

```nct
async func inspect(source: &Buffer): usize
async func view(source: &Buffer): &str from source
```

Both futures retain `source` until completion. Only the output of `view` continues to depend on
`source` after the future has been consumed. Frame allocation, scheduler storage, and capture
retention never add an implicit source-visible `from` clause.

Borrowed storage owned outside the asynchronous call may remain live across suspension. Storage
owned by the asynchronous call may also be borrowed by the exact future it awaits:

```nct
async func fill(buffer: &+Buffer): void

async func receive(): Buffer {
    var buffer = Buffer.with_capacity(4096)
    await fill(&+buffer)
    return move buffer
}
```

Loan analysis records every local, owned parameter, owned capture, or expression temporary whose
address must remain stable at each suspension. That storage resides directly in the
allocation-backed parent computation frame; it is not copied through a temporary activation
address. Cancellation releases the awaited child before destroying the borrowed parent storage.

This is structured borrowing, not a general escape. A child future that retains a borrow of
parent-owned storage cannot be returned, stored outside that owner, or otherwise outlive the
parent. Such an escape is rejected by the ordinary result-provenance and loan rules.

## Structured Ownership

Asynchronous work remains under one lexical ownership authority. The initial model has no detached
task and does not start a hidden global executor from synchronous code. Scheduling transfers a
future into a scope-owned task; normal scope exit joins remaining children, and exceptional exit
cancels them.

`async` describes a producer body that may suspend; the produced structural future supplies the
nonblocking drive invariant. Under the initial allocation-backed representation, an `async`
producer cannot satisfy `noalloc`; immediate `noalloc` code may still move, store, or return an
already-created future.

## Synchronous Blocking Effect

An immediate callable that may synchronously wait writes `blocking`:

```nct
pub blocking func read_line_blocking(): String!
pub noalloc blocking func sleep_blocking(duration: Duration): void!
```

The modifier is an admission and an API warning, not a claim that every invocation necessarily
waits. Every unqualified named function, construction function, method, primitive, or interface
requirement promises that no reachable execution path waits synchronously. This rule also applies
to private helpers, so a declaration has the same effect when called directly or stored as a
callable value. Public contract/private body pairs write the same modifier on both sides.

Blocking behavior is transitive through direct calls, selected methods, interface dispatch,
callable values, and destruction. Literals, coercions, operators, expansion, and drop declarations
cannot be marked `blocking`; an implementation that reaches a blocking operation is invalid. A
nonblocking callable may be substituted where a `blocking` callable is accepted, while the reverse
would discard a required guarantee.
