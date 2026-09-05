# Text and Collections

`std/string` provides owned UTF-8 storage and validation; the source-owned `str` instance presents
byte-oriented search and borrowed projection:

- `find` and `find_from` return byte offsets;
- `contains`, `starts_with`, and `ends_with` do not allocate;
- `split` rejects an empty separator and returns independently owned `String` values;
- `get_range`, `strip_prefix`, and `strip_suffix` return validated borrowed views;
- `split_views` and `lines` return allocation-free borrowed iterators;
- `String.concat(...parts: &str)` joins zero or more borrowed views into owned storage through the
  language argument-pack contract;
- `String.from_utf8` validates a byte slice before copying it into owned storage.

Search and range indices are UTF-8 byte offsets. `get_range` rejects endpoints that divide an
encoding. `split_views` retains both its text and separator while iteration remains live and
matches the component boundaries of owned `split`. `lines` recognizes LF and CRLF, preserves bare
CR, and does not synthesize a final empty line. Borrowed operations preserve source provenance
through a compiler-validated typed projection; raw-pointer reconstruction is not a substitute.
Returning owned `split` components keeps their lifetime independent of the input.

The practical text additions are defined by the string chapter's
[borrowed-range](../language/sequences-and-text.md#borrowed-string-ranges-and-iteration) and
[owned-transformation](../language/sequences-and-text.md#owned-text-transformations) contracts.
That chapter is the sole authority for their signatures, exact ASCII byte set, borrowed empty-view
position, replacement progress, capacity failure, and UTF-8 rules.

Text and collection observation are type-owned. `str` declares text methods and `[T]` declares
slice methods; `String` and `Vec<T>` reuse them through one-step receiver coercion. Slice indexing
is therefore the sole implementation used by direct `Vec<T>` indexing. Internal raw-view helpers
are not importable APIs. Explicit view construction uses `as`, for
example `(&text) as &str` and `(&values) as &[T]`.

`Vec<T>.retain` preserves relative order. Rejected elements are dropped exactly once, retained
elements move at most once, and the initialized prefix is updated only after compaction.
`Vec<T>.truncate` drops the removed suffix.

`Vec<T>` supports zero-sized element types. Its length and capacity remain logical element counts;
reserving, inserting, removing, iterating, and dropping such elements do not allocate backing
bytes and still perform the ordinary ownership operations once per logical element.

`String.empty()` and `Vec<T>.empty()` defer allocator selection until their first growth.
`with_capacity`, including a request for zero capacity, records the allocator selected by that
construction call so later growth cannot silently move to another allocation context.

`str` defines equality once, and `String` reaches it through readonly coercion. `[T]` defines
element-wise equality, `contains`, and `position` under `where (&T == &T): bool`; `Vec<T>` reaches the same
implementation through its readonly slice coercion.

`str` also defines bytewise lexical strict ordering, and `[T]` defines lexicographic ordering under
`where (&T < &T): bool`. `String` and `Vec<T>` reach those declarations through the same readonly
coercions; neither nominal container duplicates the algorithms.

Readwrite slices define in-place ordering whose heap-sort implementation requests no auxiliary
storage of its own:

```nct
instance [T] {
    pub method &+self.sort(): void where (&T < &T): bool
}
```

After `sort`, no later element is strictly less than an earlier element. The operation relies on
the strict total order promised by the selected `<` implementation; it does not call equality or
accept a comparator callback. The sorting machinery uses constant auxiliary storage and has
`O(n log n)` worst-case comparisons and moves. It may reorder equivalent elements. Comparison
borrows elements, while rearrangement transfers ownership without copying or destroying an
element. The method reports no recoverable failure and introduces no bounds trap for a valid slice.
The selected `<` implementation may allocate or trap, so `sort` does not publish `noalloc` under the
current operator-requirement contract and does not catch or reinterpret termination.

`Vec<T>` reaches the same method through its declared `&+Vec<T> as &+[T]` coercion and does not
declare a forwarding method. Empty, one-element, already ordered, reverse-ordered, duplicate, and
move-only element sequences follow the same contract.

The iterator terminal operations `find`, `contains`, `position`, `any`, `all`, and `fold` remain
default methods of `Iterator`. Their implementations allocate no collection of their own, but the
selected `next`, callback, comparison, or item destruction may allocate. They therefore do not
publish `noalloc` under the current requirement contracts. Their item type is `Self.Item`; they use
static generic dispatch and do not require runtime interface objects. `contains` and `position`
consume the iterator, borrow each yielded owner for equality, and destroy every yielded owner
exactly once, including the item that causes early return.

`ExactSizeIterator` declares `Self impl Iterator` as an interface prerequisite.
`I impl ExactSizeIterator` therefore exposes `next`, `I.Item`, and iterator default methods without
repeating an `I impl Iterator` predicate. Concrete iterator types still declare separate explicit
`Iterator` and `ExactSizeIterator` implementation facts.

Collection-owning terminals live in modules that depend on both the iterator contract and the
destination collection. `std/iter/collect.to_vec` consumes any `Iterator` into a `Vec`. It is not an
`Iterator` default method because making the core iterator module depend on `std/vec` would create
a module cycle: Vec iteration already depends on `std/iter`.

The associative collection contract provides representation-neutral `Map<K, V>`
and `Set<T>` rather than exposing the private hash-table strategy through `HashMap` and `HashSet`
names. Its hash coherence, mapping literal, lookup, mutation, iteration, allocation, and ordering
contracts are centralized in [Associative Collections](associative-collections.md).
