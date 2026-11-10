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

`computation` consumes selected operation kinds and frozen call edges; it does not infer Copy,
access, result witnesses or dispatch. Private required constants arrive as ordinary primitive
literals or model-owned frozen literal DAGs; this product has no unresolved declaration-reference
operation or evaluation service. Frozen records initialize one ordinary owning result through
its existing lifecycle phases. Validation checks component shapes against the exact runtime
schema, memoizing shared type/node pairs; it does not grant Copy or infer another constructor.
Selected unit-comparison intrinsics lower only their final computation to a Boolean constant;
their operand operations remain in the consumed graph. A zero-byte operand is not interpreted as
an integer comparison. `validate` checks closed representation, graph, signature,
projection, primitive-operation and flag integrity before publication. It does not repeat source
ownership analysis. The common Machine consumer lowers these closed bodies without a released
MirProgram conversion.

The [private activation handoff](../../../architecture/pipeline/value-access-declarations.md#private-activation-storage-handoff)
owns cross-stage ordering and remaining execution obligations.

Each body additionally freezes the reachable static address-slot bindings separately from its
ordered inputs. One program-owned static table retains their exact runtime data and frozen values.
Validation proves table coverage, global shapes, unique alias slots and absence from the input
layout. An immutable global is neither a parameter nor a body-owned resource; no initialization
call or destruction is inferred. Consumers receive those closed bindings without examining
source names or deriving a static role from a slot's address representation.
