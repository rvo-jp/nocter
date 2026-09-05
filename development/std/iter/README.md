# Iteration

The compiler-checked [`Iterator` module contract](index.nct) is the sole authority for exact public
interfaces, adapters, associated bindings, and declarations. This chapter defines their longer
behavior without repeating signatures.

`next` advances mutable iterator state and returns absence only after exhaustion. Default adapters
own their sources and callbacks and use static generic dispatch. Terminal operations consume the
iterator. Equality terminals borrow each yielded owner for comparison and destroy every yielded
owner exactly once, including an item that causes early return.

Default terminal implementations allocate no collection of their own, but selected `next`,
callback, comparison, and item-destruction implementations may allocate. They therefore do not
claim `noalloc` unless their complete transitive contract proves it.

`ExactSizeIterator` requires `Iterator`. An exact-size bound exposes `next`, the associated item
type, and Iterator defaults without repeating the base bound. Concrete iterators still state both
implementation facts explicitly. `remaining_len` is the exact count of future successful advances,
not a capacity hint; each successful advance reduces it by one.

An adapter retains exact-size behavior only when the exact remaining count is representable for
every valid state. Chaining two exact-size iterators does not by itself prove that property because
their sum may exceed `usize`.

Collection-owning terminals live in modules that depend on both the iterator and destination
contracts. [`std/iter/collect`](collect/index.nct) consumes an Iterator into a Vec outside the core
interface, avoiding a module cycle between iteration and Vec iteration.
