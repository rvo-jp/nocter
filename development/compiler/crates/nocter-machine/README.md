# nocter-machine

## Responsibility

Close machine layout and lower validated MIR into a target-independent machine program with explicit
ABI transport, storage, linkage, primitive dependencies, and trusted function imports.

## Contract

The crate consumes `MirProgram`, concrete semantic representations, the selected runtime contract,
and target machine facts. It publishes immutable machine layouts and machine operations. It does not
select physical registers, encode instructions, write Mach-O, or reinterpret semantic declarations.

## Internal Responsibilities

- stored layout and aggregate representation
- immutable-static serialization and data-to-data relocation construction
- call/result ABI classification and transport shared by primitive and imported calls
- one canonical imported-service identity domain retained independently of MIR
- stack objects, machine control flow, and dataflow
- deferred function execution, suspension frames, and frozen cancellation/output destruction
- explicit process-root ownership and output storage for a deferred executable entry
- structural copy/destruction expansion
- deterministic linkage and primitive dependency closure

## Invariants

- Layout is computed once and reused by every machine consumer.
- Machine code cannot reach checking or target-program storage.
- ABI rules are represented in machine contracts, not duplicated by the ARM64 encoder.
- Runtime symbols identify already selected items and never drive semantic lookup.
- Imported calls retain only a dense machine import identity; their catalog descriptor is stored
  once in the program and cannot be rebuilt from source spelling.
- Each reachable static retains its declaration identity as one addressable data object; text
  payloads may be shared, but equal static values cannot be merged.
- Machine projects MIR suspension fields and cleanup plans to dense identities without repeating
  liveness or ownership analysis.
- Deferred invocation and its state-machine body remain distinct from the ordinary callable ABI.
  The initial allocation-backed representation always requests an incoming allocation context.

The cross-stage boundary is documented in
[Machine Program and Native Target Design](../../../design/machine-program-design.md).
