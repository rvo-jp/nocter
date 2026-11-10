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
- private value-model lowering through the common layout, ABI and executable body authorities
- private activation objects, lifecycle guards and shared concrete cleanup helpers under `value_access`
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
- Layout construction accepts either MIR's selected roots or a closed runtime environment's
  explicit storage roots. Value-model field correspondence preserves its own identity domain;
  only the construction plan retains semantic member indexes. Private input planning consumes
  data and input operation separately and retains that operation alongside derived ABI transport.
- Private lowering consumes complete closed private MIR bodies and emits ordinary Machine bodies,
  including computation, control, calls and resource phases. Payload/address slots become abstract
  stack objects; final frame offsets belong to the common target frame placer. Exact typed
  projection addresses remain distinct even at equal byte offsets. Lifecycle predicates become
  logical drop flags; an action selects its alternative before either phase and retains that
  continuation through normal call completion. Cleanup helpers are shared per concrete data shape.
- Data identity and physical carrier are independent. Address values and stack objects retain the
  referent data identity but use the selected pointer ABI. Forwarding, storage alias proofs and
  dense remapping must preserve the carrier; no synthetic source pointer type supplies it.
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
  observable and cannot be removed. `NonZeroDivisor` retains exactly the selected remainder
  contract's divisor check and is also observable; `ProvenSafe` is removable when unused; `ProvenTrap` remains an
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

## Private Frozen Data

The private body consumer serializes MIR's established frozen literal DAGs into the common
`MachineDataTable`. Exact runtime field identities feed the common stored-layout authority; scalar
encoding shares the released static encoder's byte-order operation. Record declaration order and
tuple index order,
padding, signed bytes and zero-size components follow those layouts. No source initializer or
constant evaluation runs here. A literal use loads immutable backing into its own ordinary value
storage, preserving independent mutation without a source Copy requirement.

The private catalog shares equal type/graph keys in deterministic body/action order. It bounds
total serialized bytes before allocation and reports `ExecutableFailure::DataLimit`, including
allocation refusal. The current byte budget is owned by its constructor. Zero-size subgraphs are
skipped instead of expanding exponentially shared empty aggregates. Frozen graph sharing does not
justify overflowing a physical layout or allocating unbounded serialized output. This private
bound does not change the released static catalog or claim a compiler-process memory limit.

Private immutable statics use that same serializer, with a catalog entry per global identity rather
than literal-content merging. Every reference across bodies selects that same object. Empty statics
receive distinct minimal physical backing while retaining their zero-size logical layout. Entry
code binds body-local address slots to immutable backing; callable signatures and source ownership
remain unchanged. Static and literal data share the total serialization budget and common scalar
encoding. No static initializer becomes a runtime function.

## Private Array Execution

The private source path qualifies listed/repeated arrays, complete read/edit selections and
replacement through ordinary Machine bodies and the native consumer. Storage projects one shared
`RuntimeType::FixedArray` element/count descriptor. The common layout and value-input ABI own
stride, geometry and direct/indirect transport. Resolving a logical `ValueField::Array` computes one
checked offset from that descriptor, without an element-sized layout map.

`executable::body::arrays` materializes each evaluated index into a saved checked address. It uses
the common required Machine index step, then stores the pointer for an access result or snapshots
the selected Copy payload. Later uses and assignments consume that saved pointer; they do not
reload the user's index variable or repeat the source bounds decision. Zero-byte selections still
materialize their required bounds effect. Existing lifecycle phases retain RHS-first replacement,
interruption cleanup, calls and independent Copy snapshots.

`executable::body::repetition` loads the established seed once and emits a counted Machine CFG.
Its countdown counter is an ordinary SSA block parameter. Fill order has no user callback; the
established Copy seed is loaded before traversal. One homogeneous layout proves the compiler's byte
offset and successor arithmetic; internal traversal does not manufacture source bounds proofs.
The runtime environment supplies canonical usize/bool roles for that control. Count one uses the
ordinary resource assembly, and zero-byte storage requires no physical fill while preserving seed
evaluation and cleanup. Native cases cover seed effects, non-Copy count-one transfer, nested and
wide Copy payloads, interrupted seeds and maximum-count zero-byte arrays. Generated operation
counts remain equal for two and 100,000 nonzero-sized elements; no per-element owner, layout field
or generated write is introduced.

The frozen-data serializer accepts listed and sparse repeated array DAGs against that same layout.
Physical repeated storage is emitted with one traversal cursor per nesting level, rather than N
pending entries. Zero-byte subgraphs are skipped, including maximum-count unit arrays. The existing
serialized-byte budget is checked before physical backing allocation; this is no source cardinality
limit. Native qualification covers nested records/arrays, scoped saved indices, loop re-execution,
read/edit/take calls, direct and indirect array returns, scalar/zero-byte bounds traps, source order,
frozen constants/statics and independent mutation.

Empty arrays use the [private stored-layout contract](../../../architecture/pipeline/value-access-declarations.md#private-stored-representation),
retaining their element identity without requiring that payload's runtime entry or geometry.
Native qualification covers ordinary calls, Copy/replacement, zero-count seed effects and recursive
nominal payloads terminated by empty arrays. Frozen empty arrays use ordinary zero-byte literal
handling; distinct empty statics keep separate minimal addressable backing. Empty selections still
perform their required bounds trap, including when the selected nominal data is zero-sized.

Complete-array cleanup uses that same countdown traversal, calling one selected element helper in
reverse index order. Shared helpers are selected per concrete data schema, without an element-sized
call catalog or a second resource ledger. Empty arrays and elements without cleanup require no
helper. Source with a divergent error-array producer qualifies helper structure and image lowering;
it does not establish execution of an owned error graph. Actual error construction and nontrivial
resource-release effects still need source/native qualification.

Partial-array interval cleanup remains an explicit implementation gate. Precise element transfer
and restoration also require checking's separate retained-index proof. These limits do not change adopted source
semantics or qualify the ordinary released CLI as v2.
