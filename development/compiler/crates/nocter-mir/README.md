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
receipt still require independently closed or bound-input data. Changing geometry,
relocation and general carrier coherence remain separate qualification boundaries for changing
captured representations. The [checking owner](../nocter-checking/README.md#context-bound-callable-captures)
defines the qualified ordinary-source capture scope.
Adapter cleanup likewise selects its completed full carrier in the original argument slot before
projecting the completed environment. Its selected environment data must match the cleanup
payload, and its entry/completed carrier pair becomes an original-call geometry constraint.
The original input/ABI contract stays paired with call entry; adding the exit representation
grants no initialized storage, lifetime or relocation proof.
Component integrity and sparse cleanup paths use the closed runtime environment's shared type
projection, independently of source checking and without a reverse semantic-type lookup.
Static primitive edges retain their closed roles without adding a body or input locals. Validation
checks their data and input operations against the runtime contract's sole value signature;
unsupported roles and substituted signatures reject. Only body edges participate in body-graph
reachability. The Machine consumer owns their physical ABI qualification.

`computation` consumes selected operation kinds and frozen call edges; it does not infer Copy,
access, result witnesses or dispatch. Private required constants arrive as ordinary primitive
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
