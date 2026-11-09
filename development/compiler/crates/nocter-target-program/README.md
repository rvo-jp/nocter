# nocter-target-program

## Responsibility

Validate one checked program against a selected toolchain target, then close one executable or test
root into a deterministic concrete reachable program.

## Contract

`TargetProgram` owns target/toolchain acceptance for `check`, `build`, and `run`.
`ExecutableProgram` owns entry-driven monomorphization, concrete callable closure, frozen dispatch,
reachable declared-constant values, and executable type representations consumed by MIR. The crate
does not inspect syntax or repeat checking decisions.

## Internal Responsibilities

- target capabilities, primitive completeness, trusted target-service ABI validation, and
  compiler-owned runtime-storage validation
- package target and entry validation
- executable and test root selection
- concrete instance, closure, and drop reachability
- monomorphized item and representation closure
- private value-binding instance closure under `value_access`, consuming a complete accepted Unit
  and explicit root without granting released/native target admission
- concrete logical lifecycle schedules under `value_access::lifecycle`, retaining checked
  ownership conditions, operation phases and normal adapter exits before result receipt

## Invariants

- Released target validation runs once before executable specialization.
- Target validation consumes only `CheckedProgram`; this crate has no source-projection dependency.
  The checking-owned output maps that semantic component into `TargetProgram` and preserves or
  restores the exact projection pair outside this layer.
- Every concrete dispatch comes from checking's selected semantic authority.
- A target-service dispatch retains the catalog-owned target, calling convention, import, and ABI
  classes; executable closure never derives them from a declaration name.
- Runtime storage is accepted only from a toolchain-selected, fieldless, unique nominal declaration
  in the standard package; its physical layout remains outside semantic representation.
- Runtime-storage bindings are validated before primitive signatures that refer to them. An invalid
  prerequisite therefore cannot be reported as a downstream primitive mismatch.
- Runtime primitive generic slots are explicitly type-domain contracts. A constant-domain
  parameter is rejected during target validation rather than being silently discarded by MIR.
- Every primitive whose closed signature returns `future T` must carry runtime-contract evidence
  whose typed produced-computation classification certifies nonblocking drive and cancellation;
  certification on a non-future role is rejected as the same contract inconsistency.
- Executable specialization owns key construction; a consumer cannot pair a semantic identity with
  an unrelated specialization type store.
- Reachability uses semantic identities, never runtime symbol spelling.
- A concrete closure type enters the runtime type table only with the representation emitted by
  its reachable executable item. Closure types from unselected bodies remain semantic facts and
  cannot require runtime layout or body access.
- Checked constant references remain identities until executable closure freezes exactly the
  reached values; MIR cannot reopen declaration storage.
- MIR receives no unresolved requirement, interface implementation, or generic lookup.
- Private future function-witness keys have no released representation or runtime-type
  mapping. Both boundaries reject them rather than guessing a closure/function-pointer layout.

The cross-stage contract is documented in
[Target, Executable, and MIR Program Design](../../../architecture/pipeline/target-program.md).

## Private Value Instance Boundary

`value_access::InstanceProgram` owns the future private product's root-driven static instances.
It retains the exact accepted Unit and its body/type authority; no released CheckedProgram or
TargetProgram adapter exists. Roots requiring externally supplied callback witnesses reject as
unbound. Scalar/record runtime inputs remain ordinary data rather than specialization arguments.
This product grants no ABI, layout, executable cleanup or backend capability.

`build` interns keys of one named/captured code identity and its concrete formal-witness bindings.
A concrete witness pairs code identity with the supplied invocation view; the code instance itself
is shared across views. Runtime environment contents do not enter its key. The builder reserves
before following recursive edges and freezes only after every reachable instance is defined.
Only checking-reachable invocation commits create call edges. A value reference, uninvoked binding
or interrupted argument does not force a code instance. Construction indexes are discarded; one
immutable instance arena is the published authority.

`projection` substitutes already selected named/captured/formal origins and invocation views.
Callable results arrive with checking-closed code or caller-formal witnesses. The body's resolved
result data enters the same projection table; no target-side result inference or second instance
graph exists. Exact formal identity coverage is verified without repeating signature matching or
target search. One table maps source data IDs to concrete payload obligations or checked unproduced
structural contracts in the existing program authority. Unproduced data has no payload or code
instance. Captured payloads include their ordinary environment-data obligations even when reached
through a formal witness. Record schemas are visited iteratively, without expanding payload trees.
Source-selected moves, reservations and snapshots remain unchanged after concrete projection.

`invocation` binds each reached call's committed prepared bundle to the checked body's frozen input
layout. An edge retains the concrete instance, direct/value kind, explicit argument-to-local map,
environment channel and concrete input/result obligations. A named value supplies a code token without a
body environment; a captured value supplies its exact carrier and environment payloads to the declared body
slot. Original body invocation and supplied adapter view remain distinct. Normal completion either
retains the environment, leaves remaining-payload cleanup with a consuming body, or disposes an
adapter-owned environment before result delivery. Copy supply refers to its prepared snapshot,
not its original source owner. No unwind/cancellation guarantee is inferred from this normal exit
recipe. Final freeze verifies each edge's input/result payloads against its selected concrete body.
Input locals are relative to the selected body and a fresh invocation activation; sharing a
static instance never shares runtime environment values, owner state or storage between calls.

Each instance retains its exact accepted body and these frozen edges/data maps. Consumers cannot
recheck a body, select by names, infer an environment offset from arity, recover source-level Copy
from concrete payloads, or reinterpret a closure's source owner as its code body. The graph admits the bounded owned-capture producer through that same code authority. General
captures, nested/factory-result closure bodies, physical input/environment realization, executable
cleanup, ABI and backend admission remain separate obligations. The
[private cross-stage contract](../../../architecture/pipeline/value-access-declarations.md) owns
the handoff; the [current realization plan](../../../design/compiler-realization.md) owns remaining
admission obligations.

## Private Lifecycle Schedules

`value_access::lifecycle::Program` freezes derived schedules over one retained InstanceProgram;
it has no parts constructor. `lower` enumerates reachable checked graph locations and merges only
identical guard/schedule pairs. A projected condition admitting different schedules rejects
explicitly, rather than selecting by report order. No second CFG or runtime-state store is built.

`project` substitutes payload obligations through the existing per-instance data map using
checking's exhaustive resource-step transform. It does not select operations, fields or dispatch.
`completion` expands the frozen EnvironmentExit recipe only for reached calls; it does not
reclassify the supplied invocation or the body's environment authority.

The [lifecycle handoff](../../../architecture/pipeline/value-access-declarations.md#private-lifecycle-schedule-handoff)
owns cross-stage ordering, answer protection and physical-consumer obligations. This private
logical product grants no released ExecutableProgram conversion, layout, ABI or native admission.
