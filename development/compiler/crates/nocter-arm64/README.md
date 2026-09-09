# nocter-arm64

## Responsibility

Select, allocate, and encode ARM64 instructions for one immutable machine program.

## Contract

The crate consumes target-independent machine operations and runtime roles. It publishes an
`Arm64Program` containing encoded code/data sections, typed runtime-import pointer slots, and fixup
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
- Darwin kernel syscall numbers, trap encoding, native record layouts, and OS value constants have
  one backend-local authority. Emitters prepare operation-specific values but cannot restate the
  kernel ABI.
- Data-pointer fixups identify exact eight-byte fields and section-local targets; executable image
  policy remains outside this crate.
- Imported function and data slots retain exact kind-preserving trusted runtime identities; ARM64
  neither derives a loader symbol nor encodes a dylib command.
- Imported calls use the same preplanned scalar transport as other runtime calls, then load and
  branch through the machine import identity's pointer slot.
- Darwin native blocks use one runtime-owned layout schema and typed descriptor identity. The
  admitted materializer creates only a one-pointer, non-owning capture record; callers cannot pair
  an arbitrary data object or capture count with that record.
- Darwin network event transfer consumes the closed runtime catalog and emits one complete-record
  policy: interrupted calls retry, while EOF, short records, and permanent channel failures abort
  because safe native-owner progress is no longer possible.
- Connection event observation and complete-record receipt are production callable targets. The
  latter writes to an explicit caller-owned opaque event record; its pointer and descriptor remain
  in nonvolatile registers across retry and errno calls.
- Plain connection start, cancellation, final-state observation, serial-queue quiescence, and
  release are production callable targets. Native qualification calls those targets and cannot
  carry a parallel lifecycle implementation; quiescence commits only after `dispatch_sync_f`
  returns on the owner's queue.
- A monotonic-counter observation is emitted as an ordered observation, never as a speculative
  bare system-register read.
- Encoding is deterministic for one machine program.
- Stack and async frames share one aligned-object placement authority. Async placement assigns one
  stable byte range to each Machine-selected live identity and never repeats suspension liveness.
- Async frames and compiler-owned computations consume complete lifecycle-state tag sequences from
  the runtime contract. Instruction emission cannot assign numeric initial, suspension, or
  completed tags.
- Deferred functions own four distinct native entries: constructor, resume, cancellation, and
  completed-output consumption. Whole-program lowering declares all four identities before any
  body is materialized.
- A deferred process entry is driven only by the compiler-owned process root. The root consumes
  the opaque lifecycle entries and output storage selected by Machine; ordinary calls remain lazy.
- A pending process root converts the ABI interest slice into one temporary Darwin `pollfd` array
  and the earliest timer deadline into a rounded-up relative timeout. It retries interrupted waits,
  re-waits after a capped timeout until the wrapping absolute deadline is eligible, releases native
  storage before resumption, and never polls a pending computation in a busy loop.
- Compiler-owned asynchronous primitives use an explicit dependency-indexed target table. Their
  constructors and lifecycle entries are declared only from frozen Machine primitive roles; code
  emission cannot infer their presence from source names or synthesize targets on demand.
- Descriptor readiness and monotonic deadlines are allocation-backed opaque computations. Each
  frame owns one exact ABI interest record; their constructors differ, while resume, cancellation,
  and consumption share one lifecycle implementation and validate only its state.
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
