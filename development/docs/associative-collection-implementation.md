# Associative Collection Implementation Boundary

## Purpose

This document owns the cross-responsibility implementation boundary for v0.21.0 associative
collections. Public behavior belongs only to
[Associative Collections](../../spec/27-associative-collections.md). Phase scope and completion
belong to the [v0.21.0 milestone](../milestones/v0.21.0.md).

## Responsibility Split

```text
keyed-pack syntax
    -> checked keyed-entry stream
        -> TargetProgram pack ABI decision
            -> MIR and Machine transport
                -> target descriptor mechanics

Hash implementation source
    -> opaque HashState
        -> private final hash value
            -> private table probing

Map public surface
    -> private table contract
        -> RawBuffer and allocator contracts

Set public surface
    -> Map-owned private table contract
```

The frontend owns authored syntax, key/value types, evaluation order, ownership, cleanup, and
selected literal construction. TargetProgram owns the complete keyed-pack ABI plan. MIR and
MachineProgram transport that plan and cannot rediscover pair structure from types or source.

`std/hash` owns hash composition and algorithm policy. A target boundary supplies seed bytes but
does not hash user values. The private table consumes only the hash-state result and equality
dispatch selected by checking; it cannot resolve an interface or reinterpret source declarations.

`std/map` owns public associative meaning. Its internal table owns dense key/value storage, bucket
metadata, probing, growth, and removal repair. Vec and pointer helpers know initialized movement or
replacement but do not know buckets, keys, equality, or collection policy. `std/set` delegates
storage behavior to that same table contract and does not copy its probing or cleanup algorithm.

## One-Way Contracts

- Syntax publishes one keyed-pack syntax node; downstream layers do not rescan colon tokens.
- Checking publishes one typed key/value entry contract and exact `Hash`/equality evidence;
  lowering does not perform lookup again.
- TargetProgram publishes one descriptor and cleanup plan; MachineProgram does not infer ABI from
  a `Map` name or pack element layout.
- HashState accepts logical contributions; user code cannot inspect its seed, algorithm state, or
  result.
- The table publishes representation-neutral insertion, lookup, removal, reserve, iteration-step,
  and destruction operations to `Map` and `Set`.
- Map and Set iterators consume table cursor capabilities; they do not inspect control-byte
  encoding directly.
- The table consumes allocator and raw-memory contracts; it never reads allocator implementation
  fields or target syscall results.

## Dense Ownership and Bucket Metadata

The private table separates ownership from lookup metadata. Parallel dense `Vec<K>` and `Vec<V>`
stores own exactly `len` initialized keys and values. A bucket is only `empty`, `deleted`, or an
occupied dense index. Bucket rebuilding hashes dense keys into a prepared metadata range and never
moves, copies, or destroys user values.

The table alone maintains the equal-length dense-store invariant. Insertion reserves bucket, key,
and value capacity before either input is published. Removal applies the same swap removal to both
dense stores, destroys the removed key, returns the removed value, and repairs the one bucket that
points to a moved last entry. Clear destroys both initialized prefixes and resets bucket metadata
without changing the seed or releasing retained capacity.

Public capacity is the minimum of key, value, and usable bucket capacity. A recoverable reserve may
prepare one internal store before a later allocation fails, but the published minimum and logical
entries remain unchanged. Capacity arithmetic uses checked standard-internal helpers. No caller
supplies an initialized count, bucket index, or repair obligation.

## Seed Boundary

The target entropy operation returns seed material or terminates. It does not return a public
`error`, allocate, initialize HashState, or choose the algorithm. Standard hash source combines the
seed with its private state. Each Map retains the resulting seed across reserve and clear; rehash
does not request a new seed.

The target operation is justified only because ordinary Nocter source cannot obtain private OS
entropy without turning Map construction into public file or syscall policy. If a target already
has a suitable standard-internal entropy source, the hash module must use that source instead of a
second primitive.

## Hash Foundation

`std/hash` owns the public opaque `HashState` and the complete private streaming algorithm. The
public contract permits only byte contribution. Package-only construction, restart, and finalization
remain associated with the same type owner, so another module never initializes or interprets its
fields. A retained template owns one hidden seed; each lookup restarts an independent state from
that seed without requesting new entropy.

The current private implementation buffers eight bytes and compresses them through a keyed
add/XOR/rotate state. Its constants, lane count, block size, and finalization are not contracts and
may change. Standard scalar implementations contribute their exact fixed-width initialized bytes
through type-specific helpers. Text and sequences contribute a length before their bytes or
elements, and owning `String`/`Vec<T>` delegate to their borrowed views. This keeps component
boundaries in standard source rather than in the target or table.

`std/internal/hash` owns only target entropy acquisition. It returns seed material to `std/hash`
and cannot construct or finalize `HashState`. The private table consumes the package-only
`HashState` lifecycle; it cannot inspect the seed or algorithm state.

## Capability Audit

The source spike found the following reusable contracts:

| Need | Existing owner |
| --- | --- |
| allocation owner and recoverable growth | `std/mem` |
| checked size/capacity arithmetic | `std/internal/mem` |
| dense initialized storage and swap removal | `std/vec` |
| initialized-place replacement | `std/internal/ptr` |
| structural equality prerequisite | declaration/checking capability evidence |
| readonly/readwrite index selection | checked instance operations |
| readonly/readwrite/owned expansion | checked instance operations and iteration plans |
| exact-size iteration | `std/iter.ExactSizeIterator` |

The spike found these exact gaps:

| Gap | Owning change |
| --- | --- |
| key/value entry syntax and body iteration | grammar and syntax |
| one typed keyed-pack identity and cleanup | declaration/checking model |
| keyed descriptor ABI | TargetProgram, MIR, MachineProgram, target backend |
| source-expressible nontrapping hash mixing | primitive numeric surface and backend operations |
| hidden default seed acquisition | standard-internal target boundary |

The table algorithm, Map/Set declarations, hash algorithm, collision policy, and iterator cursor
are not compiler gaps.

## Enforcement

Architecture and conformance tests for the implementation phases must reject:

- compiler branching on the source names `Map`, `Set`, `HashMap`, or `HashSet`;
- a second keyed-pack interpretation below checking;
- a Machine or backend decision based on semantic interface lookup;
- public bucket, probe, control-byte, hash-result, or seed access;
- separate Map and Set probing implementations;
- iterator code that reads private table metadata without a table cursor contract;
- cleanup that trusts a caller-maintained initialized-slot count;
- a recoverable rehash that moves an entry before all fallible preparation succeeds.
