# Service Lifecycle

`std/service` owns structured lifecycle policy for long-running asynchronous services. It combines
public task ownership with cooperative cancellation; it does not inspect executor queues,
notification descriptors, or child-future representations.

The [public declaration contract](index.nct) is the API authority. This guide records lifecycle
behavior and ownership policy that does not fit in an individual declaration comment.

## Scope Ownership

`ServiceScope` is the sole owner of every computation accepted through `add`. The computation is
lazy but retained immediately. `next` drives all retained work fairly and removes one completed
outcome. Destroying the scope cancels every child still retained by its task group.

Admission is explicit. `add` returns `ServiceAdmission.stopped` with the original computation when
the scope no longer accepts work, so rejection never silently drops an owned operation.

## Graceful Shutdown

`stop` closes admission and requests the scope's shared cancellation token exactly once. Service
work should receive a token before admission and use it to leave waits cooperatively.
The transition is `noalloc`; it mutates the existing scope and notification state without creating
shutdown storage.

`shutdown` performs the same transition and then joins every retained child. It reports the first
failed child only after all children have been observed; later failures do not prevent draining.
This ordering makes resource release deterministic without hiding the primary failure. Draining
uses one inline failure slot and does not allocate merely to remember that result.

The scope does not force arbitrary I/O to become cancellable. A child must compose its operation
with the supplied token, or use an API whose own cancellation contract closes the wait when its
future is destroyed.

## Process Lifecycle

`lifecycle_requested` waits for configuration reload, interactive interrupt, or graceful
termination through the executor's existing descriptor-readiness path. On Darwin these requests
correspond to `SIGHUP`, `SIGINT`, and `SIGTERM`. A reload observation is consumed so a service can
return to waiting for a later reload. Interrupt and termination observations become sticky because
shutdown cannot be withdrawn; every later caller observes the same request.

The compiler-owned process context creates one event source, retains all three original signal
dispositions, and restores them during process finalization. The standard module sees only the
target adapter's descriptor and classified observation contract; it does not own global signal
state or reproduce native event-record layout. Signal observation does not read, validate, or
publish configuration. Application code performs those steps after receiving a reload request.
