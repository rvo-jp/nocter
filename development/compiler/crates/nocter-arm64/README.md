# nocter-arm64

## Responsibility

Select, allocate, and encode ARM64 instructions for one immutable machine program.

## Contract

The crate consumes target-independent machine operations and runtime roles. It publishes an
`Arm64Program` containing encoded code/data sections, function-import pointer slots, and fixup
information for the image writer. It does not inspect MIR semantics, declaration identities,
source, loader commands, or package state.

## Internal Responsibilities

- instruction and addressing selection
- call, aggregate, pack, primitive, error, and region lowering
- frame layout and register allocation
- allocation-backed async-frame placement from the exact Machine field union
- parallel-copy resolution
- branch, code-to-data, data-to-data, and imported-function pointer-slot fixups plus instruction
  encoding

## Invariants

- ARM64 selection implements the ABI already classified by Machine.
- The operation and selected-instruction enums are each classified exactly once. Subsystem helpers
  receive destructured payloads or a closed subsystem operation, never the complete parent enum.
- Physical register decisions cannot change semantic value transport.
- Every primitive expansion is selected by closed runtime role.
- Data-pointer fixups identify exact eight-byte fields and section-local targets; executable image
  policy remains outside this crate.
- Imported-function slots retain exact trusted runtime identities; ARM64 neither derives a loader
  symbol nor encodes a dylib command.
- Imported calls use the same preplanned scalar transport as other runtime calls, then load and
  branch through the machine import identity's pointer slot.
- A monotonic-counter observation is emitted as an ordered observation, never as a speculative
  bare system-register read.
- Encoding is deterministic for one machine program.
- Stack and async frames share one aligned-object placement authority. Async placement assigns one
  stable byte range to each Machine-selected live identity and never repeats suspension liveness.
- Until the v0.41.0 executor entry points exist, selection rejects deferred functions and
  computation-release operations explicitly. It never encodes a state-machine body as though it
  implemented the immediate callable ABI.
