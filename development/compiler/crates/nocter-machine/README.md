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

- Layout is computed once and reused by every machine consumer. Layout, ABI, stack/address data
  and cleanup keys retain the [runtime storage identity](../nocter-runtime-contract/README.md#runtime-storage-identities).
  Released semantic MIR identities are adapted at its boundary; a contextual identity is never
  relabelled as a semantic type.
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
- Private scoped writes and adapter exits carry original-action geometry constraints from MIR. Their complete
  before/after roots enter the same layout closure. `MachineLayoutPlan::has_same_geometry`
  conservatively checks size, alignment, ABI class and corresponding nested member geometry;
  root capacity alone cannot authorize shifted sibling fields. Callers establish canonical
  source-schema correspondence before this physical check. The iterative pair walk shares DAG
  work, compares arrays by count/stride rather than element expansion, and requires no empty-array
  element payload. Missing or incompatible layouts reject before body emission. This implements
  neither relocation nor a lifetime, source-admission or initialized-storage proof.
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

Private program-extent text uses the explicit `ValueText` representation and the common
pointer/byte-length view geometry. Both static encoders share descriptor byte encoding. The
private catalog interns only actually serialized character backing by content and retains explicit
data-to-data relocations, including nested text fields and embedded zeros. Empty text receives
minimal physical backing; distinct static descriptor objects retain their own identities. Text
backing counts against the same serialized-byte budget before allocation.

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

## Private Runtime Error Construction

Checked primitive edges retain their roles through MIR. The private consumer plans their stored
builtin inputs and result through the common runtime-call ABI planner, interns equal physical
ABIs and retains ordinary `MachinePrimitiveTarget` entries. It creates no synthetic source body,
local or callable instance. A body read/edit channel receives an address; the qualified error
constructors receive the text descriptor stored at that address. Loading it into the runtime's
ABI grants no source Copy operation. The shared role contract remains the signature owner;
the existing context planner, ARM64 primitive selector and graph release implementation remain
the native consumers.

`owned-error-construction.nct` executes selected `NewError` and `ErrorContext` calls with real owned
graphs. It covers direct release, partial record and constant-index array cleanup, restoration,
zero/one-count seed cleanup, owning-capture disposal and consumption through a restricted-view
adapter, primitive callback witnesses, explicit failure, propagation and owned
named/discarded failure recovery. The native test verifies that constructor calls and graph-release
operations survive Machine lowering, then executes the ordinary image. This does not qualify
arbitrary text backing, error accessors, required-evaluator error graphs, user destructors or
all retained-index cleanup cases.

## Private Optional Execution

The private consumer uses the common optional/fallible layouts, tag encoding and payload offsets. Stored tag
access requests the runtime primitive's own layout, including helper-only primitive types. Frozen
variants write only the selected payload; absence never reads uninitialized payload backing.
Assertions branch to the ordinary non-unwinding safety trap before extraction. Propagation uses
the same stored-tag condition with ordinary branch/return blocks, caller cancellation and result
transport. Complete cleanup
checks the tag before calling the selected primary/alternate payload helper, while extracted
payloads retain the checked partial-container schedule. Optional absence and fallible success
qualify skipping inactive error cleanup; they do not establish source construction or release
of an owned error graph. Native source qualifies success construction/propagation and scoped
assertion through the same result transport, with separate absence/bounds traps before an
inactive fallible payload can be read.
Recovery success paths also qualify lazy handlers, preserved original-storage update and ordered
optional/fallible recovery. Alternate access guards use the requested tag through the same
non-unwinding branch protocol. Actual owned failure recovery uses the
[runtime construction](#private-runtime-error-construction) path.

Layout and cleanup dependency discovery use iterative work over the same actual schema requests,
retaining positive-cycle rejection and empty-array/pointer cuts. Flat optional suffix depth does
not require recursive host calls. Shared source qualifies required/frozen/native handoffs; the
checking component owns source admission and the remaining outcome operations.

## Private Array Execution

The private source path qualifies listed/repeated arrays, complete read/edit selections and
replacement through ordinary Machine bodies and the native consumer. Storage projects one shared
`RuntimeType::FixedArray` element/count descriptor. The common layout and value-input ABI own
stride, geometry and direct/indirect transport. Resolving a logical `ValueField::Array` computes one
checked offset from that descriptor, without an element-sized layout map.

`executable::body::arrays` materializes each evaluated index into a saved checked address. It uses
the common Machine index step with the transported checking disposition, then stores the pointer
for an access result or snapshots the selected Copy payload. Later uses and assignments consume
that saved pointer; they do not
reload the user's index variable or repeat the source bounds decision. Zero-byte selections still
materialize their bounds effect whenever checking requires it. Native instruction tests distinguish
removed proven checks from retained required checks even after scalar constant propagation.
Existing lifecycle phases retain RHS-first replacement, interruption cleanup, calls and independent
Copy snapshots. A private no-result `BoundsTrap` becomes a terminal `MachineTrap::Bounds`
after the preceding operand effects, without constructing element geometry or a selected pointer.
The target owns the numeric trap encoding; general MIR failure retains its distinct cause.

`executable::body::repetition` loads the established seed once and emits a counted Machine CFG.
Its countdown counter is an ordinary SSA block parameter. Fill order has no user callback; the
established Copy seed is loaded before traversal. One homogeneous layout proves the compiler's byte
offset and successor arithmetic; internal traversal does not manufacture source bounds proofs.
The shared traversal accepts SSA lower/upper endpoints and skips empty runtime intervals. A
saved cleanup span loads its dedicated usize snapshot or prefix-endpoint slot; its checked original bound proves
the successor and it does not reload the source index. The [source owner](../nocter-checking/README.md#array-selection) defines retained-hole admission;
actual nontrivial effects use the bounded [owned-error qualification](#owned-error-effects).
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
it does not establish execution of an owned error graph. The separate
[runtime construction](#private-runtime-error-construction) corpus executes constant-index partial
cleanup and complete-array release; retained-index cases use the separate
[owned-error qualification](#owned-error-effects).

Partial-array cleanup consumes private MIR's checked half-open intervals. The same countdown CFG
starts at the interval end and stops at its lower bound, addressing the original array with each
original index. It calls the existing element helper once per surviving element in reverse order;
trivial elements need no physical cleanup. Generated structure stays finite for three and 100,000
elements around either a source-proven constant hole or a retained index. The required original
bounds check survives before capture; helper endpoints read the captured scalar. Retained element
projections load that same slot at each use; their addresses are not cached across alternative
blocks, where the first load would fail to dominate another route. This nontrivial helper evidence
remains image/structure qualification for large cardinalities beyond the separately executed
owned-error cases.

For multiple unordered retained field holes, `body::remainder` traverses elements and components
in reverse order and guards only the missing subtrees with predicates over the saved indices.
Equal indices exclude all corresponding missing fields. Intact subtrees use their existing cleanup
helper; trivial data needs no traversal. Generated structure depends on schemas and missing paths,
not array cardinality, and no runtime ownership table or element-sized metadata is added.
Cleanup-helper registration exhaustively covers complete, interval and sparse-remainder operations.
Nontrivial error-field cases qualify generated predicates, helpers and finite image structure;
actual effects for the qualified mixed retained-field paths execute in the corpus below.
Mixed prefix/point cleanup retains prefix exclusions as regions: an element is excluded while its
index is below the saved frontier's successor. That shared successor calculation also serves
ordinary through/after spans and remains valid at maximum usize. Source cases execute zero-trip,
partial and complete prefixes, equal-valued point/frontier backing and maximum-count zero-byte
containers. Nontrivial mixed-field images keep the nested and containing array bounds distinct.

The shared retained-source corpus executes immediate and reserved take/read/edit handoffs, later
index mutation, ownership alternatives, equal-number snapshots on different routes, containing
replacement across backedges, concrete nested parents and maximum-count zero-byte moves. Dynamic
out-of-range transfer still traps before capture, including unused zero-byte payloads. The corpus
catalog is shared with required evaluation; no additional compiler array ownership map is used.
It also executes proven unit-stride prefix loops without containing replacement. Element delivery
uses the frozen evaluated alias before replacing the saved endpoint, and remainder cleanup reads
that endpoint rather than the user's counter. Early exits include a cancelled tail-element
reservation before the counter update. Normal prefix argument handoffs use the existing prepared
owner before entering the ordinary taking or read/edit temporary channel, including a later
argument's mutation of the original counter. Concrete nested parents and maximum-count zero-byte prefixes
use the same lowering. These executions qualify lifecycle/geometry with trivial element cleanup;
actual owned error-graph effects use the separate qualification below.
Ordered bundles execute distinct nonzero payloads through separate argument owners before reusing
one endpoint slot. Cases also cover a later counter mutation, cancellation after two reservations
and maximum-count zero-byte bundles; endpoint reuse does not retarget a frozen argument address.
The corpus also executes a taking call on each prefix-loop iteration through mutually recursive
callees whose caller-write effects are closed by checking. No new native effect interpretation or
source annotation is introduced.
Selective-call cases execute a transitive update to a disjoint record field beside the partial
array, plus an edit of each transferred element temporary. The original array's endpoint and
remainder remain correct while the ordinary calls perform their actual writes.
Alias-call cases perform those disjoint updates through chained lexical edit aliases and also
update private helper-local storage. They consume checking's input-root summaries without a
native alias or ownership registry.
Caller-side alias cases also preserve disjoint prefix loops and the original element address
after index mutation. Their checking-owned may-write footprints can be wider than that address;
invocation continues to consume the existing frozen alias.
Dynamic partial-element cases execute record-field and constant inner-array extraction, index
mutation, conditional holes, containing replacement and cancelled call preparation. Small and
maximum-count zero-byte arrays use the same saved-index projections and sparse cleanup path.
Nested dynamic cases also execute two independently saved indices, outer-variable mutation during
inner-index evaluation, and maximum-count zero-byte arrays in both dimensions. Selected payloads
have distinct nonzero values so incorrectly recapturing an outer index changes the answer.
Flow-proved constant cases execute disjoint reads and transfers, frozen aliases/arguments,
restoration and a two-element call bundle after mutation of the user's index. Checking supplies
constant resource paths without saved-index owners; native lowering retains the original bounds
operations and the selected evaluated addresses.
Retained-point restoration uses the existing saved-index projection for its destination. The
replacement is installed before that snapshot's retirement. Shared source executes user-index
mutation, alternating indices across restored backedges, direct call retirement, both restoration
branches and maximum-count zero-byte arrays. These cases introduce no new ownership table or
native operation kind, and retain every original bounds operation.
The restored-epoch case then consumes the complete array in a new prefix loop. Its earlier point
snapshot ends before prefix backing is created; a shared structural representative never supplies
the new endpoint's number.
Conditional-restoration cases execute both complete and missing-point alternatives, including
different saved-index slots holding equal numbers. Existing ownership guards select a live
snapshot path or initialized referent replacement; no index comparison chooses a lifetime or
reads backing from an inactive route. Maximum-count zero-byte cases use the same transitions.
Initialized-element replacement and compound update also precede a later prefix loop in shared
source execution. Conditional restoration reaches that loop from both the complete and missing-point
routes; changed payloads make an omitted replacement observable.
Shared scalar-alias cases execute prefix loops through an outer edit-alias chain and an alias
recreated within each iteration. A read alias supplies a proved constant selection. These use ordinary
alias addresses and existing scalar/frontier storage, without alias-owned numerical backing.
A separate native fixture executes a prefix loop through a helper that constructs and destroys
an exact owned closure with a non-Copy array environment. The dormant body would trap if called;
cleanup releases the environment without invoking it. This is native evidence; the required
evaluator's closure construction/invocation remains outside its qualified domain.

Shared source fixtures execute element take, field holes, structurally disjoint unknown-index edits,
exact restoration on loop backedges and maximum-count zero-byte remainder cleanup. The checking
owner defines their admission and remaining frontier/progression limits. These limits do not change adopted source semantics or qualify the ordinary
released CLI as v2.

### Owned Error Effects

`owned-error-retained-arrays.nct` extends the ordinary native path with actual owned error graphs
inside non-Copy elements. It executes prefix take and read-temporary handoffs, tail-argument
cancellation, equal/different saved indices for sibling field holes, nested dynamic holes and
outer-index mutation during inner-index evaluation. Mixed prefix/field remainders execute zero,
partial and complete prefixes with the field hole in either containing row. Error graph release
uses the existing schema helpers, original saved indices and cleanup schedule; no second owner
table or native effect interpretation is introduced.

Conditional replacement executes both initialized and missing-point routes, including mutation of
the original index before restoration through its saved equivalent. Further cases restore the
same owned element across alternating loop indices and both restoration branches, replace the
containing array on each backedge, and start a fresh prefix after conditional restoration. A
replacement producer may construct an error snapshot without destroying the caller's saved-index
facts; checking supplies the separately qualified caller-write evidence. A wrapper's actual index
write still rejects an unsafe restoration.
Fallible replacement producers execute both success and failure with owned record payloads.
Failure propagation precedes destination receipt and releases the remaining partial array and
the previously extracted owner; outer recovery releases the propagated error graph. The common
ABI carries both the indirect record outcome and direct integer outcome without reading inactive
payload backing.

Projected-field restoration also executes with owned error graphs. The source mutates the user's
index after extracting one record field, restores through its saved equal index, then transfers
the completed array. Single and repeated alternating-index routes release the extracted old
graphs and the final fields through the existing cleanup schedule, without treating a child
write as whole-element initialization. Replacement of the other field between extraction and
restoration also executes its owned-graph cleanup without losing the original point's backing.
Conditional field restoration executes both missing and initialized routes, using a saved equal
index on the missing route and a different selected number on the initialized route. The latter
replaces its live old graph without reading an absent snapshot; both routes then transfer and
clean up the completed array.

Fallible projected replacements execute success and failure with an optional extracted owner
kept outside the conditional branch. Failure cleans the active extracted graph and the partial
array before delivering its error; absence contributes no fictitious payload. Success and catch
values differ, so a skipped failure cannot satisfy the recovery check. A target index that returns
early also qualifies RHS-first ordering and disposal of a newly prepared replacement that is
never installed. An independent checking-ledger assertion requires the complete RHS owning
replacement to be disposed exactly once; native execution exercises the contained allocated graphs.

`owned-error-adapters.nct` executes read environments forwarded through formal consuming adapters,
read-to-update-to-once views, repeated environment replacement followed by consuming completion,
and consuming bodies with partial record/retained-array cleanup. A fallible read body also returns
a separate error graph: adapter cleanup releases its environment before the caller handles that
failure. The original body invocation contract determines whether cleanup belongs to the body or
the adapter; weakening a callable view creates no second owner.

These are bounded executed effects. Outside-dependent captures, general user destructors
and required-evaluator error graphs remain separate domains. Large-cardinality helper/image tests
do not imply execution of correspondingly large owned graphs.
