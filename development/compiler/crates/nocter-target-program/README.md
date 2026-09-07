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

- target capabilities, primitive completeness, and trusted target-service ABI validation
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
- Executable specialization owns key construction; a consumer cannot pair a semantic identity with
  an unrelated specialization type store.
- Reachability uses semantic identities, never runtime symbol spelling.
- MIR receives no unresolved requirement, interface implementation, or generic lookup.

The cross-stage contract is documented in
[Target, Executable, and MIR Program Design](../../../design/target-program-design.md).
