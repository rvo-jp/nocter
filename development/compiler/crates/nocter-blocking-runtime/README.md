# nocter-blocking-runtime

## Responsibility

Own the target-independent lifecycle, capacity, admission, worker transfer, completion identity,
abandonment, and shutdown model for operations that must execute outside Nocter's executor because
the target cannot expose readiness for their synchronous progress.

## Contract

The service accepts opaque owned job inputs and publishes opaque owned outcomes. It does not know
filesystem operations, paths, descriptors, public errors, compiler IR, computation frames, target
queue records, threads, or native synchronization. A target worker claims one `RunningJob`, moves
that owner to its execution context, and completes it with one output. Dropping the owner is itself
a closed worker-loss transition, so a missing completion call cannot strand capacity or a waiter.

Saturation retains no input: `submit` returns the exact input and a capacity epoch. A completion
adapter may wait for a later epoch and retry without reconstructing the job. Job identities are
monotonic and never reused. Completion delivery may therefore contain stale identities, but a stale
event cannot name a later job.

## Invariants

- Accepted jobs never exceed `maximum_jobs`; running jobs never exceed `workers`.
- The queue owns each queued input, one `RunningJob` owns each claimed input, and the service owns
  each completed outcome.
- Cancellation removes queued and completed ownership immediately. Cancelling a running job marks
  its future result abandoned; its `RunningJob` remains the sole execution owner until completion
  or destruction.
- Completing or dropping a running owner always releases one worker. An ordinary completion keeps
  its admission slot until consumed; an abandoned completion releases it immediately.
- Dropping a non-abandoned running owner publishes `WorkerLost` rather than leaving a waiter
  suspended forever. Dropping an abandoned owner only retires its slot.
- Shutdown closes admission, extracts queued and completed ownership, abandons running work, and
  remains alive through the running owners until they retire.
- Capacity epochs advance whenever a previously occupied admission slot becomes free. They never
  assign job meaning and cannot be used to consume a result.
- Public methods validate identities and states; callers cannot mutate lifecycle fields directly.
