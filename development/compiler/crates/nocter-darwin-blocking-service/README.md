# nocter-darwin-blocking-service

## Responsibility

Own executable host conformance for fixed Darwin operation and resource-retirement worker pools and
their wake-only descriptor channels over the target-independent `nocter-blocking-runtime`
lifecycle.

## Contract

The adapter accepts one typed operation when constructed, creates exactly the configured number of
workers, and delegates all job ownership, admission, completion identity, cancellation, and
shutdown transitions to `nocter-blocking-runtime`. Workers know only the opaque input and output
types selected by their caller. They do not know task identities, computation frames, filesystem
policy, public errors, or source declarations.

The readable descriptor means only that service state may have changed. It never carries or
identifies a result. A suspended computation retains its monotonic `JobId`, drains a wake when
resumed, and queries the lifecycle authority for that exact result. Multiple changes may collapse
into one byte; a full channel is already readable and therefore does not lose the wake condition.

This Rust crate is conformance evidence for the generated Darwin service. Generated Nocter
executables do not link it.

`DarwinRetirementService` executes cleanup for resources that carry a pre-reserved retirement slot.
Owner destruction can therefore wake a fixed cleanup worker without waiting for or competing with
ordinary job admission. Explicit shutdown rejects externally held owners instead of assuming that
callers already destroyed them. Dropping the adapter with a live owner detaches only host join
handles: the lifecycle notifier and fixed workers remain reachable until the final issued owner
enters cleanup and the closed service drains.

An explicit close receives one exact `RetirementId`, observes completion after descriptor wakeup,
and consumes that identity before capacity is reusable. Adapter drop detaches any remaining close
waiters as one lifecycle transition; it never abandons the corresponding native cleanup.

## Invariants

- Construction either owns every configured worker and both channel endpoints or joins every
  partially created worker before returning failure.
- Submission notifies an idle worker only after the lifecycle service owns the input.
- A worker catches operation unwinding and lets `RunningJob` publish `WorkerLost`; one bad operation
  cannot silently reduce pool capacity.
- Completion, abandoned completion, queued cancellation, completed cancellation, and consumption
  all publish a wake after their state transition.
- Retirement owner destruction has infallible bounded queue admission because its permit was
  acquired before resource creation. Cleanup panic still destroys the resource and releases its
  reservation on the worker.
- Retirement shutdown cannot release worker storage while an external permit or owner exists. A
  detached adapter keeps no second queue or result index and its workers exit after the lifecycle
  authority becomes closed and drained.
- The channel is nonblocking and wake-only. `WouldBlock` on write is success because an unread wake
  already exists; interruption retries, and any other write failure is fatal.
- Shutdown closes admission before waking workers, joins them, and leaves no thread holding pool
  state when it returns.
