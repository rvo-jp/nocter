# Owned Blocking-Job Boundary

This document defines the cross-responsibility boundary for target operations that cannot expose
nonblocking readiness. Public filesystem and iteration behavior belongs to the standard-library
contracts and language specification.

## Authority Flow

```text
standard operation policy
  -> typed owned job input
  -> bounded blocking-job service
  -> target worker adapter
  -> typed owned job outcome
  -> wake-only descriptor readiness
  -> exact JobId observation
  -> task reactor and suspended computation
```

`nocter-blocking-runtime` is the sole lifecycle and capacity authority. It does not execute or
classify a filesystem operation. A target adapter owns native worker creation, synchronization, and
wakeup transport. The task runtime consumes only its existing readable-descriptor interest and
cannot access job identities, inputs, or outcomes. Job-specific completion remains a query against
the lifecycle authority after wakeup; the reactor never becomes a second result index.

## Ownership

Submission transfers a complete input into the service. Claim transfers it into a `RunningJob`
owner that remains valid when moved to another thread. Starting execution consumes that owner and
returns the input by value alongside an independent `RunningJobCompletion` guard. Native operation
code therefore never receives mutable access to lifecycle-owned input and cannot leave a
half-consumed record after unwinding. Completion transfers one output back to the service only
while a waiter remains. Cancellation of running work detaches the waiter but does not pretend a
synchronous syscall stopped; the completion guard later destroys its unpublished output.

Dropping an unstarted running owner or a begun completion guard is a lifecycle transition, not an
omitted callback. A waiting job receives an explicit worker-loss outcome. An abandoned job releases
capacity. Consequently neither a native adapter nor a future destructor must remember a second
bookkeeping call to keep the service valid.

Target work never retains a borrow into an abandonable future frame. Read operations use worker-
owned result storage and copy into caller storage only after the future resumes. Write, path, and
configuration inputs are owned before submission. Optimizations may remove a copy only after they
prove the same cancellation and address-stability contract.

## Capacity and Backpressure

Worker count and total admitted jobs are finite independent limits. Admitted jobs include queued,
running, abandoned-running, and completed-but-unconsumed states. Saturation returns the exact input
and the observed capacity epoch. The asynchronous adapter waits for a later epoch and retries;
saturation is not reported as an operating-system or public filesystem failure.

An epoch signals only that admission capacity changed. It cannot identify, complete, or consume a
job. Job IDs and epochs contain an opaque service qualification plus a monotonic sequence. They are
never reused by their issuer and are rejected by every other service, so a stale or misrouted
observation cannot become valid for a different job or capacity domain. A shared nonblocking
descriptor wakes suspended computations after completion or capacity change. Its bytes contain no
result identity and may be coalesced: every resumed computation polls its exact JobId or epoch
before deciding whether to complete or suspend again.

## Shutdown

Shutdown first closes admission. It extracts queued inputs and completed outcomes for ordinary
destruction, marks running work abandoned, and keeps shared service state alive through each
running owner. The target adapter then joins or drains its fixed workers before releasing native
service storage. Executor shutdown and service shutdown use this single transition; neither scans
or interprets the other's private storage.

## Prohibited Coupling

- The service cannot know paths, descriptors, errno, public error codes, or operation variants.
- A target worker cannot mutate task state or computation frames.
- The task reactor cannot inspect job state to classify operation success or failure.
- Standard-library source cannot reproduce queue records, worker synchronization, completion
  encoding, or capacity constants.
- An async filesystem implementation cannot call a public blocking wrapper or retain caller
  storage across target work.
