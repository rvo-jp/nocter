# Standard Library Implementation

This document owns implementation architecture and runtime invariants for the source tracked under
`development/std/` and packaged as `.nocter/std`. Public modules, signatures, and behavior are
defined by the responsibility-specific chapters indexed by
[Standard Library, Primitives, and OS](../../spec/11-stdlib-primitives-os.md) and by
[Practical Standard Library](../../spec/21-practical-standard-library.md).

## Responsibility Boundaries

| Area | Implementation owner |
|---|---|
| allocation layout, raw storage, allocator policy | `std/mem` |
| owned UTF-8 storage | `std/string` |
| built-in UTF-8 view methods and representation queries | `std/str` |
| shared UTF-8 byte search | `std/string/search.nct` |
| validated borrowed text ranges and cursors | `std/string/views.nct` |
| generic initialized-prefix storage | `std/vec` |
| built-in slice methods and representation queries | `std/slice` |
| iterator public contracts | `std/iter/index.nct` |
| iterator adapters and terminal operations | `std/iter/core.nct`, `ops.nct`, and `sources.nct` |
| portable byte-stream contracts and derived operations | `std/io/index.nct` and `core.nct` |
| OS-independent file ownership | `std/io` |
| buffered byte state | child module `std/io/buffer` |
| path validation and lexical operations | `std/path` |
| numeric parsing and static `Format` conformances | `std/num`, `std/fmt` |
| process-state validation and ownership | `std/process` |
| target-specific system boundary | target-gated declarations in `std/internal/os` |

The compiler does not infer behavior from public names such as `String`, `Vec`, `File`, `Iterator`,
or `Allocator`. Trusted primitives and protocol roles are resolved to validated declaration
identities. Ordinary standard-library algorithms remain Nocter source.

## Buffer Ownership

`RawBuffer` owns byte storage, allocator backend identity, and storage origin. `String` and
`Vec<T>` embed that one representation instead of maintaining independent allocation machinery.

The common invariants are:

- zero capacity uses a canonical allocation-free representation
- checked layout arithmetic completes before allocation or element relocation
- failed growth leaves pointer, length, capacity, initialized elements, and allocator identity
  unchanged
- storage publication occurs only after initialization succeeds
- storage is released through the backend that created it
- private representation never becomes public construction or field access

`Vec<T>` separately tracks its initialized prefix. Moving an element into or out of storage
transfers exactly one drop obligation. Partial initialization drops the current element and then the
completed prefix in reverse order. Removal may create one compiler-proven transient hole; it never
publishes a sparse vector state.

`String` applies the same storage rules to UTF-8 bytes. Encoding validation belongs to source
operations before publication; buffer machinery does not acquire text semantics.

Owned containers expose construction, mutation, capacity management, and ownership transfer.
Borrowed observation belongs to the built-in `str` and `[T]` surfaces. Private `String` and
`Vec<T>` raw-view helpers bridge initialized representation into coercion, index-operator, and
interface implementations; they are not public forwarding APIs.

The built-in `str` and `[T]` instance surfaces own equality and strict lexical ordering in ordinary
source. `String` and `Vec<T>` obtain both capabilities only through their readonly coercions. The
compiler and standard nominal containers do not maintain parallel comparison tables or algorithms.

## Borrowed Text Views

`std/string/index.nct` owns the public contracts for UTF-8 ranges, `SplitIter`, and `LinesIter`.
`views.nct` supplies their validation and state-machine bodies without defining another module or
public surface. Shared byte search lives in `search.nct`; owned and borrowed algorithms do not carry
divergent copies of the same loop.

Ordinary source validates every public range before calling
`std/string.str_subview_unchecked`. That declaration is a closed `pub(/)` primitive in the exact
implicit standard-library package and
has one compiler-owned `BorrowedProjection { source: 0 }` role. The role is attached only when the
owning module, visibility, declaration kind, generic arity, parameter names and types, return type,
target, and `from text` clause match exactly. Typecheck instantiates the source argument's resolved
provenance; it never derives an origin from pointer arithmetic or a helper name.

IR carries `SetStrSubview { source, start, len }` as a typed operation. The backend adds the already
validated byte offset to the source pointer and preserves the requested length. It does not expose
raw-pointer reconstruction to ordinary standard-library code. Frame, spill, process-argument,
reachability, and code-generation passes handle the instruction exhaustively.

Both borrowed iterators store only view pairs, indices, and state flags. Their constructors,
steps, and lazy iterator adapters allocate nothing. Distributed execution replaces the normal
allocator with an aborting sentinel and exercises both state machines and adapter dispatch, so a
future accidental allocation fails the gate at runtime.

## Allocation Policy and Regions

Recoverable allocation is the implementation core. Aborting allocation adapts that core only after
checked arithmetic and failure-atomic state preservation. The two policies do not duplicate
collection algorithms.

An allocation context carries backend, failure policy, and storage origin as separate facts.
Lexical child regions derive from an established parent context and release their owned storage at
region exit. Owned results retain the selected origin so compiler escape checking can reject both
direct and aggregate-hidden region escapes.

The runtime must not allocate an error while handling aborting allocation failure and must not
unwind partially initialized Nocter values through a second failure mechanism.

## Target Boundary

Target-specific declarations expose the minimum primitive operations needed by portable source.
They are package-visible only inside the implicit `std` package and are validated as one coherent
runtime capability set before lowering. Package visibility and trusted runtime authority remain
separate semantic facts.

The `arm64-darwin` entry path retains process arguments and environment pointers in reserved
runtime state. `std/process` performs bounds checks, UTF-8 validation, environment-name matching,
allocation selection, and error construction in Nocter source. Backend code never searches for
public process function names.

File opening lowers through one compiler-owned operation carrying path, flags, and mode as distinct
values. Source modules own path validation and open policy. File handles are move-only owners;
close transfers them to an inert state so later drop cannot close a reused descriptor.

## Buffered I/O

Buffered readers initialize backing storage before exposing a mutable view to an underlying reader
and retain unread byte ranges across calls. End-of-file is represented without publishing
uninitialized bytes.

Buffered writers remove bytes from pending state only after the underlying write succeeds. Explicit
close performs fallible flush before closing. Drop does not attempt an unreportable flush; it only
releases owned state according to ordinary destruction rules.

Reader and writer dispatch uses resolved interface identities and static specialization. The
standard library does not retain duplicate inherent forwarding methods, and the compiler contains
no I/O type-name table.

Whole-stream reads reuse one initialized scratch buffer. Each successful count is validated against
that buffer before its prefix is copied into owned result storage. Byte collection finishes only on
a zero count; text collection validates UTF-8 after the complete stream is available. This keeps
EOF, I/O failure, protocol violation, allocation, and text validation as separate boundaries.

## Iterator Architecture

Protocol declarations and public adapter contracts live in `std/iter/index.nct`. Stateful adapter
bodies live in `core.nct`, zero- and one-element sources live in `sources.nct`, and terminal
operations live in `ops.nct`. These are implementation sources of one module, not importable child
modules. This keeps the complete public surface in the module root without mixing implementation
state machines into it.

Adapters own their sources and callbacks as ordinary values. They allocate nothing unless a public
collection operation explicitly builds an owned collection. Early exit drops the current yielded
owner before the remaining adapter/source state. Exact-size conformance is exposed only when every
required source can prove an exact remaining count; filtering never inherits exact cardinality.

Collection builders treat iterator output as authoritative. An exact-size hint may reserve
capacity, but under-reporting continues through checked growth and over-reporting yields a shorter
valid collection.

## Verification Boundary

Distributed-home tests are the executable authority for implementation invariants. They must cover:

- zero-capacity values and final storage release
- failed growth with byte-for-byte and ownership-state preservation
- nested move-only values and reverse-order partial cleanup
- region-backed direct and aggregate escape rejection
- handle close-once behavior after descriptor reuse
- iterator exhaustion, early exit, propagation, and unconsumed-suffix cleanup
- malformed UTF-8, paths, arguments, and environment data
- short reads, partial writes, explicit flush, reopen, and append
- repository-home and packaged-home agreement without repository-local environment configuration

Compiler snapshots or backend instruction inspection alone do not prove these invariants. Public
API conformance is checked against the specification-owned surface; this document records only how
the implementation preserves it.
