# Iteration

The compiler-checked [`Iterator` module contract](index.nct) is the sole authority for exact public
interfaces, adapters, associated bindings, and declarations. This chapter defines their longer
behavior without repeating signatures.

`next` advances mutable iterator state and returns absence only after exhaustion. Default adapters
own their sources and callbacks and use static generic dispatch. Terminal operations consume the
iterator. Equality terminals borrow each yielded owner for comparison and destroy every yielded
owner exactly once, including an item that causes early return.

`LendingIterator` is the separate protocol for an item whose validity is bounded by the active
mutable receiver loan. Its `next` result is explicitly `from self`; the associated `Item` decides
whether the loan is readonly (`&T`), readwrite (`&+T`), or carried inside an aggregate. A collection
loop accepts exactly one of `Iterator` and `LendingIterator`. Implementing both is ambiguous rather
than establishing a priority. The current item must cease to be live before another lending step,
which permits one-at-a-time mutable views without a named lifetime parameter. Sequence spread does
not accept lending iterators because an argument pack may retain several yielded items at once.

Readonly view iteration retains the source view's storage provenance, and each yielded readonly
borrow carries that same origin. Readwrite view iteration retains one exclusive view and exposes one
exclusive element borrow at a time. Owning Vec iteration yields elements in source order; dropping
it destroys unconsumed elements in reverse order and releases the transferred storage exactly once.

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

`AsyncIterator` is owned by the [`std/iter/asynchronous`](asynchronous/index.nct) child module and
re-exported from `std/iter`. Its mutable `next` operation produces an executor-safe future whose
output is `Item?!`: clean exhaustion, one yielded item, and a recoverable terminal step failure are
distinct. The failure layer remains outside the optional layer so an unavailable item is not
mistaken for a yielded failure value. `WalkDir` implements this exact contract, and `for await`
consumes it without a second iteration protocol. The standard `ReadDir`, buffered text-line, and
bounded byte-chunk producers implement the same contract rather than defining subsystem-specific
loop protocols.

`AsyncLendingIterator` is the asynchronous counterpart. The pending step future and a yielded item
retain the iterator receiver loan expressed by `from self`; cancellation releases the pending step
before the iterator. `for await` accepts exactly one of the owning and lending asynchronous
protocols and rejects a type implementing both.

Asynchronous `map`, `filter`, `take`, and `enumerate` adapters own their source and any callback.
They remain lazy and request one upstream item only when downstream requests an item. A callback is
synchronous: it runs only after acquisition completes and cannot introduce an untracked suspension.
Every adapter preserves clean exhaustion and the exact upstream failure value. `take` stops
requesting upstream items after its limit, while `filter` may request multiple items to produce one
accepted downstream item. None of these adapters materializes the stream.
