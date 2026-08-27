# Machine Program and Native Target Boundary

This document owns the handoff from concrete MIR to native executable bytes. Internal mechanisms
belong in the [`nocter-machine`](../compiler/crates/nocter-machine/README.md),
[`nocter-arm64`](../compiler/crates/nocter-arm64/README.md), and
[`nocter-macho`](../compiler/crates/nocter-macho/README.md) READMEs.

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
and destruction operations, closes primitive dependencies, and owns deterministic machine linkage.

`Arm64Program` assigns physical registers and frames, selects and encodes instructions, and resolves
branch/data fixups. `MachOImage` assigns file-format structures and emits final bytes. Artifact
publication remains outside all three products.

## ABI Ownership

The public ABI contract comes from `spec/09-abi-layout.md`. Machine is its sole implementation owner
for stored layout and argument/result transport. ARM64 consumes already classified machine values;
it cannot independently decide aggregate layout or source-level calling convention. Mach-O consumes
encoded sections and cannot alter code selection or linkage identity.

## Semantic Closure

Machine input contains closed runtime representations and primitive roles. No native stage can
inspect a semantic declaration, generic requirement, interface implementation, source path, or
rendered type. Structural copy and destruction use explicit concrete schemas rather than recovering
field or variant structure from names.

## Required Invariants

- Every concrete type has one stored-layout result.
- Every call site and callee use the same machine transport plan.
- Machine linkage names already selected items and never performs semantic lookup.
- ARM64 register allocation cannot change ABI classification.
- Primitive expansion is selected by closed runtime role.
- Mach-O writing is deterministic and cannot introduce executable items.
- No partial image is a successful native artifact.
