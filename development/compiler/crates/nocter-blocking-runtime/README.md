# nocter-blocking-runtime

## Responsibility

Own the target-independent lifecycle, capacity, admission, worker transfer, completion identity,
abandonment, resource-retirement reservation, and shutdown model for operations that must execute
outside Nocter's executor because the target cannot expose readiness for their synchronous
progress.

## Contract

The service accepts opaque owned job inputs and publishes opaque owned outcomes. It does not know
filesystem operations, paths, descriptors, public errors, compiler IR, computation frames, target
queue records, threads, or native synchronization. A target worker claims one `RunningJob` and
calls `begin` to receive the input by value plus an independent `RunningJobCompletion` guard. The
worker consumes the input without partially mutating lifecycle storage, then completes the guard
with one output. Dropping either an unstarted job or a begun completion guard is itself a closed
worker-loss transition, so unwinding or a missing completion call cannot strand capacity or a
waiter.

Saturation retains no input: `submit` returns the exact input and a capacity epoch. A completion
adapter may wait for a later epoch and retry without reconstructing the job. Job identities and
capacity epochs carry an opaque process-local service qualification in addition to their monotonic
sequence, so observations from separate service instances cannot alias. The service itself invokes
one wake-only notifier after every waiter-relevant transition; an adapter cannot strand a waiter by
forgetting a second publication call. A wake carries no job meaning, so a waiter always queries its
exact identity after resuming.

`RetirementService` is the separate authority for native resources whose destruction may itself
block. A permit is reserved before the resource exists. Attaching a successfully created resource
produces a `ResourceOwner`; dropping that owner transfers the resource into its already reserved
queue without allocation, waiting, or fallible admission. A cleanup worker owns destruction and
releases the permit afterward. Explicit retirement instead receives a service-qualified identity;
the completed cleanup retains its reservation until the waiter consumes or detaches it. Ordinary
operation capacity therefore cannot prevent required cleanup, while the permit bound prevents an
unbounded retirement queue or completion set.

## Invariants

- Accepted jobs never exceed `maximum_jobs`; running jobs never exceed `workers`.
- The queue owns each queued input. Claim transfers it to one `RunningJob`; `begin` atomically
  separates that input from the sole completion guard. The service owns each completed outcome.
- Cancellation removes queued and completed ownership immediately. Cancelling a running job marks
  its future result abandoned; its `RunningJob` or `RunningJobCompletion` remains the sole
  lifecycle owner until completion or destruction.
- Completing or dropping a running completion guard always releases one worker. An ordinary completion keeps
  its admission slot until consumed; an abandoned completion releases it immediately.
- Dropping a non-abandoned running owner publishes `WorkerLost` rather than leaving a waiter
  suspended forever. Dropping an abandoned owner only retires its slot.
- Shutdown closes admission, extracts queued and completed ownership, abandons running work, and
  remains alive through the running owners until they retire.
- Capacity epochs advance whenever a previously occupied admission slot becomes free. They never
  assign job meaning, cannot be used to consume a result, and are rejected by any service other
  than their issuer.
- Wake notification runs after releasing the state lock, so it may safely re-enter read-only service
  queries without becoming a lifecycle mutation path.
- Public methods validate identities and states; callers cannot mutate lifecycle fields directly.
- Live retirement owners, unattached permits, queued resources, and running retirements share one
  bounded reservation count. Closing ordinary admission does not invalidate already reserved
  cleanup.
- Resource-owner destruction is the retirement transition. A caller does not need to invoke a
  second cleanup callback, and worker-guard destruction releases capacity after cleanup failure.
- Explicit close observes one exact retirement identity. Adapter destruction detaches all such
  waiters without cancelling cleanup, so an unconsumed completion cannot retain the service.
