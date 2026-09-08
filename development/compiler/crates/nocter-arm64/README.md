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
- Deferred functions own four distinct native entries: constructor, resume, cancellation, and
  completed-output consumption. Whole-program lowering declares all four identities before any
  body is materialized.
- Resume restores only the Machine-selected state projection, uses ordinary selected-operation
  emission for body instructions, and persists exactly that projection when a child remains
  pending.
- Computation release loads the cancellation entry from the opaque runtime header. Callers never
  inspect a deferred function's state layout or cleanup plan.
- Deferred variadic-pack construction moves a caller stack descriptor and callback state into one
  allocation-backed owner. Forwarding an already allocation-backed pack transfers that owner
  directly instead of copying its owned elements. The same descriptor ownership marker makes
  ordinary body cleanup and cancellation release the allocation exactly once without changing
  synchronous stack packs.
