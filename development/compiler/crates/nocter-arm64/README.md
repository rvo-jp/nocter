# nocter-arm64

## Responsibility

Select, allocate, and encode ARM64 instructions for one immutable machine program.

## Contract

The crate consumes target-independent machine operations and runtime roles. It publishes an
`Arm64Program` containing encoded code/data sections and fixup information for the image writer. It
does not inspect MIR semantics, declaration identities, source, or package state.

## Internal Responsibilities

- instruction and addressing selection
- call, aggregate, pack, primitive, error, and region lowering
- frame layout and register allocation
- parallel-copy resolution
- branch/data fixups and instruction encoding

## Invariants

- ARM64 selection implements the ABI already classified by Machine.
- The operation and selected-instruction enums are each classified exactly once. Subsystem helpers
  receive destructured payloads or a closed subsystem operation, never the complete parent enum.
- Physical register decisions cannot change semantic value transport.
- Every primitive expansion is selected by closed runtime role.
- A monotonic-counter observation is emitted as an ordered observation, never as a speculative
  bare system-register read.
- Encoding is deterministic for one machine program.
