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

`std/map` owns public associative meaning. Its internal table owns storage layout, probing, growth,
and sparse initialization metadata. Raw memory helpers know byte addresses and typed movement but
do not know buckets, keys, equality, or collection policy. `std/set` delegates storage behavior to
that same table contract and does not copy its probing or cleanup algorithm.

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

## Sparse Initialization

Table metadata is the sole authority for whether a key and value slot is initialized. Length is an
observation derived from committed occupied metadata, not a second initialization map. A table
transition follows three states:

1. replacement storage and empty metadata are fully allocated;
2. entries move one at a time, with the destination marked occupied only after both key and value
   are initialized and the source marked transferred before another fallible boundary;
3. the old storage is released only after every occupied source slot has transferred.

Allocation, layout, and capacity checks complete before step 2. No equality, hashing, allocation,
or other fallible/user-authored operation occurs after movement begins. Destruction walks the same
metadata authority and therefore cannot depend on a caller-supplied initialized count.

## Seed Boundary

The target entropy operation returns seed material or terminates. It does not return a public
`error`, allocate, initialize HashState, or choose the algorithm. Standard hash source combines the
seed with its private state. Each Map retains the resulting seed across reserve and clear; rehash
does not request a new seed.

The target operation is justified only because ordinary Nocter source cannot obtain private OS
entropy without turning Map construction into public file or syscall policy. If a target already
has a suitable standard-internal entropy source, the hash module must use that source instead of a
second primitive.

## Capability Audit

The source spike found the following reusable contracts:

| Need | Existing owner |
| --- | --- |
| allocation owner and recoverable growth | `std/mem` |
| checked size/capacity arithmetic | `std/internal/mem` |
| typed size and alignment | `std/internal/ptr` |
| arbitrary sparse store/take/drop | `std/internal/ptr` |
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
