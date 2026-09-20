# Cooperative Synchronization

[`index.nct`](index.nct) is the checked public contract for shared ownership and synchronization.
This guide explains its execution boundary and ownership rules.

Nocter sync values coordinate asynchronous computations scheduled cooperatively on one executor
thread. `Mutex<T>` therefore protects logical task interleaving; it is not an atomic cross-thread
mutex and must not be passed through a foreign-thread boundary. `share` is explicit because each
returned handle participates in the lifetime of one page-backed value.

`lock` waits through the executor's descriptor-readiness contract. It neither spins nor blocks the
executor thread. The returned `MutexGuard<T>` is the only public capability that exposes the value,
and its destruction releases the mutex on ordinary return, failure propagation, and future
cancellation. Notification bytes only prompt another state check; they never represent lock state.

`Channel<T, N>.bounded()` creates a queue whose positive capacity `N` is part of its type. Calling
`split` consumes that new channel and returns its initial sending and receiving endpoints.
Construction performs every allocation and descriptor operation; later sends, receives, endpoint
clones, and wakeups do not grow storage. `try_send` returns an unsent value on a full or
receiver-closed channel. `send` applies executor-friendly backpressure and returns the value only if
every receiver closes. A sent value's storage origins must be contained by the sender, and a
received value remains bounded by the receiver. This prevents a short-lived borrow from escaping
through shared queue storage. Receivers distinguish temporary emptiness from final sender closure.

`CancellationSource` is the unique capability that changes cancellation state. `token` returns a
clonable observer, `cancel` is idempotent, and destroying the source requests cancellation so a
remaining token cannot wait forever for an owner that no longer exists. `cancelled` uses the same
executor readiness path as mutex and channel waits. Cancellation is a cooperative request: it does
not interrupt code, detach a task, or suppress ordinary owned-value destruction.

The checked contract marks handle cloning, cancellation state changes and inspection, and
non-waiting channel attempts as `noalloc`. Construction owns every mapping and descriptor needed by
those operations. Asynchronous waits still construct ordinary owning computations, but driving one
does not grow the channel buffer or create a second notification authority.
