# Target, Executable, and MIR Boundary

This document owns the cross-crate handoff from checked semantics to concrete MIR. Internal design
belongs in the [`nocter-target-program`](../compiler/crates/nocter-target-program/README.md) and
[`nocter-mir`](../compiler/crates/nocter-mir/README.md) READMEs.

## Pipeline

```text
CheckedProgram + ToolchainSnapshot
  -> TargetProgram
  -> ExecutableProgram
  -> MirProgram
```

`TargetProgram` is the common public acceptance boundary for `check`, `build`, and `run`. It owns the
selected target and validates target availability, toolchain primitive completeness, package target
identity, and target-dependent buildability once. A library-only check may stop at this boundary.

`ExecutableProgram` consumes one exact executable or native-test selection and closes its reachable
monomorphized callable graph. It freezes concrete dispatch, closure/drop instances, primitive
dependencies, and executable type representations before MIR.

`MirProgram` consumes that closed graph and expresses concrete control flow, places, operations,
cleanup, regions, outcomes, packs, and calls. MIR validation checks internal representation
integrity; it cannot reject a source-language capability already accepted by Target.

## Dispatch Contract

Checking retains abstract requirements for generic bodies and owns the selector capable of proving
their concrete substitutions. Executable specialization asks that authority once and stores the
selected direct callable, primitive, closure body, coercion-plus-operation plan, or other closed
dispatch result.

MIR receives only that result. It cannot inspect interface implementations, instance declarations,
requirements, method names, or source visibility. Callable values are statically witnessed and do
not become an erased runtime interface or vtable.

## Representation Contract

Executable specialization freezes every concrete nominal field/variant payload and opaque witness
needed by reachable items. MIR uses those identities when constructing concrete places and
operations. Machine layout later assigns storage and ABI classification; Target and MIR do not
implement machine layout.

## Entry and Reachability

An executable or test root is compiler-owned metadata, not a synthetic source declaration.
Reachability starts from that exact root and follows checked call/dependency identities. Runtime
symbol spelling is generated after selection and cannot be used to locate a semantic item.

## Required Invariants

- Target acceptance is shared by all commands and runs once per checked/toolchain pair.
- Executable roots cannot be invented for library-only packages.
- Concrete substitutions contain every owner and callable generic argument exactly once.
- Every reachable call has a frozen target or primitive role before MIR.
- MIR cannot inspect syntax, declarations, source projection, or generic proof inputs.
- A later backend failure is an integrity/output failure, not a second language diagnostic.
