# Structured Process I/O Boundary

This document owns the cross-component design for child-process ownership, executor-safe process
completion, and synchronous and asynchronous process pipes. Exact public declarations and
observable operation behavior belong to the checked `std/process` source and its behavior guide.
Target syscall constants belong to the selected target implementation. General future, task,
blocking, and byte-stream semantics remain in their existing language and standard-library
authorities.

## Problem

Before v0.49.0, `std/process` offered only closed synchronous `status` and `output` operations. Its
private command-I/O session owned fork, exec reporting, finite input, concurrent output draining,
failure precedence, descriptor cleanup, and exact-child observation, but callers could not compose
a child with the executor, transfer bytes incrementally, apply generic buffering, or race process
completion against another computation.

Adding only asynchronous `status` and `output` wrappers would leave the process model incomplete.
It would also tempt the implementation to run `waitpid` or the existing blocking poll loop while
driving a `future T`. Adding public pipe values without an abandonment authority would be worse:
ordinary destruction or cancellation could leave a live child or zombie process behind.

Before v0.49.0 Phase 1, the generated ARM64 Darwin process root converted descriptor and timer
interests into a temporary `pollfd` array. `poll` could not observe process completion. Periodic
`waitpid` probes, blocking waits, and feature-specific sleeps remain rejected because they
introduce latency, consume CPU, and create a second scheduling policy outside the reactor contract.

## Adopted Public Shape

`Command` remains the sole owner of an unlaunched request. Process-I/O selection is explicit at the
spawn boundary rather than encoded through unrelated command mutations. The intended public shape
is:

```nct
var io = ProcessIo.inherit()
io.stdin(Stdio.pipe)
io.stdout(Stdio.pipe)
io.stderr(Stdio.pipe)

var child = await command.spawn(move io)?
let stdin = child.take_stdin() otherwise { return missing_pipe() }
let stdout = child.take_stdout() otherwise { return missing_pipe() }
```

`Stdio` has exactly `inherit`, `null`, and `pipe` modes. `ProcessIo` owns one mode per standard
stream and provides named constructors for the common inherited and fully piped configurations.
The exact constructor and mutator spellings are fixed in `std/process/index.nct`, not here.

One `Child` owns one live or cached-terminal child identity. A configured pipe initially belongs to
that `Child`; a `take_*` operation transfers one endpoint exactly once. `ChildStdin` implements the
canonical asynchronous `Writer` and explicit `BlockingWriter` contracts. `ChildStdout` and
`ChildStderr` implement `Reader` and `BlockingReader`. They therefore use the existing generic
buffer and byte-copy algorithms without a process-specific buffering path.

Canonical process operations are executor-safe and unqualified. Synchronous twins use the
`_blocking` suffix. Closed `status` and `output` operations are rebuilt as convenience operations
over the same launch, endpoint, and observation authorities; they are not a second child-process
implementation. Compatibility aliases for the old synchronous names are not retained.

## Ownership States

The process lifecycle has these semantic states:

1. `Command` owns validated source configuration but no target resource.
2. launch preparation owns every stable argument, environment, path, descriptor, and report
   address before child creation;
3. a launch attempt owns either no child or one exact created child plus every parent endpoint;
4. successful exec reporting produces one `Child` owner and zero or more endpoint owners;
5. nonwaiting observation either retains the pending owner or reaps once and records one terminal
   result inside that owner;
6. consuming observation returns either the cached result or reaps and records the exact child;
7. destruction of a still-unobserved `Child` transfers the child to the abandonment authority;
8. the abandonment authority terminates and reaps that exact child before releasing its record.

A raw PID is an operating-system subject, not an ownership proof. The private standard-library
owner pairs one PID with unique lifecycle authority and keeps that authority until observation or
abandonment. Darwin cannot reuse the PID while its terminal status remains unreaped. Reactor
registration separately uses a generation-qualified logical identity, so replacing a registration
cannot validate an earlier event even when it names the same still-unreaped process.

Pipe endpoint ownership is independent after transfer, but process observation remains unique.
Closing a pipe never implies child completion, and child completion never implies that all bytes
from inherited descendant endpoints have reached EOF. Generic I/O code observes only endpoint
progress and EOF; it cannot inspect process state.

## Executor-Safe Launch

Launch preparation completes before `fork` and owns stable path, argument, environment, stream,
and report storage. Pipe creation, descriptor normalization, null-device opening, fork, child-side
descriptor installation, directory change, and executable replacement cross closed target
operation roles. These roles describe exact transitions rather than exposing a generic syscall to
an async body, and none waits for progress from another process.

The parent immediately wraps the returned PID and every configured endpoint in standard-library
owners. It then reads the nonblocking close-on-exec report. Canonical `spawn` suspends through the
descriptor reactor when the report is not ready; `spawn_blocking` uses the synchronous readiness
adapter. Both paths consume one read-attempt classifier, one payload decoder, and one public
failure selector. Cancellation destroys the future frame, which closes report and stream owners
and transfers the child owner to abandonment. The compiler never inspects those owners.

## Cancellation and Abandonment

Cancelling a future that merely borrows a `Child` removes its wait registration and returns control
with the `Child` still owned by its caller. Cancelling a closed operation that owns its internal
child destroys that owner and therefore enters the same abandonment path as ordinary `Child`
destruction. There is no separate timeout cleanup rule.

The abandonment operation must not synchronously wait on an executor thread. The standard-library
owner passes only its exact unreaped PID through a closed primitive contract. The Darwin target
service first sends the forced-termination signal on the calling thread, then transfers the PID to
a private dispatch worker that performs the sole blocking reap. Sending termination before transfer
ensures that immediate parent shutdown cannot leave a live child; the operating system adopts the
already-terminated child if the private worker cannot finish before process exit.

Allocation, dispatch transfer, and reaping invariants are fail-stop. A transfer either succeeds
completely or terminates the parent process; continuing after losing the only reaping owner is
forbidden. The service cannot inspect `Command`, `Child`, source types, standard error values, or
future frames. Its hidden cleanup allocation is target bookkeeping and does not use or alter a
Nocter allocation context.

Normal programs pay no background-cleanup cost after explicit observation. The abandonment service
exists solely to make destruction and cancellation safe. Its implementation strategy is owned by
the target service contract and may change without changing `std/process` declarations.

The repository `subprocess-pipeline` package is the reference composition of these boundaries. It
runs generic `io.copy`, three independent `Reader` collections, and two exact waits beneath one
structured timeout. It needs no pipeline object, process-specific progress loop, or secondary
lifecycle registry. Cancelling the parent computation destroys the same endpoint and child owners
that ordinary lexical destruction would destroy.

## Reactor Extension

The target-independent wait vocabulary gains a process-completion interest beside descriptor and
timer interests. The runtime contract remains the only authority for its semantic variant, numeric
record tag, subject field, validation, and ABI encoding.

The host Darwin reactor and generated ARM64 Darwin process root must both implement that contract.
The generated root moves from a `poll`-specific mapping to a Darwin event mapping capable of
representing descriptor readiness, monotonic deadlines, and process exit in one blocking wait.
Target code owns kqueue filters, flags, record layout, syscall numbers, interrupted-wait retry, and
native registration cleanup. Operation code still owns the interpretation of readiness after its
future resumes.

Darwin rejects a new process filter after the process has already exited, even while its status
remains unreaped. Process registration therefore treats the native missing-process result as
immediate readiness only after validating the semantic process record. Status remains untouched
for the unique `Child` observer. This closes the exit-before-registration race without a periodic
probe or a second status decoder.

The reactor wakes computations; it does not reap children. `std/process` or the abandonment service
performs the one exact observation transition through target process operations. This separation
prevents native event delivery from becoming a second exit-status decoder.

## Closed Operations and Streaming

`status` inherits standard output and error. `output` owns finite input and captures both output
streams through the same endpoint types exposed by streaming spawn. After the shared launch path
produces a child and one close-on-exec fact, the asynchronous closed operations advance stdin,
stdout, stderr, and child observation through structured tasks without waiting for one bounded pipe
to finish before servicing another. Their blocking twins use the same launch, endpoint, and child
owners; only blocking output adds one three-direction readiness coordinator. That coordinator
cannot create or observe a child and does not implement another byte-transfer classifier.

When a caller takes output endpoints, it also takes responsibility for driving or closing them.
The safe convenience path remains `output`; advanced streaming code composes endpoint futures and
`Child.wait` through structured `join`, `race`, and timeout operations. Destruction remains safe
even when application progress is incorrect: a stalled pipeline may fail to complete, but dropping
its child cannot leak ownership.

## Information Flow

The dependency direction is:

1. language contracts define future ownership, cancellation, blocking effects, and byte
   interfaces;
2. the runtime contract defines opaque descriptor, timer, and process-completion interests plus
   typed target-service roles;
3. task lifecycle accepts only those interests and never inspects process or descriptor internals;
4. the Darwin reactor maps interests to native events without owning child lifecycle;
5. `std/process` owns command policy, child and endpoint state, public failures, and observation
   precedence;
6. MIR and machine stages consume selected primitive roles and encoded wait records without
   rediscovering standard-library names;
7. public convenience operations and applications compose only the checked process and I/O
   contracts.

No compiler or editor component may recognize `Command`, `Child`, `Stdio`, or pipe type spellings.
No standard source may reproduce runtime interest tags or native event constants. No target layer
may select a public `std.process` error.

## Generic Transfer and Lifecycle Control

Generic byte transfer belongs to `std/io`, not `std/process`. `io.copy` and `io.copy_blocking`
own one bounded scratch allocation and depend only on their matching reader and writer contracts.
They do not flush, close, or inspect either stream. A process pipe therefore composes with the same
algorithm as any other stream without exposing descriptor state or process ownership.

`Child.try_wait` is the sole nonwaiting observation entry. It may transition the private owner from
pending to one cached terminal state; later calls and consuming wait use that state without another
native observation. One wait-attempt classifier owns interruption, pending state, exact returned
child identity, and status decoding for both nonwaiting and consuming observation. Callers do not
preclassify a raw wait result. `terminate` and `kill` request graceful and forced target actions but
never mark the child observed. The unreaped owner remains with `Child`, so a subsequent wait or
ordinary destruction still proves exact cleanup. Closed primitive roles own the target meanings
and signal encoding; source receives no arbitrary PID or numeric-signal interface.

## Rejected Alternatives

- Async wrappers around blocking `waitpid` or `poll` would violate the guarantee that `future T`
  is safe to drive on the executor thread.
- Periodically probing every child would add latency and a second timer policy.
- Treating EOF on stdout or stderr as child completion fails when descendants inherit an endpoint
  and loses exit status when no pipe is configured.
- Letting `Child` destruction detach silently would permit live-child and zombie leaks.
- Waiting synchronously from `drop` would make cancellation secretly blocking.
- Making callers promise to call `wait` would place a process-safety invariant outside the type's
  implementation.
- Maintaining the current closed session beside a new streaming session would create two launch,
  descriptor, failure-precedence, and cleanup authorities.
- Adding async filesystem or asynchronous item iteration to this milestone would combine separate
  scheduling and language problems with process ownership instead of completing one area.

## Completion Boundary

This boundary is complete when one launch authority supports closed and streaming execution, one
process owner always reaches explicit observation or abandonment, pipe endpoints satisfy ordinary
I/O interfaces and generic transfer, nonwaiting observation cannot reap twice, termination retains
the exact cleanup obligation, synchronous and asynchronous names follow the execution convention,
generated and host reactors implement the same three-interest contract, and whole-area review finds
no blocking future path, periodic process polling, raw-PID ownership assumption, duplicate session,
hidden descriptor copy, caller-required cleanup, or source-name special case.
