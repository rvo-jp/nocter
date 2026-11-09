# Asynchronous Computations

## Frame Storage

The future [storage rules](memory-and-regions.md#storage-optimization)
permit register, embedded, stack, or heap representations when they preserve the computation's
logical ownership, access, dependencies, stable observations, and cleanup. There is no universal
heap-frame or user-authored pinning requirement. This does not change the released representation
and effect contracts or complete the remaining output/type grammar.

Eliminating physical frame or capture fields does not release an input dependency retained by the
selected public contract, erase a value output's outside dependencies, change source availability, or make
access into a destroyed local valid. Pending and ready computations retain the dependencies needed
by their output and cleanup. Cancellation preserves child-before-owner cleanup; an unconsumed ready
output is destroyed before its dependencies are released.

Complete actual required construction storage before invocation-time transfer commits. A private
optional heap request may disappear under the storage contract, but a required recoverable/resource
failure cannot move into drive or cancellation. Infer waiting and allocation through the actual
chosen backing strategy; no authored `blocking`, `noalloc`, or `notrap` substitute is introduced.
An unproved drive-safety obligation remains a validity error rather than a hidden worker thread
or an assumption that an unknown backend is safe.

Combining backing blocks does not itself combine tasks, change poll/tie order, omit cancellation,
or turn cancellation into successful completion of a resource protocol. Task composition's
observable schedule and effects remain governed by its selected contract. Stronger coroutine fusion
needs a separate effect/cleanup argument, not just a proof that frame bytes can be reused.
