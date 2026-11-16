# nocter-mir

## Responsibility

Lower one closed executable program into concrete target-independent semantic control flow.

## Contract

MIR consumes monomorphized items, selected operations, concrete representations, cleanup plans, and
runtime roles, and closed target-service descriptors. It publishes validated functions, immutable
static identities and frozen values, places, values, blocks, operations, packs, primitive
dependencies, and foreign-call plans. It does not inspect syntax, resolve names, prove requirements,
or assign a machine ABI.

## Internal Responsibilities

- CFG and dense local identity construction
- concrete place and projection lowering
- lossless transport of checking-owned safety dispositions
- canonical physical aggregate assembly after source-ordered initializer evaluation
- explicit cleanup, destruction, region, outcome, and switch edges
- deferred execution, suspension edges, continuation liveness, and checked cancellation plans
- the compiler-owned process operation that drives a selected deferred entry without changing
  ordinary call semantics
- operation and pack schemas
- whole-program MIR validation
- complete bounded private bodies under `value_access`, including control, computation, calls,
  activation requirements and lifecycle phases, separate from released executable admission

## Invariants

- Each block has one exact terminator and typed merge contract.
- Struct initializer evaluation order remains checked-program order, while the finished MIR
  aggregate is ordered once by the executable runtime representation.
- Cleanup timing comes from checked plans, not operation-shape inference.
- Deferred cleanup distinguishes runtime-owned opaque storage from source aggregates. It retains
  the selected storage role and drop item without inventing fields for target-owned bytes.
- Calls target concrete executable item identities, closed primitive roles, or already validated
  target-service descriptors.
- Declared constants are resolved only through values frozen by executable reachability; MIR never
  reads declaration records or repeats constant evaluation.
- Validation checks representation integrity, not source-language acceptance.
- Dynamic indexes retain the exact bounds-check disposition selected by checking. MIR neither
  derives a proof from constants nor changes a required check into a proven result.
- Source arithmetic retains checking's exact trap disposition. Compiler-generated loop and pack
  arithmetic uses an explicit internal-invariant state instead of masquerading as source proof or
  silently selecting unchecked source semantics. A nonzero-divisor-only remainder disposition
  preserves the selected intrinsic's result domain; it is not a proof that division is safe.
- Suspension-frame liveness is derived once from the closed MIR CFG. Cancellation order and
  conditional initialization are consumed from checked ownership rather than inferred from MIR
  operation shapes.
- A fallible deferred output completes as one ordinary fallible value; MIR does not invent a
  second task-failure channel.

## Private Value Bodies

`value_access::Program` consumes one complete private lifecycle product and its exact instance
data/input authority. Its builder freezes the reachable control graph, interns dense slots, lowers
selected scalar operations and static calls, and integrates resource phases and committed call
bindings. The result contains its runtime environment and closed bodies, with no checking/target
storage reference. There is no separately pairable storage companion or released MirProgram adapter.
Slots, places, static data and value signatures retain the
[runtime storage identities](../nocter-runtime-contract/README.md#runtime-storage-identities).
During construction, signatures and payload slots use the same prepared contextual interner.
The finished product retains only its closed runtime environment and bodies.
Expression production and local binding select their frozen data at the original action. Operand
reads use the same occurrence authority, and whole-slot roots preserve the selected payload rather
than replacing it with a TypeId-wide default. Detached scalar/selector metadata must be independently
declaration-closed; a missing contextual occurrence rejects. No MIR lookup consults the legacy
frame-wide type map. This includes non-owning local bindings: their actual definition determines
the alias representation. Source-place address projection consumes checking's frozen reference-root
contracts at the original action, while partial input resource roots retain the bound input payload.
Contextual evaluated reference arguments likewise retain the original action's complete selected root.
Outcome tag inspection projects its selected data from that same original referent-root contract,
including nested callable contexts; it does not reconstruct the carrier from an unbound type.
A checked whole-value move into an original expression merge uses checking's shared receiving
contract. If its actual representation differs, lowering emits `Transport` directly from the
original supplied place into the common expression slot. Subsequent merged reads therefore consume
an encoded value, not a larger allocation containing bytes written in an older format. Original
source-step availability and ownership stay unchanged. A concrete view that cannot use that
receiving representation rejects rather than treating metadata as a physical conversion.
Bare callable domains retain a dedicated `CallableAlternatives` disposal marker from their
instantiated payload contract. The marker consumes the existing owner and delegates selected-case
cleanup to Machine; it does not turn a symbolic environment into a single captured value or grant
an invocation target.

Integrity also rejects a mixed slot's positive-domain injection when its old registered cases would
otherwise acquire a selector only through metadata. Local and loop-epoch conversion remains
unqualified; sufficient slot capacity and equal dimensions cannot establish an active tag.

Integrity validation rejects uninhabited stored-slot representations and positive places, while
outcome metadata can retain an excluded child. Constructing that child's primary tag rejects;
an existing tag assertion remains a guard rather than a payload-construction permission.
At invocation commit this root must match the prepared argument data; the previously evaluated
expression is not queried as a fresh operand or replaced by a declaration-wide type default.
Independently closed schemas remain sufficient for representation-only references, including
compiler-retained index references without a new source-value occurrence at a resource phase.
Matched scoped replacement uses its sealed before-root for retired-leaf cleanup and its after-root
for supplied-leaf placement, preserving the same resource path and cleanup order. An unpaired raw
scoped replacement rejects. It retains same-schema virtual and original containing root pairs as
`GeometryConstraint` metadata at the original action. Containing selections must match their slots
and paths on both sides. Constraints grant no resource phase or initialized value; integrity checks
reject absent action/data identities. The Machine layout authority proves their physical compatibility.
Dependent resource-source projections consume their original step's retained containing data;
independent roots keep their declaration-closed representation. Array remainder exclusions use
the same selected container root as the cleanup operation. Partial destinations without an exact
receipt still require independently closed or bound-input data. Matched owning partial moves use
their original after-container for placement, preserving the preceding separate disposal.
For direct owning assignments through tuple and singleton-array paths, `Effect::Repack` retains
the original before-container, value/prefix exclusions outside the replaced subtree and
all initialized disjoint siblings. Its shared structural projection requires unchanged sibling
data. A singleton array contributes its one established resource coordinate, never a guessed
first-element context. Plain targets require absence of retained addresses/reservations on both
sides of the action. Evaluated targets require absence on completion: the original selection and
bounds effects have finished, assignment emits no separate computation, and its receipt projects
the new root at the sealed numeric/resource coordinate rather than reusing the old staged pointer.
Both absence queries include all alternatives. Direct owning relocation still rejects surviving
views/reservations; the separately qualified scoped case below carries retained endpoint checks.
Each selected lifecycle resource step supplies its original destination's entry-state holes.
Different sibling holes keep distinct original guards through lifecycle simplification and MIR;
no downstream scan requires unrelated alternatives to agree. For the qualified assignment,
external holes remain unchanged between entry and receipt; holes inside the replaced subtree
stay covered by its original retirement.
The structural complement splits only ancestors of actual missing paths, leaving unaffected
shared schemas opaque; it never snapshots a hole or a partially initialized parent as a whole.
Unchanged record siblings use the same runtime aggregate fields, preserving canonical nominal field
identities rather than rebuilding a record map. Transferred values remain independently owned
until their original restoration; initialized siblings retain their own field identities even when
their data is identical. Required/native cases cover nested records and actual saved/remaining error graphs.
This broadens preservation, not the tuple/singleton destination qualification.
Complete optional/fallible siblings retain the original tag and each resolved alternative's
code/environment metadata while the active alternative remains runtime-selected. Required/native
cases cover presence/absence, successful callbacks and actual failed-result/capture error cleanup.
This does not resolve conflicting positive contexts or authorize an inactive payload projection.
Machine snapshots every preserved source before writing any new offsets and then performs the
original receipt. Nested tuples, environment growth/shrinkage, multiple holes, saved-instance
restoration, partially retired targets, exactly-once index effects, original bounds traps and owned
sibling cleanup have ordinary source/native coverage. Missing descendants can likewise traverse singleton arrays, preserving initialized
fields inside their one element without copying the missing element or a partially initialized
parent. Destination and singleton projections share one length check. Larger unchanged arrays
with concrete missing paths remain one `Preserved` subtree tagged `ArrayRemainder`, never an
expanded element list or a complete initialized load. `Complete` subtrees contain no missing
storage; validation reconstructs that distinction through the same structural projection and
rejects changing a sparse array to `Complete`. The missing paths remain the single exclusion
authority. Dynamic descendants retain their original `Projection::Index` snapshot slot through
the same resource-path converter as ordinary places. Structural projection selects only the array
element data; body validation independently checks each saved-number role, usize storage and exact
array bound. A local value of the same type or a snapshot from another bound cannot substitute.
Singleton coordinates keep their established constant selection. Machine snapshots and restores
only remaining fields through its shared sparse walk, without acquiring another owner or Copy
proof. Prefix exclusions use the same `RemainderExclusion` as cleanup, with a typed original
array place and its saved `ArrayFrontier` endpoint. Structural validation checks the old root,
array schema and disjoint destination; body validation requires the frontier role, usize storage
and exact containing bound. Cleanup and repacking share one exclusion validator before applying
their separate containing-source/receipt constraints. Sparse arrays stay one preservation part
even for a singleton prefix.
Original `MoveArrayElement` delivery consumes its ordered payload and retained selection address;
it cannot query a fresh contextual operand at a later resource phase. Required/native cases cover
empty, partial and complete prefixes, counter mutation, bounds traps and actual error cleanup.
Other partial moves keep the before/after geometry constraint. Unresolved represented alternatives,
external carriers and general relocation remain separate qualifications. The [checking owner](../nocter-checking/README.md#context-bound-callable-captures)
defines the qualified ordinary-source capture scope.
Scoped replacement through a whole edit alias can repack a single corresponding owned local
carrier when its original lifecycle retains identical owning exclusions at entry, the operation
boundary and normal exit. Both virtual-root leaves must exactly match that carrier's original before/after
selection. Tuple and singleton-array containing paths reuse the structural preservation authority;
no missing sibling is assumed initialized. Scoped and direct receipts share the same missing-path
conversion and structural complement; sparse arrays remain remainder preservation, never whole
initialized loads. One payload slot registers both complete carrier
representations, preserving its base and reserving their maximum size/alignment. The old alias
value is disposed first; the existing Repack effect snapshots initialized siblings before writing
new offsets and receiving the separately supplied value.

The paired GeometryRequirement::ProjectionOffsets retains every original before/after access
region for that owner, including reservations and known/unknown coordinates. Machine validates
all endpoint offsets independently of the full-layout equality used by ordinary scoped writes
and adapter exits. A live sibling whose offset changes still rejects; an expired alias does not
create a constraint. Missing region evidence, changing ownership exclusions, external inputs and
unsupported containing paths retain the full-geometry boundary. Closed MIR validation requires nonempty valid
paths and a matching Repack at the same original action. The shared
`contextual-scoped-growth.nct` source qualifies repeated shrinking/growth, nested tuples and a
singleton array through required/native execution. It also covers saved whole siblings, nested
holes and a larger array containing a partial tuple element. Its error variant retires real old
captures and preserves/cleans actual saved and remaining owned environments. Exclusion and
preservation-kind mutations reject, and exhausted metadata work permits a healthy retry. This
does not qualify changing sparse states, general external aliases or changing retained endpoint
offsets.

Adapter cleanup likewise selects its completed full carrier in the original argument slot before
projecting the completed environment. Its selected environment data must match the cleanup
payload, and its entry/completed carrier pair becomes an original-call geometry constraint.
The original input/ABI contract stays paired with call entry; adding the exit representation
grants no initialized storage, lifetime or relocation proof.
Component integrity and sparse cleanup paths use the closed runtime environment's shared type
projection, independently of source checking and without a reverse semantic-type lookup.
Place, copy/transfer, call-boundary and return validation consume the runtime contract's shared
value-representation comparison. Empty arrays have no positive element; their source descriptors
remain intact. All positive leaf identities, including static callable tokens, must still agree.
This check neither uses byte geometry as a type proof nor permits conversion between distinct
nominal records. Machine consumes the same authority for selected place metadata.
`Effect::Transport` retains separate source/destination places and validates structural domain
injection, including guarded exclusions and complete positive callable cases. The runtime contract
owns that directed relation separately from equality and guarded-only comparison. Transport still
requires payload storage in different root slots and an unprojected destination; domain membership
supplies no alias stability, initialized backing, overlapping or in-place reconstruction permission.
Exact `Transfer` equality remains unchanged. Source-backed integrity mutations reject replacing
transport with copying, reversing its direction or aliasing the roots even when both representations
are registered. Machine selects each side's own layout. Checking's original source gate remains
separate from this closed runtime validation.

Static primitive edges retain their closed roles without adding a body or input locals. Validation
checks their data and input operations against the runtime contract's sole value signature;
unsupported roles and substituted signatures reject. Only body edges participate in body-graph
reachability. The Machine consumer owns their physical ABI qualification.

`computation` consumes selected operation kinds and frozen call edges; it does not infer Copy,
access, result witnesses or dispatch. `SelectedCall` retains a finite callable carrier, its ordered
complete runtime cases, the corresponding body instances, common explicit inputs and one answer
slot. It admits only the target graph's retained-read body cases. Validation checks complete
canonical coverage, each body's read environment and explicit/result signature, the original
argument ordinals and answer identity. Candidate environments have no general `Place` projection;
they are available only inside selected invocation. Every candidate contributes a body reachability
edge. An ordinary `Call` still rejects the unresolved `CallTarget::Selected` marker.
Private required constants arrive as ordinary primitive
literals or model-owned frozen literal DAGs; this product has no unresolved declaration-reference
operation or evaluation service. Frozen aggregates initialize one ordinary owning result through
its existing lifecycle phases. Validation checks component shapes against the exact runtime
schema, memoizing shared type/node pairs; record-field keys and tuple positions retain their
separate meaning even at equal offsets; it does not grant Copy or infer another constructor.
Selected unit-comparison intrinsics lower only their final computation to a Boolean constant;
their operand operations remain in the consumed graph. A zero-byte operand is not interpreted as
an integer comparison. Integer constants retain mathematical values; validation checks their
exact integer domain against the selected data identity before any physical encoding. `validate` checks closed representation, graph, signature,
projection, primitive-operation and flag integrity before publication. It does not repeat source
ownership analysis. The common Machine consumer lowers these closed bodies without a released
MirProgram conversion.

Outcome assembly freezes payload transfers followed by a `VariantTag` effect and initialization
in the same lifecycle phase, including wrappers materialized at call commit. `RequireVariant` is
a separate guard for the exact primary/alternate tag before selecting the payload; it supplies
no ownership or unwind edge.
`PrimaryCondition` produces a Boolean from a stored carrier, leaving propagation's branches,
returns and cleanup in the same ordinary closed graph. Scoped assertions place a normal address
binding after the successful guard; their payload selection never copies the carrier or acquires
another owner. The checking-owned access world retains tag stability, not this physical product.
Validation checks these operations against the exact runtime optional/fallible shape. Assembly
derives the tag from the selected variant key, including when both fallible payload types are error.
Frozen variants retain their selected branch rather than masquerading as ordinary aggregates;
this path does not admit frozen owned-error graphs.

Evaluated primitive array selection freezes one `Index` operation with its base, numeric index,
selected data, established access-versus-Copy result role and checking's exact bounds disposition.
The shared role is reexported from Model; no conversion or range classifier reconstructs it here.
Validation checks the homogeneous runtime shape, usize index, element identity and destination
storage form; it does not grant Copy.
Concrete assembly fields resolve against that same element/count descriptor. Frozen listed and
repeated arrays validate their exact count and actual value edges without count expansion.
Runtime repetition freezes one schema-counted seed fill above count one; the ordinary resource
assembly handles count one. Validation checks element identity and owning result storage, without
inferring Copy or expanding the count. A result-producing `Index` rejects `ProvenTrap`.
Guaranteed failure instead freezes a separate `BoundsTrap` with no result slot, element address
or payload requirement. Validation requires it to be the sole operation of the final action before
`Unreachable`, without lifecycle effects, call bindings or flag writes; no consumer may execute
cleanup after that non-unwinding failure. The [native consumer](../nocter-machine/README.md#private-array-execution) owns executable scope.

The [private activation handoff](../../../architecture/pipeline/value-access-declarations.md#private-activation-storage-handoff)
owns cross-stage ordering and remaining execution obligations.
Sparse array cleanup freezes `DisposeArrayElements` with its containing array place and an
`ArrayRange`: a half-open constant u64 interval, or the portion before/through/after a saved index slot.
Constant validation requires `start < end <= length`. Saved spans require a payload usize
`SlotKey::IndexSnapshot` or `SlotKey::ArrayFrontier` whose original checked bound equals the positive array length. Slot role
and representation mutants test this structural consumer contract; they do not prove snapshot
initialization or executed native cleanup. [Source selection](../nocter-checking/README.md#array-selection)
qualifies admission. `Projection::Index` requires the immutable snapshot role; an advancing frontier
is not an element address. Prefix delivery uses an ordinary transfer from the actual evaluated alias.
`ConsolidateArrayPrefix` retains its array and frontier roles as validated lifecycle metadata; native
execution already implements the transfer, saved scalar and derived guard updates. No span invents a subarray type or
expands element projections. Checking owns the surviving
intervals and their order. The native consumer selects element geometry/helpers and qualifies
executable scope; MIR does not infer source holes from byte offsets.

`DisposeArrayRemainder` instead carries an owned array place and the existing sparse missing paths
when retained indices have no proved numeric order. It ends the container after cleanup.
Validation requires unique descendant selections under the same owning slot/root and validates
each saved-index projection against its original bound. This freezes exclusions rather than
reconstructing ownership, equating descriptor keys or granting source access. Malformed duplicate
and ancestor exclusions are integrity failures.
`RemainderExclusion` preserves either a selected value place or an array-prefix base plus its
frontier slot. Prefix endpoints require the frontier role and the prefix array's own positive
bound; they do not become element projections or inherit the containing array's length. Role and
bound mutations qualify this distinction.

Each body additionally freezes the reachable static address-slot bindings separately from its
ordered inputs. One program-owned static table retains their exact runtime data and frozen values.
Validation proves table coverage, global shapes, unique alias slots and absence from the input
layout. An immutable global is neither a parameter nor a body-owned resource; no initialization
call or destruction is inferred. Consumers receive those closed bindings without examining
source names or deriving a static role from a slot's address representation.
