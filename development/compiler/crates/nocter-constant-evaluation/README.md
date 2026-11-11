# nocter-constant-evaluation

## Responsibility

Own closed, syntax-independent plans and deterministic evaluation for compile-time expressions and
checked callable bodies.

## Contract

The crate consumes syntax-owned structural constant expressions plus explicit semantic support and
produces typed scalar plans; it also owns recursively frozen tuple and fixed-array values,
checked-body callable recipes, closed initializer/callable plans, and semantic evaluation failures.
Recipes retain checked body identities,
store-relative types, and already-selected call targets; closed plans replace only those type edges
with evaluator-domain shapes. This crate never consumes a checked body directly. It does not
perform body checking, runtime execution, name lookup, overload selection, or target code
generation.

Integer token decoding delegates to the syntax-owned complete-token magnitude decoder. Evaluation
still owns contextual numeric type, sign, range, and checked arithmetic.

## Invariants

- Evaluation order and supported operations follow the public constant contract.
- The evaluator receives resolved inputs instead of repeating declaration lookup.
- Decimal floating evaluation receives syntax-owned suffix decomposition and a selected format;
  it never parses a suffix or asks the compiler host for a floating-point value.
- Failure cannot publish a partially evaluated semantic constant.
- Callable recipe and plan construction validate every node, parameter, and local edge before
  publication. A closed plan retains the exact type of every parameter and its result, so execution
  does not rely on its caller to pair values with an external signature.
- Closing a plan table validates each call edge's target, receiver/argument arity, input shapes,
  and result shape against the target plan. An executor can never observe a partially linked plan
  graph.
- A callable plan contains no syntax or source coordinate and cannot request a semantic decision.
- One generic expression-tree operation representation serves both checked recipes and closed plans. Its exhaustive
  call-target mapping is the only recipe-to-plan operation transform, so specialization cannot
  grow a second partial operation model.
- Callable specialization keys contain closed evaluator-domain type shapes rather than `TypeId`, so
  a plan identity cannot be paired with a sibling checked-program type store.
- Generic recipe projection validates eligible bodies before reachable specialization, including
  uninvoked declarations. One query closes each reachable plan under an exact argument set;
  recursive source calls remain graph edges rather than plan-construction cycles.
- Evaluation limits count semantic plan operations and source-call depth, never host instructions
  or elapsed time; all callers use the same nonzero limit contract.
- The expression-tree executor owns its remaining budget and memoized call results. It validates input,
  local, node, and result shapes; recursive calls therefore cannot bypass the depth limit or make
  malformed values observable.
- Every nested operation produces either a typed value or one function-level control interruption.
  `return` and unreachable control therefore propagate uniformly through discarded expressions,
  aggregate elements, operands, arguments, and nested blocks instead of relying on each parent
  operation to remember a special case.
- The executor's typed result performs the only conversion into scalar declaration constants or
  recursively frozen tuple/array statics; adapters cannot flatten aggregate structure themselves.
- Constant expressions and checked callable plans delegate arithmetic, comparison, conversion, and
  target floating behavior to one scalar operation implementation.
- Expression-tree constant plans freeze reference edges once during planning. Evaluation requests those edges
  through the query and does not construct a separate dependency-order traversal.
- The syntax-owned expression planner is an early structural service only. Ordinary constant and
  static initializers reach execution through checked initializer plans, so adding calls does not
  expand the syntax planner into a second body checker.

## Stateful Value Plans

`state` owns the closed graph-plan domain for the private value-access source path. Checking projects
its selected operations and exact resource alternatives into this product; this crate never reads
its typed bodies or source. `Program::new` validates finite type/field schemas, slot roles, control,
operation shapes, exact call signatures and resource references. Equal owning-state guards cannot
select different resource phases. Initialization thunks have no inputs and cannot be ordinary call
targets. Positional type identities retain the producer's nominal distinctions within one immutable
table; structurally similar records do not make call edges interchangeable.

Plan tables, execution sites and failure roots retain a complete `ValueCodeRoot`. Different
required expressions can share a source owner without sharing an executable plan; the parent's
plan need not exist to execute an expression. Expression roots are input-free initializer plans,
never ordinary call targets. Direct calls continue to select declaration code explicitly. The
validated plan supplies the result type; the evaluator cannot infer usize or any other data
contract from a code key. Work/storage/depth accounting and failure call paths use the same
executor for declaration and expression roots. This closed-plan support does not itself admit
body-local count source through checking's dependency query.

`Type::builtin` owns the admitted primitive domain for the current 64-bit profile. The checker uses that same admission
for body-domain checks. Ordered aggregate schemas compose those leaves without claiming Copy. Checking retains source
record/tuple type distinctions while using that one evaluator shape. Schema
projection and value validation memoize shared subgraphs rather than expanding trees. Boolean/i32/usize
arithmetic delegates to the common scalar operations without a target-dependent fallback. The
selected integer domain preserves the distinction between checked-quotient remainder in the
published contract and direct remainder in the [adopted contract](../../../spec/language/integers.md);
only the quotient-domain policy differs, not the arithmetic implementation. Floating
operations still require an explicit compilation target in their existing domain.

A selected comparison with a fixed result arrives as a literal operation after the original
operand actions. This does not authorize skipping calls, access validation or failure before that
operation, and the executor performs no source-type dispatch to derive the result again.

The executor keeps fresh invocation frames, canonical scoped addresses, independent scalar
snapshots and sparse owning states. Immutable aggregate data may share internal nodes; writes copy only
the affected path, preserving independent value semantics and the identity of aliased places.
Frozen before/after effects transport ownership, assemble aggregates and perform actual sparse cleanup.
Taking inputs are protected before retirement; read/edit inputs bind addresses after retained
caller temporaries exist. Ordinary calls execute every time, including unit-returning editing
calls. The executor performs only closed execution; checking owns required value requests and
caches completed values through the common
[construction dependency query](../nocter-dependency-query/README.md). Unresolved declaration references
cannot enter a stateful plan. Frozen source dependencies, including unevaluated authored references,
are resolved before a plan executes. Callable recursion remains ordinary graph execution.

`state::Budget` owns one remaining-work counter across dependency preparation and nested closed
executions. Actions, resource effects and control transitions consume it without resetting for a
callee. The dependency query exposes its own active depth, rather than asking checking to maintain
a second stack count; the executor combines suspended depth with invocation frames. Arithmetic,
work, live-storage and call-depth failures retain code/action identities and the call path. A failed
execution clears its frames and cannot return a partly initialized value. No ordinary call or
initializer cache is maintained by this executor.
The executor does not prove source access safety, choose another operation, perform final source
publication or serialize a runtime object. Checking's
[initializer boundary](../nocter-checking/README.md#required-private-initializers) owns source
admission, exact-generation reuse and the currently supported final-value scope.

### Evaluated Array Addresses

The closed stateful domain retains fixed arrays as homogeneous element-type/target-u64-length
metadata, separately from ordered record/tuple schemas. Schema validation does not expand a count.
Positive containment edges follow the finite table order; a zero-length edge requires a valid
element type but contributes no payload containment. Nominal/structural identity remains the
producer's type-table identity, never a physical value's shape.

`Type::Unavailable` names declared data for which this closed table provides no payload representation.
Every literal value rejects against it. A zero-length array can retain that element identity
without constructing it, while a positive element cannot smuggle unit/scalar/aggregate data into
that shape. The descriptor creates no source type or runtime representation. Checking derives
its payload-domain decision from the same projected table and links zero-element identities
without introducing positive containment cycles.

Listed arrays use the same `Data::Aggregate` nodes, metered import, copy-on-write, assembly and
frozen publication as records/tuples. The selected slot schema validates their exact count and
homogeneous element data. This physical sharing does not turn an array into a tuple or an empty
array into void. `Operation::Repeat` uses the result schema's element/count and requires a data
result with count greater than one; checking supplies the source-context Copy proof. Zero/one
source repetition instead uses ordinary empty/single-element assembly and seed cleanup.

`Data::Repeated` retains a positive u64 count, one default element and ordered sparse replacements.
Reading an untouched position shares the default. Writing a position creates or replaces one
entry, with copy-on-write along the selected path; it never expands the count or accumulates an
update history for that position. Listed and repeated arrays preserve the same indexed access,
snapshot and typed-value behavior. Value equality compares logical components using existing
listed entries or the union of sparse positions, without iterating an implicit repeated range.
The default need not compare when all positions are replaced. This is internal value equality,
not new source comparison syntax or Copy evidence.

`Operation::Index` requires an array base, a usize index and an alias result of the element type.
`Operation::IndexCopy` instead snapshots the selected payload into a data result; its producer
owns the Copy proof. Both use the same numeric address/bounds implementation.
Execution resolves the canonical base, reads the evaluated index once, checks it against the
schema length, then appends the numeric index to the existing metered address path. It never
retains the mutable index slot as the address recipe or reads the whole array to form an address.
Later index mutation, a repeated operation on a backedge and a nested call cannot retarget an
older address. New selections still read their own current evaluated index. Bounds failure
reports `IndexOutOfBounds` at that operation and publishes no selected address or result. Failed
frames release their storage through the same executor recovery as arithmetic/budget failures.

`Operation::BoundsTrap` represents a producer-proven bounds failure after ordinary operand
evaluation. It reports the same `IndexOutOfBounds` at its own site without a result slot, selected
address or element payload representation. Executor storage recovery does not synthesize source
cleanup or a normal continuation after that non-unwinding failure.

Closed-plan tests qualify listed construction/publication, array fields, independent snapshots,
selected edits, backedge re-execution, cross-frame addresses, exact bounds and failed-budget
cleanup and indexed Copy snapshots with distinct data/alias result roles. Repetition tests also
cover maximum-count selected edits/snapshots, sparse replacement, malformed result roles/counts,
logical equality, deep mixed DAGs and storage-failure recovery. Checking separately
qualifies [source listed construction and whole-value handoffs](../nocter-checking/README.md#array-construction)
and [source read selection](../nocter-checking/README.md#array-selection).
This domain alone proves neither precise source element conflict/initialization safety,
dynamic resource-hole cleanup nor native array storage. The remaining integration follows the
[evaluated-selection obligations](../../../design/compiler-realization.md#evaluated-array-selection-closure).

`Effect::DisposeArrayElements` retires an intact half-open interval under one owning slot. Type
validation requires an owned array place and `start < end <= length`. `memory::ownership` keeps
individual missing paths and cleanup intervals in one metered sparse frontier, rejecting overlap
and retirement of unavailable parents. Container release collapses its retired descendants without
visiting elements. Intervals are cleanup state, not restorable per-element initialization or a new
input ownership condition. Repeated failed execution releases their leases. Maximum-count and
small repeated arrays have the same minimum live-storage budget for interval cleanup; nested
partial elements and intact intervals share that same owner.

### Live Working Storage

`state::Limits` adds a deterministic storage limit without giving the expression-tree executor an
unenforced setting. `state::Budget` owns the one work counter and one shared storage meter. An
outer request restarts work only. All nested executors, suspended dependency requests and cached
completed values retain the same meter; neither opening an executor nor memoizing a result can
reset its storage charge.

The metric counts logical live storage units, not target bytes or process RSS. Each data node,
listed field/path/staging array charges one header plus its stored edge count. A repeated node
charges one header and one default edge; each sparse replacement charges a numeric key and a
value edge. Its logical count contributes no entries. Each dynamic frame, cell,
ownership entry, sparse hole or literal-import traversal entry charges two metadata units. Empty
canonical root paths need no allocation. Frame slots reference immutable plan schemas. Sharing
handles, paths or immutable data does not duplicate their allocation charge. Reservations precede
allocation; checked arithmetic rejects oversized reservations. Leases release charges automatically
on overwrite, last-reference drop or failed execution. Cells that still physically retain retired
data continue to count until overwritten or their frame ends.

`Value` is an immutable sharing handle. Plan literals are imported into the active meter with one
node per literal DAG identity, including when their data came from another budget. A write to
shared data reserves its new node and field references before shallow cloning; descendants stay
shared until their own path is updated. Thus scalar snapshots and independent aggregate values keep
their semantics while sharing physical data. Returned handles keep their lease after frame
teardown or publication. This is evaluator storage identity, not source nominal identity or a
source comparison capability. Integer nodes carry mathematical i128 data; exact i32/usize
range and operation identity come from validated slots, not from the payload or host integer size.

`Program::new` validates every literal shape and each operation/call's type edges. Execution
preserves that closed typing invariant through typed writes and canonical alias bindings; it does
not rewalk an entire value DAG on every call and return. Runtime checks still reject uninitialized
cells, wrong cell roles, invalid addresses and invalid ownership transitions. Alias creation resolves
to a non-alias root immediately, so no alias chain or cycle can enter a frame.

Deep value destruction uses an iterative worklist, including on exhausted-budget cleanup. Its
scratch is bounded by edges already retained in the value graph and cannot admit new evaluated
data. Host allocator overhead/capacity, immutable source/plans, ordinary checking, query bookkeeping
and diagnostic rendering are outside this working-storage metric.

### Frozen Publication

`state::Value::freeze` converts a completed value to the model-owned `FrozenData` DAG once per
required declaration. Identity-memoized postorder preserves sharing and never recursively expands
aggregates or repeated cardinalities. A frozen repeated node retains its default/count and sorted,
unique, in-range sparse replacements; every referenced value precedes the node. The artifact has no evaluator leases, scoped addresses, type-store IDs or source identity.
`from_frozen` reconstructs immutable plan literals from the selected root's reachable nodes only,
preserving shared components without rebuilding unrelated siblings or rejecting their unselected
scalar families. Its scratch depends on that reachable subgraph, not the retained graph's size.
It does not evaluate source;
closed plan construction validates the literal against its selected type. These are publication
and plan-construction transforms, not another evaluator or a source Copy capability.

### Immutable Static Backing

Closed plans retain explicit static address bindings separately from invocation inputs. Their
program carries established immutable data and evaluator-domain types by `ValueStatic` identity;
construction validates each binding's alias role, unique slot and data shape. No initializer code
or dependency request runs when a static address is bound. Checking supplies the already completed
query value, allowing the current meter's immutable nodes to stay shared rather than rebuilding
and charging another copy.

Memory addresses distinguish invocation-local roots from immutable static roots. Alias formation
canonicalizes both, so a static remains available across nested frames without becoming a local
owner or a hidden parameter. Each executor retains one metered backing entry per used static;
reads can share its data, while stores and ownership transitions reject that root. The backing
entries count against live storage and are released with the executor. This physical backing map
is not another initializer evaluator or a result-only cache for ordinary calls.
