# Filesystem and File I/O Boundary

This document owns the current cross-responsibility design for executor-safe file and path
operations. Exact public declarations and observable errors belong to the checked `std/io` and
`std/fs` contracts. Generic job lifecycle belongs to `nocter-blocking-runtime`; Darwin execution
and ABI encoding belong to their target adapters.

## Execution Surfaces

`File` is the canonical executor-safe owning file. Its open, create, exclusive create, append, read,
write, synchronize, non-waiting exclusive lock, position, seek, truncate, and explicit close
operations are asynchronous. The lock operation is package-visible infrastructure rather than a
public filesystem policy. `File` implements `Reader` and `Writer`. No operation calls a public
blocking wrapper or performs a potentially blocking filesystem operation on the executor thread.

`BlockingFile` is the explicit synchronous twin. It implements `BlockingReader` and
`BlockingWriter`, and its operation names retain `_blocking` where the interface requires them.
Both types consume the same path validation, operation-result facts, public error classification,
and partial-progress rules. They do not share execution control: the asynchronous surface submits
owned jobs, while the blocking surface invokes the target operation directly.

Process-global standard streams remain on the explicitly blocking surface. A borrowed
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
Transient saturation keeps the complete prepared job and returns its shared service interest.
Closed admission instead has one prepared-to-completed rejection transition and reports the
closed-service failure without dispatching target work.

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

The generated retirement target family makes reservation, owner publication, drop, explicit-close
drive/cancel/consume, worker dispatch, notification, and dispatch-group accounting inseparable.
Close is issued exactly once, including an indeterminate target failure. A detached worker clears
all transient record ownership before atomically publishing reusable capacity; explicit close
retains its validated two-word failure fact until the waiter consumes or cancels it. Native
qualification transfers a real Darwin pipe descriptor through this complete path before root
shutdown.

## Result Facts and Policy

The Darwin operation adapter publishes typed raw facts: operation kind, retained file owner when
applicable, initialized read length, completed write prefix, resulting position, and target error.
It does not construct `std` errors or interpret UTF-8. Standard source maps those facts once through
the existing portable I/O error authority.

Those facts use the closed `DarwinFileFailure` and `DarwinFileWriteFact` runtime contracts. A host
conformance adapter reduces `std::io::Error` to a positive Darwin errno or an explicit adapter
failure before publication; generated code never receives a Rust error object. Zero-progress and
position-overflow failures have their own variants and are not disguised as target errno.
Every operation publishes the same fixed completion record. It contains the retained owner,
transfer and position facts, metadata and identity facts, and the closed failure pair; fields unused
by an operation are zero. Explicit close uses its dedicated preallocated retirement computation. A
zero owner is the empty moved-from representation and is required for failed open and terminal
close. Within the failure pair, zero kind and zero errno mean success; only the Darwin-target kind
admits a positive errno. Every other encoding is invalid, so standard source never guesses whether
a numeric word is an errno or an adapter classification.

A failed read, positioned read, seek, truncate, flush, or lock attempt restores the file owner
before returning its error. Lock acquisition never waits; host and native adapters normalize
contention to one closed runtime failure fact before `std/io` maps it to `std.io.lock_contended`. A write or positioned-write
failure restores the owner but retains an observable completed prefix; retrying the whole input is
not implied. Positioned operations leave the shared cursor unchanged. Close is terminal whether
its target operation succeeds or fails. A malformed target fact is an internal target-contract
failure rather than a fabricated filesystem result.

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
The runtime contract owns the frame's fixed prefix, operation-owned trailing-byte region, and sole
checked allocation-size calculation. Open, read, and write bytes never point into authored
storage. Positioned offsets, truncate lengths, seek displacements, transferred byte counts, and
result positions have distinct fields rather than an operation-dependent anonymous payload slot.
The closed operation contract assigns each operation its exact owned-byte, retirement-state,
operand, and scalar-result interpretation.
Read frames separately retain a consumer-only destination pointer. The target worker cannot access
that field, and cancellation clears it before atomically detaching the worker, so worker ownership
never extends the caller's buffer borrow.
The frame also carries an explicit capacity-ownership word. A service-closed rejection reaches
completion without capacity, while a dispatched completion retains capacity until consume or
cancel. Cleanup therefore never infers admission from an error kind or lifecycle tag. Before a
worker publishes attached completion, it retains the notification descriptor and worker group in
callee-saved state. After publication it signals and leaves the group without reading the frame,
allowing the consumer to release the frame immediately without a worker race.
The service wake descriptor is shared and carries no identity. The generated wait projection
coalesces its equal native registration keys and fans one returned event back out to every matching
semantic readiness destination before any future performs its exact state query.
The closed file-service import catalog is the only layer allowed to select these Darwin system
symbols; generated instruction code receives typed import identities.

The source primitive boundary uses two package-internal opaque storage declarations. `FileOwner`
is one retirement-record handle; `FileCompletion` has the runtime-owned uniform completion layout.
Target-program validation binds both declarations to closed storage roles before Machine assigns
their ABI. Source policy can take the owner or read an individual scalar only through exact
primitive roles. Taking or disposing clears the owner slot first, so subsequent implicit
destruction is inert and cannot retire the same descriptor twice.

ARM64 scans the frozen Machine primitive targets once and declares the complete file target family
only when a file role is reachable. That family is the sole mapping from source roles to job
constructors, explicit close, ownership disposal, and completion accessors. Its shutdown entry is
published through the generic process-finalizer catalog and runs once on each ordinary process-root
return after source cleanup. A target with no file role has no file-service code or imports.

## Path and Metadata Jobs

An asynchronous path operation validates each borrowed spelling through `std/path.validate`
before constructing a job. The generated constructor checks allocation-size arithmetic, appends
one terminal NUL byte per path, and copies every byte into the job's trailing owned region before
admission. Rename stores two consecutive owned paths and one runtime-contract offset naming the
second path. A worker never retains authored text or a pointer into a caller.

Path mutation uses the same bounded service admission, wake descriptor, state machine, and
completion record as file operations. It creates no descriptor owner and therefore consumes no
retirement reservation. Queued cancellation destroys owned input immediately; running abandonment
leaves the job with the service until the worker publishes and destroys its unobserved result.
Mutation is attempted exactly once because retrying an interrupted mutation could apply it twice.

Metadata appends aligned worker-only target-record scratch storage to the same owned-path job. The
worker may retry an interrupted read-only query, decodes the native record once, and publishes only
portable classification, length, and normalized timestamp facts. Standard source cannot inspect
native scratch bytes or reconstruct the Darwin layout.

Directory, canonicalization, link-reading, and recursive operations extend the same service
contract with owned output bytes or directory owners. Recursive source policy uses explicit
bounded stacks and streams; it does not create an unbounded native call stack or materialize an
entire tree before publication.

## Authority Boundaries

`nocter-runtime-contract` owns the closed operation vocabulary, owned-byte shapes, result records,
and job layouts. `nocter-target-program` validates each primitive's checked source signature. The
standard profile maps semantic roles to physical declarations exactly once. Machine and ARM64
consume those closed roles and layouts without inferring operations from source names.

`std/path` owns UTF-8 and NUL validation, package-private I/O source maps uniform completions, and
`std/fs` owns public operation policy. The Darwin service is lifecycle conformance, not another
public filesystem implementation.

## Required Invariants

- `File` operations never block the executor thread.
- Workers never retain caller paths, buffers, or future-frame borrows.
- File destruction always has pre-reserved retirement capacity.
- Path mutation is never retried after an indeterminate target result.
- Native records are decoded by the target adapter and never reconstructed in standard source.
- Compiler stages consume closed primitive roles instead of source paths or declaration names.

The closed worker-operation, access-mode, and seek-origin vocabularies are owned by
`nocter-runtime-contract`. Host conformance and generated target code consume those tags. Standard
source calls separate semantic open and seek primitives and therefore cannot construct or duplicate
a target tag.

Exclusive creation is an `Open` access mode rather than a second operation lifecycle. The runtime
contract owns its access tag, target adapters map it to create-with-exclusion, and standard source
receives the ordinary retained owner or portable already-exists failure. This keeps temporary-file
policy in `std/fs` while making accidental truncation impossible at the target boundary.

`std/fs` durable replacement composes these existing operations: exclusive sibling creation,
complete write, file synchronization, close, one rename, package-internal directory open, and
directory synchronization. The file service knows none of the naming or replacement policy, while
filesystem policy cannot inspect descriptors, access tags, jobs, or target errno values.
