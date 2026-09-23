# Machine Program and Native Target Boundary

This document owns the handoff from concrete MIR to native executable bytes. Internal mechanisms
belong in the [`nocter-machine`](../../compiler/crates/nocter-machine/README.md),
[`nocter-arm64`](../../compiler/crates/nocter-arm64/README.md), and
[`nocter-macho`](../../compiler/crates/nocter-macho/README.md) READMEs.

## Pipeline

```text
MirProgram
  -> MachineLayoutStore
  -> MachineProgram
  -> Arm64Program
  -> MachOImage
```

`MachineLayoutStore` assigns immutable stored layouts to the concrete types already closed by
Executable. `MachineProgram` classifies arguments and results, expands target-independent storage
and destruction operations, closes primitive dependencies and imported-service identities, and owns
deterministic machine linkage. Each function then follows one Machine-owned mutable body draft,
target-independent optimization, immutable freeze, and dataflow-validation path. Generated
destruction and erased-callable functions cannot bypass that path.

`Arm64Program` assigns physical registers and frames, selects and encodes instructions, and resolves
branch/data fixups. `MachOImage` assigns file-format structures and emits final bytes. Artifact
publication remains outside all three products.

Optimization is an internal Machine transformation rather than an additional cross-stage product.
Its input is already closed machine operations, layouts, ABI transport, linkage, and runtime roles.
It cannot inspect MIR or an earlier semantic product, and ARM64 cannot repeat a target-independent
decision. Immutable liveness is derived once from the optimized body rather than repaired after a
mutation.

## ABI Ownership

The public ABI contract comes from `spec/platform/abi-and-layout.md`. Machine is its sole implementation owner
for stored layout and argument/result transport. ARM64 consumes already classified machine values;
it cannot independently decide aggregate layout or source-level calling convention. Mach-O consumes
encoded sections and cannot alter code selection or linkage identity.

## Semantic Closure

Machine input contains closed runtime representations, primitive roles, and target-service
descriptors. No native stage can
inspect a semantic declaration, generic requirement, interface implementation, source path, or
rendered type. Structural copy and destruction use explicit concrete schemas rather than recovering
field or variant structure from names.

## Required Invariants

- Every concrete type has one stored-layout result.
- Every call site and callee use the same machine transport plan.
- Machine linkage names already selected items and never performs semantic lookup.
- ARM64 register allocation cannot change ABI classification.
- Primitive expansion is selected by closed runtime role.
- Operation removal requires Machine-owned proof that evaluation is both unobservable and
  non-trapping; unknown effects are retained.
- Safety checks remain unless an exact path proof makes their trap condition impossible.
- Imported-service descriptors are interned once into a dense machine domain; calls retain only its
  identity and the shared preplanned runtime-call ABI.
- Mach-O writing is deterministic and cannot introduce executable items.
- No partial image is a successful native artifact.
