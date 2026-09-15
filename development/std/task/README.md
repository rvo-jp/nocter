# Structured Asynchronous Tasks

The compiler-checked [`std/task` contract](index.nct) is the sole authority for exact public
declarations. This guide expands the observable scheduling and ownership rules.

`task.join` takes ownership of exactly two lazy computations. Awaiting the returned computation
starts both children in left-to-right order, allows either child to make progress whenever its wait
interest becomes ready, and completes only after both outputs are available. The result tuple keeps
the argument order.

Joining does not interpret output types. In particular, joining `future A!` and `future B!` produces
`future (A!, B!)`; it does not cancel one child merely because the other completed with a
recoverable failure value.

The joined computation is the sole lifecycle owner of both children. Destroying it before its
result is consumed cancels both children and releases their initialized state exactly once.
Consuming a completed join transfers both outputs and retires both child computations. No detached
task, hidden process-global executor, or independently copyable task handle is created.

`task.race` takes ownership of two computations with the same output type and returns
`Race.first(value)` or `Race.second(value)`. It polls from left to right. If both branches can
complete during one drive step, the first branch wins. Selecting one branch immediately cancels
the other; dropping the race instead cancels every child still owned by it. The winner value stays
owned by its completed child frame until the race result is consumed.

`task.with_timeout(computation, timeout)` races one owned computation against `time.sleep(timeout)`
and returns `Timeout.completed(value)` or `Timeout.elapsed`. It does not reinterpret the child's
output: `future T!` produces `Timeout<T!>`, so a recoverable operation failure remains distinct from
elapsed time. The child is polled first and therefore wins if both outcomes can complete during the
same drive step. A zero timeout still permits one immediate child poll. Selecting elapsed time
cancels the child before the timeout computation completes.

`TaskGroup<T>` owns a runtime-sized homogeneous set of `future T` values. `add` transfers one lazy
computation into the group. `next` drives every retained computation and removes one completed
output. It returns absence immediately when the group is empty. Selection among children already
completed in the same drive step is deterministic but is not an ordering contract. A child starts
only while `next` is being driven; adding a future does not create detached background work.

The wait performed by `next` borrows the group's child set. Cancelling that wait releases its
temporary readiness records but does not cancel or remove a child. The group remains the sole
owner, so a later `next` can resume the same computations. Destroying the group instead destroys
its backing Vec and therefore cancels every retained future exactly once. A completed child remains
in the group until `next` removes and consumes it.

`join` and `race` remain fixed-arity conveniences with distinct result policies. `TaskGroup` is the
dynamic ownership primitive; it does not reinterpret child outputs, select a scheduler, or expose
an independently copyable task handle.
