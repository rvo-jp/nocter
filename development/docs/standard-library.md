# Standard Library Runtime

This document records the implementation tracked in the repository and packaged under
`.nocter/std`. [Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md) is the
authority for public API semantics; this document adds no specification rules.

## Released Runtime Baseline

| Module | Current role | v0.2.0 result |
|---|---|---|
| `error` | structured recoverable errors | stable allocator and collection error IDs |
| `fmt` | scalar and text formatting helpers | owning-text behavior verified |
| `io` | file open/read/write/close and stdout/stderr | deterministic handle ownership |
| `mem` | `Layout`, `RawBuffer`, `Allocator`, page boundary | complete layout/grow/free contract |
| `os` | target-gated syscall boundary | restricted to allocator internals |
| `prelude` | implicit common declarations | unchanged for v0.2.0 |
| `process` | exit/abort/cwd/args; env is check-only | maintained where allocator completion requires it |
| `ptr` | restricted pointer primitives | retained within the `pub(nocter)` trust boundary |
| `string` | owned UTF-8 bytes | common allocator and failure-atomic growth |
| `vec` | owned generic sequence | non-copy initialized-prefix drop and pop |

### Shared buffers

`std/mem` provides checked `Layout`, a canonical empty buffer, private allocator provenance,
failure-atomic growth, and deterministic `RawBuffer` drop. Distributed-home runtime tests fix the
alignment, zero-size, out-of-memory, and failed-growth preservation behavior.

`String` stores allocator provenance and capacity in a private `RawBuffer`. Its common Allocator
supports empty, with_capacity, from/copy, view, len/capacity, reserve, clear, push_str, and
deterministic storage release. Runtime tests prove content, length, and capacity preservation after
failed growth.

`Vec<T>` also centralizes byte capacity and allocator provenance in private `RawBuffer`; its typed
pointer is a non-owning alias. Empty, with_capacity, from_slice, len/capacity, reserve, push, clear,
views, and storage release use the common Allocator. Non-copy push transfers ownership into storage;
clear and drop recursively destroy the initialized prefix in reverse order; pop transfers the final
obligation into the return value. Phase 2 adds optional borrowed access, insertion/removal, and
readonly/consuming iteration while retaining the same initialized-prefix model.

An externally observable test proves that a descriptor number reallocated after
`Vec<File>.clear()` remains readable after vector drop, fixing close-once behavior. A nested-vector
test proves that an inner `String` remains recoverable and usable after failed growth.

### v0.2.0 fixed behavior

### `std/mem`

- checked `Layout` construction
- canonical empty allocation state
- allocator identity retained through allocation, growth, and free
- recoverable overflow, invalid alignment, and out-of-memory errors
- failure-atomic growth that retains the old allocation
- representation fields hidden outside `pub(nocter)`

### `std/string`

- empty / with_capacity / from_str / copy
- len / capacity / is_empty / view / bytes
- reserve / push_str / clear / drop
- agreement between UTF-8 view and owned storage after repeated growth
- unchanged content, length, and capacity after allocation failure

Unicode scalar/character indexing and normalization are not v0.2.0 criteria. Do not add an
ambiguously bounded byte-indexing API.

### `std/vec`

- empty / with_capacity / reserve / push / clear / drop for both copy and non-copy elements
- from_slice and immutable/mutable views for copy elements
- pop as ownership extraction from the end
- recursive drop of nested owning elements
- atomic behavior on capacity overflow and allocation failure

The meaning of duplicating non-copy elements from a borrowed slice is not defined, so `from_slice`
is limited to copyable `T`. Until the type system can express that constraint, the compiler keeps
the public boundary narrow and rejects misuse with a source-backed diagnostic.

## Phase 0 Allocator and Region Runtime

The completed Phase 0 implementation retains the v0.2.0 buffer, initialized-prefix, recursive-drop,
and failure-atomic publication invariants while changing how allocation failure is exposed.

`std/mem` provides one recoverable core and one aborting adapter:

- `TryAllocator` and `try_*` operations return stable `std.mem.*` errors
- `Allocator` and normal operations terminate without unwinding on allocation failure
- the root allocation context uses the aborting system allocator
- `RawBuffer` keeps backend identity and storage origin independently of failure policy
- region child allocators derive from an established aborting parent context

`String` and `Vec<T>` provide paired policy surfaces. Normal constructors and growth use the current
allocation context and do not return allocation-only `T!`. Explicit `try_*` operations retain
recoverable failure and the old failure-atomic state guarantees.

Do not duplicate collection algorithms between the two surfaces. The aborting path adapts the
fallible core after it has performed checked arithmetic and preserved the old value.

## Stable Acceptance Baseline

| Scenario | Required observation |
|---|---|
| `String` repeated growth | bytes preserved; one final storage free |
| failed `String.reserve` | pointer/content/len/capacity unchanged |
| `Vec<String>` growth | every string remains usable and drops once |
| `Vec<String>.pop()` | returned string remains owned after vector drop |
| `Vec<File>.clear()` | initialized handles close once; later vector drop is empty |
| `Vec<Vec<String>>` early `?` | completed prefixes unwind in reverse order |
| zero-capacity values | no allocation and no invalid free |
| lexical region storage | mapping remains live in the body and is released at region exit |
| region-backed aggregate/error | direct and indirect escape rejected before lowering |
| packaged-home execution | behavior matches repository-local source |

Tests observe semantic effects such as handle closure, output, error identity, and post-operation
state. Backend instruction snapshots alone do not prove the standard-library contract.

## Phase 0 Boundary

Phase 0 includes the root allocation context, aborting/recoverable policy split, lexical region
runtime, and storage-origin propagation through existing owning types. It intentionally left typed
literals and per-literal `using` to Phase 1.

## Phase 1 Typed Construction

Distributed `std/vec` and `std/string` now declare their construction syntax in ordinary Nocter
source:

```nct
pub literal Vec<T> [](...items: T): Self { ... }
pub literal String ""(text: &str): Self { ... }
```

`Vec [..]` evaluates and transfers its owned elements from left to right, reserves exactly the
non-empty element count, and uses the canonical allocation-free empty state for `Vec<T> []`.
`String ".."` copies the static `&str` payload into owned storage. Both bodies use only public
collection construction APIs and inherit the current allocation context unless the expression has
an explicit `using` override.

Packaged-home native tests cover inferred scalar vectors, empty vectors, owned `String` elements,
reverse-order pack cleanup, stable allocation-abort status, lexical-region construction, explicit
root-context construction inside a child region, child-origin escape rejection, and OS-observed
release of `Vec` literal storage.

## Phase 2 Collection Access and Iteration

Distributed `std/iter` and `std/vec_into_iter` provide two deliberately separate cursor modes:

- `ViewIter<T>` borrows a contiguous readonly view, allocates nothing, and returns `(&T)?`
- `VecIntoIter<T>` consumes `Vec<T>`, returns owned `T?`, drops its unconsumed suffix in reverse
  order, and releases the transferred raw storage once

`Vec<T>` now exposes `get`, `get_mut`, `insert`, `try_insert`, `remove`, `iter`, and `into_iter`.
`String.bytes_iter()` yields borrows to exact UTF-8 bytes and makes no Unicode scalar or grapheme
claim. `try_insert` performs every fallible step before element relocation and publishes `len`
last. Move-only insertion and removal use one transient uninitialized slot, never a sparse public
state.

Packaged-home tests observe scalar and move-only source order, exact readonly element addresses,
byte order, source-loan retention, mutation visibility, failed-growth state preservation, region
escape rejection, and cleanup across exhaustion and early exits. Remaining later work includes
spread, interpolation lowering, environment retrieval, rich path APIs, collection `for`, iterator
interfaces/adapters, Unicode text APIs, and general allocator plugins.
