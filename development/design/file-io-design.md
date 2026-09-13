# Canonical File I/O Boundary

This document owns the cross-responsibility design for v0.50.0 executor-safe local-file operations.
Exact public declarations and observable errors belong to the checked `std/io` and `std/fs`
contracts. Generic job lifecycle belongs to `nocter-blocking-runtime`; Darwin execution and ABI
encoding belong to their target adapters. The public surface described here becomes current only
when Phase 1 performs its complete declaration and implementation cutover.

## Execution Surfaces

`File` is the canonical executor-safe owning file. Its open, create, append, read, write, flush,
position, seek, truncate, and explicit close operations are asynchronous. It implements `Reader`
and `Writer`. No `File` operation calls a public blocking wrapper or performs a potentially blocking
filesystem operation on the executor thread.

`BlockingFile` is the explicit synchronous twin. It implements `BlockingReader` and
`BlockingWriter`, and its operation names retain `_blocking` where the interface requires them.
Both types consume the same path validation, operation-result facts, public error classification,
and partial-progress rules. They do not share execution control: the asynchronous surface submits
owned jobs, while the blocking surface invokes the target operation directly.

Process-global standard streams remain on the explicitly blocking surface in Phase 1. A borrowed
standard-input descriptor cannot safely enter a blocking worker because cancellation cannot close
it to release that worker. A later asynchronous standard-input owner must use descriptor readiness
and nonblocking transfer rather than weakening file cancellation.

## File Ownership During Suspension

An executor-safe `File` contains one target file owner and one pre-reserved retirement permit.
Starting an operation moves both into the owned job input and makes the public value temporarily
terminal. The exclusive mutable receiver prevents another operation from observing this state.
Every ordinary outcome returns the same owner and permit before portable result mapping begins.

Cancellation never leaves a worker borrowing the `File`, its future frame, a caller buffer, or an
authored path. A cancelled queued input and an ignored completed output both drop the same
retirement-aware owner. A running abandoned job drops it after the target operation returns. Worker
loss does the same through the worker guard. The public `File` remains terminal after cancellation
or worker loss; preserving apparent usability while an uninterruptible operation still owns its
position would be unsound.

Open jobs own the normalized target-path bytes. Read jobs own result storage and copy only the
completed initialized prefix into the caller's buffer after resumption. Write jobs own a copy of
their complete input bytes. No target worker publishes directly into caller storage. Open output
already carries a retirement permit, so cancellation after native creation closes the unpublished
file on a cleanup worker.

The generated ARM64 Darwin runtime admits at most four active operation callbacks and 64 total
operations. Retirement has its own one-worker, 64-reservation capacity and cannot be consumed by
ordinary jobs. These are runtime-contract values rather than standard-library constants or backend
defaults.

## Infallible Retirement Admission

Ordinary bounded job admission cannot be the destruction path. A full operation queue would force
drop either to block, leak the descriptor, or fail. The runtime therefore reserves one retirement
slot before an asynchronous file can be created. The unattached permit, live file, queued cleanup,
and running cleanup are mutually exclusive states of the same bounded reservation.

Dropping a live owner fills its reserved queue slot without allocating, waiting, or returning an
error. A fixed cleanup worker owns the potentially blocking close and then releases the slot. New
file construction treats exhausted retirement capacity as backpressure, waits for the issuer's
service-qualified capacity epoch, and retries. Retirement queues cannot exceed their permit bound,
and ordinary operation saturation cannot consume their admission.

Explicit asynchronous close uses the same retirement transition but retains a waiter until cleanup
finishes. Its service-qualified retirement identity cannot alias another service or a later close,
and its reservation remains occupied until the waiter consumes or detaches completion. Destruction
uses the same queue without a waiter. Runtime shutdown closes new permits first, cancels or
abandons operation waiters, detaches close waiters, and continues draining every existing
retirement reservation before releasing worker storage.

## Result Facts and Policy

The Darwin operation adapter publishes typed raw facts: operation kind, retained file owner when
applicable, initialized read length, completed write prefix, resulting position, and target error.
It does not construct `std` errors or interpret UTF-8. Standard source maps those facts once through
the existing portable I/O error authority.

Those facts use the closed `DarwinFileFailure` and `DarwinFileWriteFact` runtime contracts. A host
conformance adapter reduces `std::io::Error` to a positive Darwin errno or an explicit adapter
failure before publication; generated code never receives a Rust error object. Zero-progress and
position-overflow failures have their own variants and are not disguised as target errno.
The optional failure occupies exactly two target words: a closed non-zero failure kind and a
positive errno admitted only by the Darwin-target kind. The all-zero record is success. Every
other encoding is an invalid target fact, so standard source never guesses whether a numeric word
is an errno or an adapter classification.

A failed read, positioned read, seek, truncate, or flush restores the file owner before returning
its error. A write or positioned-write failure restores the owner but retains an observable
completed prefix; retrying the whole input is not implied. Positioned operations leave the shared
cursor unchanged. Close is terminal whether its target operation succeeds or fails. A malformed
target fact is an internal target-contract failure rather than a fabricated filesystem result.

## Dependency Direction

```text
std/io and std/fs policy
  -> typed file-operation primitive
  -> bounded blocking-job lifecycle
  -> Darwin file-operation adapter
  -> typed operation fact
  -> std portable result mapping

resource owner drop
  -> pre-reserved retirement transition
  -> Darwin cleanup worker
  -> permit release and wake-only capacity notice
```

The job lifecycle knows no file operation, descriptor, path, errno, standard error, future frame,
or source declaration. The Darwin adapter knows no public API or task identity. The reactor sees
only its existing descriptor-readiness interest. MIR and Machine consume a compiler-owned primitive
role and frozen target ABI; neither rediscovers a file operation from declaration names.

The process context contains one compiler-owned pointer slot for the lazily created blocking
service. The runtime ABI schema is the only authority for that slot and for the entry argument and
environment fields surrounding it. ARM64 frame layout and generated service code consume the
schema; neither owns a private copy of its offsets. The root initializes the slot to zero before
any authored call can observe the context. The generated service ensure target publishes exactly
one root through that slot and reuses it on later calls from the single executor thread. Its paired
shutdown target changes admission from accepting to draining, waits for the shared worker group,
proves the active-operation count and all retirement records are empty, changes the root to
released, clears the process slot, and only then frees storage. A second shutdown is an explicit
no-op rather than a caller precondition.

The root owns four operation queues, one retirement queue, one worker group, and one shared wake
pipe. Both pipe descriptors are nonblocking and close-on-exec before the root is published. Every
fixed retirement record receives the same service pointer and notification-reader interest, plus
its own readiness cell. Construction failure is fail-stop before publication; process termination
then owns partial native cleanup. Normal shutdown releases every dispatch object, issues each
descriptor close exactly once without retrying an indeterminate Darwin close result, clears the
context slot, and frees the service allocation.

One allocation-backed computation frame is also its worker-owned job record. Before admission it
owns complete input; after admission it is in exactly one of attached-running, detached-running,
or completed ownership states. This removes a second queue record and result index. State
publication uses target atomic operations, so resume and cancellation never wait on a lifecycle
queue and retain the drive-safe future guarantee. Accepted blocking work executes on four private
serial dispatch queues, providing the fixed worker bound. A dispatch group accounts for every
active worker callback so root shutdown can drain exact ownership before freeing service state.
The service wake descriptor is shared and carries no identity. The generated wait projection
coalesces its equal native registration keys and fans one returned event back out to every matching
semantic readiness destination before any future performs its exact state query.
The closed file-service import catalog is the only layer allowed to select these Darwin system
symbols; generated instruction code receives typed import identities.

## Completion Gate

Phase 1 is complete only after generated Darwin executables use this ownership path, `File` and
`BlockingFile` replace the former blocking-only surface in one migration, standard whole-file
helpers select the correct execution surface, compiler and editor projections show the checked
contracts, cancellation and drop are exercised through native execution, and no old alias or
executor-blocking implementation remains.

The closed worker-operation, access-mode, and seek-origin vocabularies are owned by
`nocter-runtime-contract`. Host conformance and generated target code consume those tags; neither
may maintain a private operation list.
