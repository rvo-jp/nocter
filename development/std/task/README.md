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

The initial fixed arity is an intentional bounded-concurrency surface. A later homogeneous
collection operation can build on the same ownership and scheduling contract without changing
`join`.
