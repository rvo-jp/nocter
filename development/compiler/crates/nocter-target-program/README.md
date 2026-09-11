# nocter-target-program

## Responsibility

Validate one checked program against a selected toolchain target, then close one executable or test
root into a deterministic concrete reachable program.

## Contract

`TargetProgram` owns target/toolchain acceptance for `check`, `build`, and `run`.
`ExecutableProgram` owns entry-driven monomorphization, concrete callable closure, frozen dispatch,
and executable type representations consumed by MIR. The crate does not inspect syntax or repeat
checking decisions.

## Internal Responsibilities

- target capabilities, primitive completeness, trusted target-service ABI validation, and
  compiler-owned runtime-storage validation
- package target and entry validation
- executable and test root selection
- concrete instance, closure, and drop reachability
- monomorphized item and representation closure

## Invariants

- Target validation runs once before executable specialization.
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
- Every primitive whose closed signature returns `future T` must carry runtime-contract evidence
  that its drive and cancellation entries are nonblocking; certification on a non-future role is
  rejected as the same contract inconsistency.
- Executable specialization owns key construction; a consumer cannot pair a semantic identity with
  an unrelated specialization type store.
- Reachability uses semantic identities, never runtime symbol spelling.
- MIR receives no unresolved requirement, interface implementation, or generic lookup.

The cross-stage contract is documented in
[Target, Executable, and MIR Program Design](../../../design/target-program-design.md).
