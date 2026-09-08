# Asynchronous Computations

This chapter defines deferred computation values, asynchronous producer declarations, suspension,
ownership, and storage lifetime.

## Computation Type

`async T` is an owning structural type for one lazy computation that eventually produces `T`.
The computation is a value: it may be bound, stored in another sized value, moved, passed to a
call, or returned through an enclosing non-async type.

```nct
let pending = fetch(url)
let response = await pending?
```

Every `async T` value is move-only, irrespective of `T`. Copying it would create two authorities
for starting, cancelling, or consuming the same work. `await` consumes the value directly, so its
canonical operand spelling does not add `move`. Using the binding again after `await` is an
uninitialized-place error.

An unfinished computation is lazy. Creating it captures its invocation inputs but does not execute
the producer body. Awaiting it starts or resumes execution. Destroying an unfinished computation
cancels it and releases its initialized captures exactly once. Destroying a completed but
unconsumed computation destroys its stored output exactly once.

## Producer Declarations

A function or method is a deferred producer when its normalized declared result has `async` as its
outer constructor:

```nct
func fetch(url: Url): async String! {
    // This body produces String!, not async String!.
}
```

Calling `fetch` captures its arguments and returns immediately with `async String!`. The body is
checked against `String!` and runs only when the computation is driven.

Aliases are expanded before this classification. Generic substitution happens afterward and
cannot change it:

```nct
type PendingCount = async usize

func count_later(): PendingCount { 0 } // deferred; body result is usize
func identity<T>(value: T): T { move value } // always immediate
```

Instantiating `identity` with `T = async usize` transfers an existing computation. It does not turn
`identity` into a deferred producer.

Only ordinary functions and methods admit deferred producer bodies. Primitive functions,
constructors, literals, coercions, operators, drop declarations, tests, and anonymous closures do
not acquire deferred execution from an `async` result. A structural callable result such as
`func(Input): async Output` describes an immediate invocation that returns a computation value; it
does not independently reclassify an unknown callable body.

A primitive may therefore declare an outer `async` result when its compiler implementation creates
an opaque computation directly. The primitive remains immediate: invoking it constructs the value,
and only `await` or a structured executor starts the represented work.

## Type Layering

`async` consumes a complete type operand:

```nct
async String!   // async (String!): awaiting produces String!
(async String)! // immediate fallible value containing async String on success
async &T        // awaiting produces a readonly borrow
&async T        // readonly borrow of the computation value
```

Outcome propagation follows `await`. `await operation()?` first consumes `operation()`'s
computation, obtains its fallible output, and then propagates failure through the current deferred
body.

## Suspension

`await` is valid only inside a deferred function or method body. Its operand must be an owned
`async T` value, and its expression type is `T`. A borrowed computation cannot be awaited because
the borrower does not own its consumption authority.

Nested computations require one `await` per layer:

```nct
func flatten(): async i32! {
    await await nested()?
}
```

Control flow does not weaken this rule. Branches and loops use the same ownership joins as other
move-only values, so a computation consumed on only one path is maybe initialized afterward.

## Process Entry

The selected top-level `main` may return `async R` when `R` is one of the ordinary accepted process
results: `void`, `void!`, `i32`, `i32!`, `usize`, or `usize!`.

```nct
func main(): async i32! {
    let response = await fetch()?
    io.print(response)
    0
}
```

The compiler-generated process adapter invokes the lazy producer, becomes the owner of that one
root computation, drives it until completion, and then applies the same exit-status and error
reporting rules as the corresponding immediate result. This is a process-boundary rule, not an
alternate calling convention visible to source code. Calling the same function normally still
returns an unstarted `async R` value.

When the root computation suspends, it supplies one or more descriptor-readiness or absolute
monotonic-deadline interests. The process adapter waits for any interest to become eligible and
then resumes the computation; it does not repeatedly poll a pending computation. Readiness is only
permission to retry the suspended operation. The operation remains responsible for reporting
success, closure, timeout, or failure.

## Captures and Result Provenance

A pending computation retains every receiver and argument needed to begin its producer body. A
borrowed input therefore constrains how long the pending computation may remain alive even when
the eventual output is storage-independent.

The `from` clause has a separate meaning: it constrains only the value produced by `await`.

```nct
func inspect(source: &Buffer): async usize
func view(source: &Buffer): async &str from source
```

Both pending computations retain `source` until completion. Only the output of `view` continues to
depend on `source` after the computation has been consumed. Frame allocation, scheduler storage,
and capture retention never add an implicit source-visible `from` clause.

Borrowed storage owned outside the deferred call may remain live across suspension. A borrow into
the deferred call's own local, owned parameter, captured owned value, or expression temporary may
not cross `await`, because doing so would make the movable computation frame self-referential.
Such a program is rejected before executable lowering.

## Structured Ownership

Asynchronous work remains under one lexical ownership authority. The initial model has no detached
task and does not start a hidden global executor from synchronous code. Scheduling transfers a
computation into a scope-owned task; normal scope exit joins remaining children, and exceptional
exit cancels them.

`async` describes the ability to suspend. It does not promise that the body avoids blocking its
operating-system thread. A future `noblock` guarantee will express that independent property.
