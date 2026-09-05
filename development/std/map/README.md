# Associative Collections

## Public Meaning, Private Representation

The standard associative collections are `Map<K, V>` and `Set<T>`:

```nct
use std/string.String

let counts = Map [
    String "apple": 3,
    String "orange": 2,
]

let names = Set [String "alice", String "bob"]
```

`Map` and `Set` describe collection meaning. Their names do not expose the storage algorithm. The
implementation uses a hash table, but bucket layout, probing, control bytes, load factor,
and tombstone representation are private standard-library details. The public library does not
provide `HashMap` or `HashSet` names or compatibility aliases.

A future collection that guarantees key ordering and range queries may use the separate names
`TreeMap<K, V>` and `TreeSet<T>`. `Map` does not become ordered when those types are introduced.

The public contract guarantees:

- one stored entry for each equality class of keys or elements;
- expected constant-time lookup, insertion, and removal under a well-behaved `Hash`
  implementation;
- storage growth through the ordinary current allocation context or an explicit recoverable
  allocator API;
- exact once-only destruction of every initialized key, value, and element;
- iteration over every current entry or element exactly once.

It does not guarantee:

- iteration order, stable hash values, bucket count, or a particular hash algorithm;
- a worst-case constant lookup bound;
- insertion order or key order;
- stable element addresses across a structural mutation.

Iteration order may differ after insertion, removal, reserve, rebuilding, process restart, target
change, or standard-library update. Programs must not use it as serialized or user-visible order.

## Hash Contract

`std/hash` declares the opaque state and key capability in its compiler-checked
[public contract](../hash/index.nct).

Only the standard library constructs or finalizes `HashState`. A user implementation contributes
the complete logical value by delegating to hashable fields or by writing a canonical byte
encoding. It does not select an algorithm or read the final hash:

```nct
instance UserId {
    impl Hash

    noalloc method &self.hash_into(state: &+HashState): void {
        self.namespace.hash_into(state)
        self.value.hash_into(state)
        return
    }
}
```

`Hash` has the following semantic obligations:

- `a == b` implies that hashing `a` and `b` into states with the same seed produces the same final
  hash;
- repeated hashing of an unchanged value into equivalent states contributes the same bytes in the
  same order;
- every equality-relevant field contributes to the hash;
- a variable-length component contributes an unambiguous boundary, normally through that
  component's own `Hash` implementation;
- hashing does not mutate the value, allocate, fail recoverably, or depend on its storage address.

The compiler checks the declared equality prerequisite, method signatures, and transitive
`noalloc` guarantee. It cannot prove behavioral coherence between arbitrary authored equality and
hashing bodies. Violating that coherence is a program contract violation: lookup may fail to find
an equal key, but memory safety and once-only destruction must remain intact.

The standard library implements `Hash` for `bool`, every built-in integer, `str`, `String`, and
slices and `Vec<T>` where `T impl Hash`. Integer encodings have fixed width and string, slice, and
sequence encodings include their length before contents, so component boundaries are unambiguous.
`Map` and `Set` are not themselves hashable.

Every `Map` owns a hidden seed initialized from the target entropy boundary. `Set` uses the seed of
its owned map storage. The default implementation must not use one published fixed seed. Failure
to initialize the ordinary hidden seed terminates construction rather than adding an unrelated
recoverable I/O failure to every collection constructor. Hash output and seed values have no
public accessor and are not persistent data formats.

The target entropy boundary is package-internal standard-library infrastructure. Collection and
hashing source receives only a `u64` seed; it cannot observe an operating-system handle, syscall
number, or recoverable I/O result. On Darwin, the adapter obtains all eight seed bytes from
`getentropy` and terminates if the call fails.

## Key Stability

`Map<K, V>` and `Set<T>` require their key or element type to implement `Hash`. While a value is
stored, equality- and hash-relevant state must not change. The collections enforce the ordinary
case by never returning a readwrite borrow to a stored key or set element.

A readonly map entry exposes `&K` and `&V`. A mutable map entry exposes `&K` and `&+V`. Set
iteration exposes `&T`, not `&+T`. An owning iterator may move keys, values, or elements out only
after it owns the complete collection.

## Mapping Literals

Mapping literals are declaration-driven typed literals owned by the compiler-checked
[`Map` contract](index.nct), not compiler knowledge of `Map`.

The use-site form is:

```nct
use std/string.String

let values = Map [
    String "one": 1,
    String "two": 2,
]

let empty = Map<String, i32> [:]
```

A string expression has type `&str`; an owning string key therefore uses the `String "..."`
literal shown above. Generic arguments may still be omitted because those key expressions identify
`String` and the values identify `i32`.

There is no bare mapping literal. `Map` may omit its generic arguments only when the key and value
expressions or an expected result type determine both uniquely.

`...entries: K: V` is a keyed argument pack. One entry contains one owned `K` and one owned `V`;
its grammar, body operations, ownership, cleanup, forwarding, and ABI are defined only in
[Argument Packs, Literal Definitions, and Sequence Spread](../../../spec/language/literals-and-packs.md).

Mapping literal entries are evaluated key first and then value, from left to right. An equal key
already present in the same literal retains its first stored key object and replaces its value
with the later value. The replaced value and the later redundant key are each destroyed exactly
once. Set literals use the existing sequence literal; a duplicate keeps the first stored element
and destroys the later equal element.

An ordinary mapping or set literal allocates through the current aborting allocation context and
accepts the existing `using allocator_place` override. Recoverable construction uses named
`try_from_entries` or `try_from_items` APIs rather than a second literal syntax.

## Map Surface

`std/map` exports `Map<K, V>` where `K impl Hash`; explicit import is
`use std/map.Map`. Future prelude exposure is defined in
[Modules, Use Declarations, and Source Visibility](../../../spec/language/modules.md#synthetic-standard-prelude).

The exact construction and observation surface is owned only by
[`std/map/index.nct`](index.nct).

`capacity` is the minimum number of entries that can be held without allocation; it is not a
bucket count. `reserve(additional)` ensures capacity for `len() + additional` entries. Capacity
arithmetic is checked and never wraps.

Insertion of a new equality class stores its key and value. Insertion of an equal key retains the
existing stored key, replaces only its value, and returns the old value. The incoming equal key is
destroyed once. `try_insert` leaves the map unchanged when allocation fails; its consumed inputs
are cleaned up normally. Removing an entry returns its value and destroys its key. `clear` retains
the allocation and hidden seed for reuse.

Borrowed lookup takes `&K`. Heterogeneous borrowed lookup, entry handles, drain APIs,
retention callbacks, custom hashers, and raw-table access are separate future designs.

## Indexing and Equality

`Map` supplies the readonly, readwrite, and equality operators declared by its
[public contract](index.nct).

Indexing an absent key terminates through the ordinary bounds-style safety boundary. `get` and
`get_mut` are the partial operations. Assignment through `map[&key]` updates an existing value and
never inserts, allocates, or constructs a default value. New keys require `insert`.

Map equality is independent of capacity, seed, bucket placement, and iteration order. Two maps are
equal exactly when they contain equal key classes whose corresponding values are equal. `K impl
Hash` already supplies the equality prerequisite; the operator adds only the value equality
requirement.

`Set` equality likewise ignores capacity, seed, bucket placement, and iteration order. No
associative collection defines `<`. Set union, intersection, difference, and symmetric difference
begin as named methods rather than expanding the closed operator grammar solely for one API.

## Iteration

Map iteration uses the public semantic entry values and expansion operators declared by
[`std/map/index.nct`](index.nct), rather than exposing table slots.

Each iterator implements `Iterator` and `ExactSizeIterator`. Readonly iteration yields
`MapEntryRef<K, V>`, mutable iteration yields `MapEntryMut<K, V>`, and owning iteration yields
`MapEntry<K, V>`. Every current entry is yielded exactly once in unspecified order. The ordinary
loan rules prevent structural mutation while a borrowed iterator is live.

## Set Surface

`std/set` exports `Set<T>` where `T impl Hash`. Its compiler-checked
[public contract](../set/index.nct) parallels map storage without exposing a dummy value.

`insert` returns `true` only when it adds a new equality class. A duplicate retains the existing
element and destroys the incoming value. `remove` returns whether an element was present and
destroys a removed element.

`Set` deliberately has no readwrite expansion or index operator. Mutating a stored element could
invalidate its hash and equality class without relocating it. The standard implementation owns
one private map storage engine; it does not maintain an independent set table algorithm.

## Allocation and Failure Atomicity

Ordinary constructors, literals, `reserve`, and `insert` use the aborting allocation policy.
Recoverable constructors and mutations use an explicit `TryAllocator`, following the same owner
affinity as `Vec` and `String`. Once storage is bound to an allocator, later growth uses that same
owner.

Every recoverable operation either commits one complete new collection state or leaves the prior
state observable. All fallible capacity work completes before an input key or value becomes part
of the collection. Internal ownership and metadata, not caller discipline, identify every
initialized key and value at every transition.

Hashing and equality bodies do not return failure. If an authored body terminates, the program
terminates; the collection does not attempt recovery from partially executed user code.

## Non-goals

This contract does not add ordered maps, insertion-ordered maps, heterogeneous lookup, custom hasher
selection, stable hash serialization, concurrent collections, weak keys, multimap semantics,
entry handles, mutable set iteration, or collection-specific union operators. None of those
features may be approximated through a compatibility alias or by exposing the private table.
