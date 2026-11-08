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
- Private future named-function witness keys have no released representation or runtime-type
  mapping. Both boundaries reject them rather than guessing a closure/function-pointer layout.

The cross-stage contract is documented in
[Target, Executable, and MIR Program Design](../../../architecture/pipeline/target-program.md).

## Private Value Instance Boundary

`value_access::InstanceProgram` owns the future private product's root-driven static instances.
It retains the exact accepted Unit and its body/type authority; no released CheckedProgram or
TargetProgram adapter exists. Roots requiring externally supplied callback witnesses reject as
unbound. Scalar/record runtime inputs remain ordinary data rather than specialization arguments.
This product grants no ABI, layout, executable cleanup or backend capability.

`build` interns keys of one callable and its concrete formal-witness bindings, reserves before
following recursive edges, and freezes only after every reachable instance is defined. Only
checking-reachable invocation commits create call edges. A function reference or interrupted
argument does not force an uninvoked body into the graph. Construction indexes are discarded;
one immutable instance arena is the published authority.

`projection` substitutes already selected named/formal origins and invocation views. Callable
results arrive with checking-closed named witnesses; the body's resolved result data enters the
same projection table. No target-side result inference or second instance graph exists. It verifies
formal identity coverage without repeating signature matching or target search. One table maps
source data IDs to concrete payload obligations in the existing program type authority. Schema
cleanup classification remains checking-owned. Record closure visits shared schemas iteratively,
without expanding payload trees. No symbolic environment destruction remains in this concrete
projection, while the original body's source-selected moves/reservations stay unchanged.

Each instance pairs frozen call edges and data projections with its original checked body. Later
consumers must use those edges/projections and the recorded direct/value invocation kind; they
cannot infer an environment offset from arity, recheck a body, select by names, or reinterpret
source-level Copy from the concrete payload. Input-dependent results and captured producers require
further source closure before admission. The [private cross-stage contract](../../../architecture/pipeline/value-access-declarations.md)
owns the handoff; the review/handoff own work status.
