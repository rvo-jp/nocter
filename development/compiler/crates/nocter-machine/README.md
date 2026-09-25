# nocter-machine

## Responsibility

Close machine layout and lower validated MIR into an optimized target-independent machine program
with explicit ABI transport, storage, linkage, primitive dependencies, and trusted function
imports.

## Contract

The crate consumes `MirProgram`, concrete semantic representations, the selected runtime contract,
and target machine facts. It publishes immutable machine layouts and machine operations. It does not
select physical registers, encode instructions, write Mach-O, or reinterpret semantic declarations.

## Internal Responsibilities

- stored layout, aggregate representation, and ABI-owned opaque runtime-storage layout
- immutable-static serialization and data-to-data relocation construction
- call/result ABI classification and transport shared by primitive and imported calls
- one canonical imported-service identity domain retained independently of MIR
- stack objects, machine control flow, and dataflow
- one mutable body-draft boundary, exhaustive operation-effect classification, target-independent
  optimization, semantic-value/physical-storage identity, immutable body freeze, and
  post-optimization dataflow construction
- deferred function execution, suspension frames, and frozen cancellation/output destruction
- whole-program propagation of runtime-contract-owned ambient capability requirements
- explicit process-root ownership and output storage for a deferred executable entry
- structural copy/destruction expansion
- deterministic linkage and primitive dependency closure

## Invariants

- Layout is computed once and reused by every machine consumer.
- Generic member placement is keyed by the complete concrete type plus semantic member identity;
  distinct specializations cannot overwrite one another's field, variant, payload, or capture
  correspondence.
- Compiler-owned storage consumes the runtime role's ABI layout directly and remains an explicit
  `RuntimeStorage` layout and destruction kind; Machine never projects native fields, disguises it
  as an empty source struct, or repeats native offsets.
- Machine code cannot reach checking or target-program storage.
- Every ordinary or compiler-generated body uses the same draft-to-freeze path. Optimization runs
  before immutable tables and liveness exist, so a backend cannot observe a mutable draft or stale
  pre-optimization dataflow.
- Reachability and execution-resource retention form one Machine-owned closure. Blocks, operations,
  values, stack objects, addresses, drop flags, and pack state are compacted together, and every
  surviving reference is rewritten before immutable tables expose dense identities.
- An operation is removable only when Machine's exhaustive effect authority proves it pure and
  non-trapping. Calls, ownership changes, cleanup, suspension, and safety traps are conservative
  barriers without a stronger explicit proof.
- Local storage forwarding consumes an explicit same-block, whole-stack proof and feeds aliases
  into the common dense remapper. It never crosses a call, block edge, indirect store, or mismatched
  value representation, and it does not reclassify general address evaluation as non-trapping.
- Stores and their stack/address domains disappear only when the whole non-parameter object has no
  surviving projection, escape, destruction, deferred-state, cancellation, region, pack, switch,
  or unforwarded-read use. The proof removes operations through the common pruning authority.
- Drop-flag writes coalesce within one block only while no callable boundary can observe cleanup
  state. The final write before control transfer remains part of the immutable Machine body.
- Load or address materialization is locally non-trapping only when its direct stack address has no
  dynamic projection, its complete constant offset and fixed-index path is proven in range, and its
  stored extent remains aligned inside the stack object. Dereferences, views, dynamic offsets, and
  unresolved indexes retain the conservative trapping classification.
- Machine may resolve an SSA index into a constant machine representation, but it preserves the
  required, proven-in-bounds, or proven-trap disposition frozen by checking and transported by MIR.
  It neither strengthens nor locally revalidates that disposition: checking may have proved an
  access from path facts that are intentionally absent from Machine. Structural indexing and
  checked address paths share this contract, and targets consume it without repeating
  source-safety analysis or erasing a trap.
- Integer arithmetic carries the frozen checking disposition unchanged. `Required` remains
  observable and cannot be removed; `ProvenSafe` is removable when unused; `ProvenTrap` remains an
  unconditional trap. Machine does not inspect constants or ranges to change that classification.
- Representation-preserving operations may retain distinct semantic value and type identities while
  naming one proven physical-storage identity. Machine validates that relation after dense pruning;
  targets consume it directly and cannot rediscover semantic equivalence or erase the typed value.
- ABI rules are represented in machine contracts, not duplicated by the ARM64 encoder.
- Runtime symbols identify already selected items and never drive semantic lookup.
- Imported calls retain only a dense machine import identity; their catalog descriptor is stored
  once in the program and cannot be rebuilt from source spelling.
- Each reachable static retains its declaration identity as one addressable data object; text
  payloads may be shared, but equal static values cannot be merged.
- Machine projects MIR suspension fields and cleanup plans to dense identities without repeating
  liveness or ownership analysis.
- Deferred invocation and its state-machine body remain distinct from the ordinary callable ABI.
  The initial allocation-backed representation always requests an incoming allocation context.
- Machine computes the least fixed point that carries hidden allocation and process contexts to
  direct calls and callbacks, but it does not decide which primitive roles consume those contexts.

The cross-stage boundary is documented in
[Machine Program and Native Target Design](../../../architecture/pipeline/machine-program.md).
