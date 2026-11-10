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
- One dependency query owns absent, active, and completed key states. It memoizes shared
  dependencies, reports exact active cycles, and removes failed active branches before returning.
- Each completed query value is allocated once and shared across every dependent request. The
  query has no `Clone` implementation, so a semantic construction cannot fork a second memoization
  authority or copy a large frozen aggregate on each dependency edge.
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

`Type::builtin` owns the admitted fixed-width primitive domain. The checker uses that same admission
for body-domain checks. Ordered aggregate schemas compose those leaves without claiming Copy. Checking retains source
record/tuple type distinctions while using that one evaluator shape. Schema
projection and value validation memoize shared subgraphs rather than expanding trees. Boolean/i32
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
caches completed values through the common `DependencyQuery`. Unresolved declaration references
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

### Live Working Storage

`state::Limits` adds a deterministic storage limit without giving the expression-tree executor an
unenforced setting. `state::Budget` owns the one work counter and one shared storage meter. An
outer request restarts work only. All nested executors, suspended dependency requests and cached
completed values retain the same meter; neither opening an executor nor memoizing a result can
reset its storage charge.

The metric counts logical live storage units, not target bytes or process RSS. Each data node,
field/path/staging array charges one header plus its element count. Each dynamic frame, cell,
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
source comparison capability.

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
aggregates. The artifact has no evaluator leases, scoped addresses, type-store IDs or source identity.
`from_frozen` reconstructs immutable plan literals from those established nodes without evaluating source;
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
