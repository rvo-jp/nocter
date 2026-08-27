# nocter-machine

## Responsibility

Close machine layout and lower validated MIR into a target-independent machine program with explicit
ABI transport, storage, linkage, and primitive dependencies.

## Contract

The crate consumes `MirProgram`, concrete semantic representations, the selected runtime contract,
and target machine facts. It publishes immutable machine layouts and machine operations. It does not
select physical registers, encode instructions, write Mach-O, or reinterpret semantic declarations.

## Internal Responsibilities

- stored layout and aggregate representation
- call/result ABI classification and transport
- stack objects, machine control flow, and dataflow
- structural copy/destruction expansion
- deterministic linkage and primitive dependency closure

## Invariants

- Layout is computed once and reused by every machine consumer.
- Machine code cannot reach checking or target-program storage.
- ABI rules are represented in machine contracts, not duplicated by the ARM64 encoder.
- Runtime symbols identify already selected items and never drive semantic lookup.

The cross-stage boundary is documented in
[Machine Program and Native Target Design](../../../docs/machine-program-design.md).
