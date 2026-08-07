# Memory, Regions, and Allocators

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Status

Nocter provides deterministic ownership, explicit fallible allocation, lexical regions, storage
provenance, and a statically propagated current allocation context for `String`, `Vec<T>`, and
`RawBuffer`. Typed literals, sequence spread, and collection iteration build on that foundation.

## Memory Model

Nocter does not use garbage collection. Its memory model combines:

- owned values and explicit moves
- readonly and readwrite borrows
- source-level non-lexical loan ranges
- deterministic drop at scope exit
- allocator-backed owned storage
- lexical allocation regions
- compile-time escape checking

Owned values are dropped when their scope ends unless ownership has moved. A storage-dependent
value may move only to a destination outlived by every storage origin it carries.

## Three Separate Concepts

Allocator, allocation context, and region are not synonyms.

- An **allocator backend** obtains and releases bytes at runtime.
- An **allocation context** selects an allocator backend, failure policy, and storage origin for an
  allocating operation.
- A **region** is a lexical child lifetime with its own allocation context and release boundary.

Borrow loans are separate again: they restrict access to source places while a borrow-like result is
live. Allocator provenance does not replace loan checking, and a runtime allocator handle is not a
source-level lifetime annotation.

## Default Allocation Context

Every executable starts with a program-lifetime allocation context backed by the standard aborting
system allocator. An allocating callable receives the current context as a compiler-propagated
capability.

The current context is not:

- a mutable process-global variable
- a thread-local lookup
- selected by searching for a standard-library name
- reconstructed by the backend from source syntax

The compiler records an execution allocation requirement for each callable and statically passes
the context only where required. Source functions infer that internal requirement from their bodies
and callees. Trusted bodyless standard-library declarations carry compiler metadata attached to
declaration identity.

Changing a helper body so that it allocates may change its compiler-owned execution requirement.
The whole compile unit contains the source and summaries required to propagate that fact. Execution
allocation and fresh storage retained by a result have no source-level annotation.

## Result Storage Contracts

Callers see only external storage relationships they must preserve:

```nct
func len(text: &str): usize
func copy(text: &str): String
func view(text: &String): &str from text
func copy_with(allocator: &+Allocator, text: &str): String from allocator
```

`from X` reports a receiver, parameter, allocator capability, or `static` origin retained by a
storage-bearing result projection. A public body that retains caller-managed storage must declare
every such origin. Absence of `from` means that the result exposes no caller-managed external
origin. It does not promise allocation-free execution or storage independence from the active
lexical region; future `noalloc` and `realtime` guarantees are outside the current language.

Body-backed summaries infer fresh storage and exact path-sensitive origins from implementations.
Bodyless abstract callables use their written `from` clause, or a conservative compiler-owned
fresh-result capability when no external origin is declared. Trusted allocation primitives use
semantic roles attached to declaration identity. None of these internal facts changes callable
type compatibility or editor signature text.

## Allocation Failure Policies

Nocter provides two policies over one fallible allocation implementation.

### Standard allocator

`Allocator` is the ordinary allocation capability. Its allocation and growth operations either
succeed or terminate the process immediately.

Allocation termination:

- is not a recoverable `T!` result
- performs no additional allocation
- does not unwind Nocter scopes or run pending drops
- uses a stable target-independent reason and abnormal process status
- must not publish a partially updated buffer or collection before terminating

This policy lets ordinary owning values and future typed literals avoid pervasive allocation-only
`?` handling.

### Recoverable allocator

`TryAllocator` is the explicit recoverable capability. Its `try_*` operations return the built-in
`error` payload and are failure-atomic.

Stable memory error codes include:

- `"std.mem.out_of_memory"`
- `"std.mem.invalid_argument"`
- `"std.mem.capacity_overflow"`

Constructing these errors must not allocate. A fixed-capacity arena, per-request budget, speculative
large operation, compiler, or server may use this path even when exhaustion of the process allocator
would be fatal.

The two policies share layout validation, backend calls, buffer publication, provenance, and cleanup
logic. Conceptually, `Allocator` is an abort-on-error adapter over the `TryAllocator` core. They are
not separate allocator implementations.

An ambient allocation context contains an `Allocator`, not a `TryAllocator`.
Recoverable allocation uses named `try_*` APIs. This prevents the type of one expression from
changing between `T` and `T!` based only on which context is selected.

## Standard-Library Boundary

Allocator APIs live in `std/mem`. `Allocator`, `TryAllocator`, `Layout`, and `RawBuffer` are ordinary
standard-library names, not compiler built-ins.

The compiler's special behavior is limited to:

- allocation-effect and storage-origin metadata on trusted declarations
- statically propagating the current allocation context
- the `region name using allocator_place { ... }` language construct
- escape checking and ordered region cleanup

`Layout` validates size and alignment before a backend call. Zero-sized allocation produces a
canonical empty buffer without invoking the OS. `RawBuffer` retains its actual allocated layout,
backend identity, and storage origin. User code cannot construct or mutate its representation.

Normal and recoverable collection APIs are paired over one implementation:

```text
with_capacity / try_with_capacity
reserve       / try_reserve
push          / try_push
copy          / try_copy
```

Normal operations use the current aborting context. `try_*` operations take or otherwise select a
`TryAllocator` explicitly. I/O, parsing, and other recoverable failures remain `T!`; only allocation
failure is removed from the ordinary allocation path.

## Lexical Regions

Syntax:

```text
region name using allocator_place {
    statements
}
```

Example:

```nct
region scratch using arena {
    let source = read_file("main.nct")?
    let tokens = lex(source.view())?
    consume(tokens.view())
}
```

`allocator_place` must resolve to an established aborting allocator or allocation-context place. It
is not an arbitrary effectful expression. The parent is evaluated and validated before the child
region is entered.

Entering the statement:

1. creates a fresh lexical region identity
2. derives a child runtime allocator from the selected parent
3. binds the immutable region handle to `name`
4. makes the child allocation context current for allocating calls in the body

The region name is an ordinary lexical binding name for lookup and diagnostics. The compiler does
not infer semantics from spellings such as `scratch`, `temp`, or `arena`.

Rules:

- The region handle exists only inside the body.
- It cannot be reassigned, moved out, returned, or explicitly dropped by user code.
- Allocator capabilities derived from the handle carry the same region origin.
- Owned storage allocated through the current child context carries the region origin.
- Borrows, views, iterators, raw pointers, and aggregates derived from that storage preserve the
  origin.
- A value can leave only when every component is proven independent of the child region.
- Pure integers, booleans, and copy aggregates containing only independent fields may leave.
- Unknown provenance cannot escape.
- A nested child is shorter than its parent and may receive parent-derived values.
- A child-derived value cannot flow into its parent or an unrelated region.

At every normal exiting edge, live values owned inside the body are dropped in reverse ownership
order before the child allocator releases its storage. This applies to fallthrough, `return`,
`break`, `continue`, and `?` propagation.

Calling a `never` function does not cause implicit cleanup. Allocation failure on the standard path
terminates immediately without region release; the operating system reclaims process resources.

## Escape Examples

Invalid owned escape:

```nct
func load_text(allocator: &+Allocator): String {
    region scratch using allocator {
        let text = String.copy("temporary")
        return move text // error: text storage belongs to scratch
    }
}
```

Invalid indirect escape:

```nct
struct ResultView {
    text: &str
}

func load_view(allocator: &+Allocator): ResultView {
    region scratch using allocator {
        let text = String.copy("temporary")
        return ResultView { text: text.view() }
        // error: ResultView carries a view into scratch
    }
}
```

Valid independent result:

```nct
func count_bytes(allocator: &+Allocator): usize {
    region scratch using allocator {
        let text = String.copy("temporary")
        return text.len()
    }
}
```

## Borrow Origins and Elision

Nocter does not expose Rust-style lifetime parameters or annotations. The compiler tracks storage
origins through values and callable summaries.

Elision and inference rules:

- A borrow-like result with one borrow-like input is tied to that input.
- A method result may be tied to its borrowed receiver when that is its only declared origin.
- A concrete body can establish that a result comes from a particular parameter, receiver, static
  storage, or multiple possible inputs.
- A result with multiple possible inputs is constrained by all of them at the caller.
- A trusted bodyless declaration must provide compiler-owned origin metadata.
- An untrusted bodyless declaration with an ambiguous borrow-like result is invalid.

A borrow returned through a call remains a loan of the original caller place through the returned
value's last source-level use. Return validation and ordinary NLL use the same callable provenance
summary.

Source-level lifetime syntax may be reconsidered only when public APIs need relationships that
cannot be expressed by these rules, such as multiple independently named regions in bodyless APIs,
higher-order functions, or separately compiled region-parameterized types.

### Explicit result provenance

An identity-based `from` clause expresses public result provenance without adding lifetime names:

```nct
pub method &self.get(key: &K): &V? from self
func choose<T>(left: &T, right: &T, first: bool): &T from left | right
```

An identifier after `from` names a receiver or a parameter whose semantic value can carry storage
provenance. This includes borrows, owning values, generic values, and allocator capabilities.
`static` denotes program-lifetime storage. Source-level `from current` is not valid; fresh ambient
result storage is compiler-owned and therefore needs no public origin name. Concrete public bodies
are checked against the declared origin set; bodyless interface methods use the clause as their
external provenance summary.

Result provenance applies both to source-level borrows and to pointer-backed owning aggregates.
Raw pointers remain outside borrow checking, but an owning `String`, `Vec<T>`, or user-defined
buffer still carries the allocation context responsible for its storage.

The execution allocation requirement remains inferred and has no source annotation.

## Typed Literal Allocation

Typed literals use the same allocation boundary:

```nct
let values = Vec [1, 2, 3]
let values = Vec [1, 2, 3] using arena

region temp using arena {
    let text = String "hello"
}
```

- Omitting `using` selects the current aborting allocation context.
- `using arena` selects an established aborting allocator/context for that literal.
- A region body changes the current context lexically and transitively for allocating callees.
- A literal allocated in a lexical region carries that region origin.
- Recoverable allocation uses named `try_*` construction rather than changing a literal's result
  type according to allocator policy.
- Bare `"hello"` remains a static `&str` and performs no allocation.

The full literal and sequence rules live in
[Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md).

## Current Non-goals

- typed literal parsing or lowering
- sequence spread, variadic capture, or embedding
- ambient recoverable allocation contexts
- fallible `region` statements
- source-level lifetime parameters
- dynamic allocator interface dispatch or arbitrary user allocator plugins
- concurrency or thread-local context semantics
- a native backend other than `arm64-darwin`
