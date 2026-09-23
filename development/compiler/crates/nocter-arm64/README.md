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
- declaration-to-materializer binding for compiler-owned asynchronous primitive families
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
- Value planning assigns one physical register range to Machine-proven storage aliases. Instruction
  selection verifies that shared placement and emits no copy; it cannot infer aliases from source
  types or target-local operation heuristics.
- Non-overlapping memory copies stabilize both address roots in boundary-only registers before
  chunk materialization. Large stack offsets therefore cannot replace an indirect argument or
  result pointer that the remainder of the copy still needs.
- A runtime-projected erased-callable address is stabilized before its environment and invoke
  fields are loaded into ABI boundary registers; loading one field cannot invalidate the address
  needed for another.
- Every primitive expansion is selected by closed runtime role.
- Darwin kernel syscall numbers, trap encoding, and compiler-owned native records and OS constants
  have one backend-local authority. Source-owned target adapters retain their own native records;
  emitters accept only their typed pointer ABI and cannot reinterpret standard-library data.
- A kernel syscall emitter initializes every argument named by that backend-local ABI, including
  kernel-only output pointers absent from a public C wrapper. Ambient register contents never cross
  the syscall boundary as an accidental argument.
- Closed datagram lowering owns fixed socket calls and invocation flags. The target-specific
  standard adapter owns socket-address and message-header construction, malformed-record checks,
  retry policy, and descriptor cleanup after a configuration failure.
- Data-pointer fixups identify exact eight-byte fields and section-local targets; executable image
  policy remains outside this crate.
- Imported function and data slots retain exact kind-preserving trusted runtime identities; ARM64
  neither derives a loader symbol nor encodes a dylib command.
- Imported calls use the same preplanned scalar transport as other runtime calls, then load and
  branch through the machine import identity's pointer slot.
- Compiler-generated helpers treat allocation and process context registers as volatile across a
  foreign ABI call. A helper that consumes either context afterward must retain or reload it from
  owned frame state before dereferencing it; loader functions are never assumed to preserve the
  Nocter-only context lanes.
- Darwin native blocks use one runtime-owned layout schema and typed descriptor identity. The
  admitted materializer creates only a one-pointer, non-owning capture record; callers cannot pair
  an arbitrary data object or capture count with that record.
- Darwin network event transfer consumes the closed runtime catalog and emits one complete-record
  policy: interrupted calls retry, while EOF, short records, and permanent channel failures abort
  because safe native-owner progress is no longer possible.
- Connection event observation and complete-record receipt are production callable targets. The
  receiver keeps the ownership-bearing provider record in private target storage, consumes it, and
  returns only normalized state and error words through the source ABI.
- Plain connection construction publishes one atomic target set containing construction, event,
  and terminal-lifecycle entries. A consumer cannot request the constructor while accidentally
  omitting the operations required to drain callbacks and release its owner.
- Whole-program lowering scans Machine primitive calls once. Closed role usage and the exact ABI
  targets needed for source wrappers are passed to helper families; those families cannot
  rediscover dependencies by walking Machine independently.
- The generated process-lifecycle service owns one Darwin `kqueue` signal source for reload,
  interrupt, and termination. It consumes reload events, preserves the first termination request
  in the runtime-owned context, restores all replaced dispositions at process finalization, and
  exposes only descriptor and classified-observation targets to source wrappers.
- Plain connection start, cancellation, final-state observation, serial-queue quiescence, and
  release are production callable targets. Native qualification calls those targets and cannot
  carry a parallel lifecycle implementation; quiescence commits only after `dispatch_sync_f`
  returns on the owner's queue.
- Effective local and remote connection addresses use one provider-ownership cleanup path. The
  target validates and copies a complete IPv4 or IPv6 socket record before releasing its endpoint
  and path; only caller-owned bytes and their validated length cross the source ABI.
- Receive and send starters own their fixed callback signatures, Block descriptors, and callable
  entries as one transfer target set. Send copies borrowed source bytes into system-owned dispatch
  data before returning and releases its local dispatch owner immediately after Network.framework
  accepts the operation.
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
- A pending process root converts the ABI interest slice into one temporary Darwin event change
  list, event list, and rounded-up timer timeout. Descriptor readiness, process exit, and fixed
  monotonic deadlines share one `kevent64` wait. Every returned event is validated against the
  originating semantic record before readiness is published.
- The wait projection coalesces equal native registration keys for descriptor-direction and
  process-exit interests, then publishes a returned event to every matching semantic destination.
  Darwin's single registration key can therefore never discard a later computation's waiter.
- The generated Darwin file-service root is reached only through the runtime-owned process-context
  slot. Its ensure target constructs five serial queues, one worker group, a close-on-exec
  nonblocking wake pipe, and every fixed retirement record from the runtime schema; its shutdown
  target closes admission, drains the group, proves all records available, clears the slot, and
  releases the root. A target primitive cannot construct or free a parallel service root.
- Darwin's register-returning `pipe` syscall and close-once descriptor cleanup remain backend ABI
  facts. The file-service generator does not reinterpret the syscall as a C output-parameter call,
  and shutdown never retries or treats a failed `close` as proof that the descriptor stayed open.
- Generated file retirement is one target family covering bounded reservation, owner publication,
  drop, explicit-close future drive/cancel/consume, worker dispatch, wake notification, and group
  accounting. Reusable records publish `Available` only after transient descriptor, failure,
  readiness, and allocation-context state is cleared.
- Generated file operations share one target family for a closed constructor set, bounded admission,
  worker execution, drive, cancellation, and consumption. The future frame is the dispatched job;
  it owns path and write bytes, owns read output until consumption, and publishes one five-word
  completion record. Workers retain their notification descriptor and group before publishing
  completion, so successful publication is also the point after which they never inspect frame
  storage again.
- Source-visible file roles select that family through two opaque runtime-storage declarations.
  `FileCompletion` owns the uniform five-word result layout; compiler-generated drop calls clear
  and retire an unclaimed owner rather than requiring source policy to reconstruct target fields.
  The generated file-adapter family owns its direct-register call shapes, and instruction
  selection only verifies the already planned Machine ABI against those shapes. Ownership-transfer
  adapters name every scratch register explicitly, so clearing source storage cannot silently
  clobber the returned owner.
  When any file role is reachable, the process root invokes the family's shutdown target after
  source cleanup and before its normal return epilogue. A program with no file role declares
  neither the family nor its loader imports.
- Generated service counts use shared acquire/release bounded-increment and non-zero-decrement
  emitters. Saturation and underflow cannot wrap, and callers cannot omit lost-reservation retry or
  exclusive-reservation cleanup.
- Native process registration treats `ESRCH` as immediate readiness only after validating that the
  failed change originated from a process interest. This closes exit-before-registration without
  reaping status or introducing periodic probes. Interrupted and capped waits always recalculate
  from the fixed absolute deadline.
- Compiler-owned asynchronous primitives use an explicit dependency-indexed target table. Their
  constructors and lifecycle entries are declared only from frozen Machine primitive roles; code
  emission cannot infer their presence from source names or synthesize targets on demand.
- Descriptor readiness, process completion, and monotonic deadlines are allocation-backed opaque
  computations. Each frame owns one exact ABI interest record; their constructors differ, while
  resume, cancellation, and consumption share one lifecycle implementation and validate only its
  state.
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
