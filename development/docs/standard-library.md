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
| generic initialized-prefix storage | `std/vec` |
| iterator protocols and stateless operations | `std/iter/core`, `std/iter/ops` |
| stateful iterator adapters | focused modules under `std/iter/` |
| OS-independent file ownership | `std/io` |
| buffered byte state | `std/io_buffer` |
| path validation and lexical operations | `std/path` |
| numeric parsing and formatting | `std/num`, `std/fmt` |
| process-state validation and ownership | `std/process` |
| target-specific system boundary | target-gated declarations in `std/os` |

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
They are available only inside the `pub(nocter)` trust boundary and are validated as one coherent
runtime capability set before lowering.

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

## Iterator Architecture

Protocol declarations live in `std/iter/core`. Stateful adapters live beside their state machines;
terminal operations that need no new state live in `std/iter/ops`. This prevents the core protocol
module from owning unrelated algorithms or creating module cycles.

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
