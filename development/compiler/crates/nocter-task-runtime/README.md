# nocter-task-runtime

## Responsibility

Own target-independent task lifecycle, opaque computation payload orchestration, runnable ordering,
and stable readiness-registration identity for the generated asynchronous runtime.

## Contract

The crate accepts opaque descriptor/timer interests through a `Reactor` boundary. `Scheduler`
publishes only task transitions and cleanup obligations. `Executor` pairs those identities with
opaque computations and owns the resume, cancel, and completed-output consumption order. Neither
layer inspects compiler IR, computation-frame layout, descriptor implementation, operating-system
event records, or source semantics.

## Invariants

- A task has one generation-qualified identity and one lifecycle state.
- The scheduler removes every wait registration before publishing cancellation cleanup.
- One readiness event consumes the complete wait set. Simultaneous or stale events cannot enqueue
  the same task twice.
- Reusing a task, registration slot, or operating-system descriptor cannot validate an event from
  an earlier generation.
- Canonical registration order decides wake order. Multiple events for one wait set still produce
  one runnable transition; operation code decides readiness-versus-deadline results when resumed.
- The reactor owns waiting and native registration mechanics. It cannot mutate task lifecycle.
- A pending computation carries a structurally non-empty `WaitSet`; an empty wait cannot strand a
  running task.
- Failed reactor registration returns its task to the runnable queue before the executor exposes
  the error.
- Executor cancellation detaches the complete wait set before invoking computation cleanup.
- An empty reactor event batch is not executor quiescence while a runnable or waiting task remains.
