# Machine Program and Native Target Design

This document defines machine-layout, machine-program, ARM64, and Mach-O implementation
responsibilities. It does not define language behavior. Stored layout, argument and result
transport, primitive behavior, and executable image requirements remain owned by the public
specification.

## Boundaries

The machine pipeline has four distinct authorities:

```text
MirProgram
  -> MachineLayoutStore
  -> MachineProgram
  -> Arm64Program
  -> MachOImage
```

`MachineLayoutStore` computes immutable stored representations from concrete MIR types and the ABI
identity already selected by `ToolchainSnapshot`. `MachineProgram` owns ABI-classified values,
stack objects, calls, control flow, and deterministic linkage without assigning physical ARM64
registers. `Arm64Program` owns instruction selection, physical registers, frame layout, branch
fixups, and encoded sections. `MachOImage` owns file-format tables and bytes.

No boundary may inspect syntax, source paths, source names, generic requirements, interface-implementation
tables, or archived compiler behavior. A later boundary consumes the decision made by the prior
one; it cannot reconstruct it from a type spelling or operation shape.

## Concrete Representation Closure

Machine layout is not a second generic-specialization engine. Before MIR construction,
`ExecutableProgram` freezes one `ExecutableTypeRepresentationTable` in its forked concrete type
store. Each concrete nominal type records declaration-order specialized field or variant-payload
types. Each concrete opaque type records its exact specialized witness. Closure environments keep
their existing executable-owned capture table.

This division leaves two non-overlapping responsibilities:

- executable construction decides which concrete type each semantic member or witness denotes
- machine layout decides the size, alignment, stride, and byte offset of those concrete types

The representation closure uses an explicit pending set and an independent completed set. Recursive
child discovery therefore cannot make result order or validity depend on numeric `TypeId` order.
Symbolic representation children are rejected before machine layout.

## Stored Layout Authority

`nocter-machine` owns `MachineTarget`, `MachineLayoutStore`, and the complete recursive layout for
every runtime type referenced by validated MIR. The implementation follows
[ABI and Layout](../../spec/09-abi-layout.md) and currently selects only the frozen
`Arm64DarwinV1` ABI identity.

Each stored layout records its byte size, byte alignment, representation class, concrete child
types, and every offset a later stage needs. This includes view pointer and length offsets, the
one-word error handle, enum and outcome tag and payload offsets, nominal fields, variant payloads,
fixed-array stride, closure captures, and opaque witnesses. Downstream lowering must query this
table; it may not repeat padding arithmetic or embed an independently remembered offset. Runtime
error-node offsets and tags belong only to `nocter-runtime-contract`, not to stored semantic type
layout.

The builder starts from function signatures and all local, place, value, pack, process-root, and
test-root types in `MirProgram`, then closes recursively over stored children. `void` and `never`
remain completion types without stored layouts. Borrows to `str` and slices are two-word views;
other raw pointers and borrows are one-word pointers. Unsized or symbolic by-value types,
incomplete representations, recursive values, invalid alignments, and arithmetic overflow are
typed compiler-integrity failures. A construction-only `MachineLayoutPlan` additionally maps
semantic field, variant, payload-parameter, and capture identities to those physical members while
MIR is lowered. `finish` discards that correspondence. `MachineLayoutStore` therefore exposes
physical representation only and cannot be used by target lowering to reinterpret semantic
membership.

Stored layout and call transport remain separate. A type has exactly one stored layout even when
the ABI later carries it in zero, one, or two words or indirectly. This separation is required for
locals, projections, aggregate construction, inactive outcome payloads, and caller-provided result
storage to agree without encoding call-site policy in type layout.

## Machine Program Authority

`MachineProgram` construction consumes `MirProgram` together with its `MachineLayoutPlan`, then
retains only the completed `MachineLayoutStore`. Its
schema uses distinct dense identities for functions, blocks, values, stack objects, and constants.
Typed operations make loads, stores, copies, address projections,
integer operations, tags, direct calls, primitive calls, and exits explicit. Aggregate movement and
destruction must use the stored-layout authority and MIR's active-payload control flow rather than
introducing a second semantic aggregate model.

During construction, every linkage entry owns exactly one emitted function in the same dense
order. `MachineLinkagePlan` and `MachineFunctionDomain` derive executable-item and
generated-destruction function IDs from that order without parallel maps. The final function table
and root contain only machine identities; semantic linkage keys and reverse indexes are discarded
when construction finishes.

ABI lowering belongs at this boundary. Each callable input and result receives one immutable
transport plan naming direct words, stack slots, or indirect storage. Callers and callees must
share the same plan object or derive identical plans through one planner. Compiler-owned process
and test roots use the same call boundary as ordinary direct calls; they remain roots rather than
synthetic functions.

`MachineAbiPlan` now freezes that contract once for every dense executable item. Stored values are
classified as zero-word, one/two-word direct, or indirect. Arguments are placed left to right. The
first transport that does not fit completely in the remaining argument registers closes the
register window; later non-zero transports use ordered aligned stack slots even if a smaller value
would fit an abandoned register. The planner records final call-boundary stack padding separately
from each slot. Results distinguish completion, divergence, omitted zero-sized values, direct
result registers, and caller-owned indirect storage.

Argument packs remain outside the ordinary argument list. A machine signature may combine normal
fixed parameters with one compiler-owned pack-descriptor pointer lane without inventing a source
type or platform variadic ABI. The
machine program assigns every caller-owned descriptor a body-local `MachinePackId`. Each descriptor
retains its exact element and optional-next types, total-length value, and ordered fixed or spread
segments. A spread contains only a machine address, remaining-count value, direct function
identity, receiver byte offset within the transferred iterator, contribution mode, and optional
generated cleanup-function target. The referenced function remains the single callable-ABI
authority. Machine lowering proves the receiver is a
static subaddress of the iterator and removes the caller-side borrow SSA value; target callbacks
never retain a pointer into the caller's former iterator storage. Fixed segments use the same
cleanup-function domain. The callable body exposes only explicit length, consuming-next, and destroy
operations over its hidden pointer.

Concrete destruction plans are interned once across pointer primitives and pack segments while
the machine program is constructed. Their generated functions use one compiler-owned
`(byte_pointer, byte_offset)` ABI. Construction then discards the recursive plans, MIR-operation
edges, and plan-to-function indexes. A partially consumed pack and the final program retain only
ordinary function identities; neither carries a recursive recipe into callback or target lowering.

Linkage identities derive from dense executable item and compiler-owned root identities, not
source spellings. Human-readable names may be retained as presentation metadata only after a
collision-free semantic linkage key exists. Primitive lowering dispatches on the closed
`PrimitiveRole` carried by MIR and never searches a standard-library symbol name.

`MachineLinkagePlan` temporarily closes one code identity for every executable item, process root,
isolated test root, and generated destruction from typed owner keys. Test presentation names move
into the final root, which preserves declaration order independently of key order. Static UTF-8
data uses a distinct `MachineDataId` domain. `MachineDataPlan` deduplicates complete byte strings
and assigns IDs in byte order; its text-to-ID index is discarded after constants are lowered, while
the final `MachineDataTable` retains only dense bytes.

`MachineProgram` now owns a separate dense function domain and body-local stack-object, drop-flag,
address, SSA-value, operation, argument-pack, and basic-block domains. MIR identities are translated
once while the program is built and are absent from the immutable result. Every address starts from
an abstract stack object, pointer value, or two-word view and then uses only byte offsets,
dereferences, and checked index steps. Fixed arrays retain their declared bound and layout-owned
stride. Slice and string indexes retain the length from the current view. Field, capture,
variant-payload, outcome, and opaque-witness identities are consumed while those steps are
constructed.

Function entries carry the single callable ABI object owned by `MachineAbiPlan`. One
`MachineCallTarget` domain distinguishes direct machine functions from compiler-known primitives;
arguments, allocation context, and the optional hidden pack transport use one call representation.
Direct targets therefore cannot invent a second call contract, and new target kinds cannot create
a parallel argument-transport path.

`MachineContextPlans` is the single whole-program authority for compiler-propagated ambient
capabilities. Its shared fixed-point engine builds independent allocation and process plans over
ordinary calls, user-drop calls, spread iterator callbacks, and generated residual-destruction
functions. A forwarding edge propagates callback context requirements from its incoming-pack
callable to the next consumer during the same fixed point. Roots establish the program-lifetime
allocation default. An explicit `using` selection
supplies an allocation-dependent callee without making its caller allocation-dependent; it does
not interrupt process-state propagation. Process roots and independently launched test roots own a
process context only when their reachable graph queries entry state. The plans are complete before
ARM64 selection, so the target layer neither scans operations nor guesses whether to emit either
hidden context lane.

Constants, loads, address formation, stores, aggregate writes, scalar operations, integer
conversion, drop-flag control, block arguments, scalar and stored-tag switches, returns, process
exits, and direct calls have closed machine forms. Aggregate writes and tag switches use the same
stored-layout offsets and tag values as address projection. An explicit call allocation context is
an address identity rather than a retained MIR place. Unsupported MIR operations fail construction
explicitly instead of surviving inside a generic passthrough operation.

SSA identity and stored representation are intentionally separate. A `MachineValue` is classified
as stored bytes, successful completion, or divergence. Only stored values require size and
alignment; explicit MIR completion values therefore survive control and call wiring without
inventing a zero-byte layout for `void` or `never`. User-drop invocation names a machine function,
machine address, and the same closed allocation selection as an ordinary call. Region creation
names its parent value and compiler-owned stack resource directly; release names that same
resource. Lexical selections likewise retain a machine stack identity rather than an address to
movable source storage. Process error reporting names the exact machine error value. None retain
MIR-local identities.

Standard primitive targets retain their closed `PrimitiveRole`, concrete layout-key type
arguments, exact concrete signature, and an explicit specialized semantic dependency. Most
primitives carry no dependency. Pointer destruction carries its concrete subject plus an optional
`MachineDestructionPlan`; absence of a plan means the subject is known not to need destruction,
not that analysis was omitted. MIR-to-machine lowering translates every nested layout and user-drop
item once. Plans with work from either pointer calls or argument-pack segments enter a
construction-only, content-ordered `MachineDestructionPlanTable`, receive generated linkage only
after the source-function domain, and use the common compiler-owned
`(byte_pointer, byte_offset)` ABI. Pointer calls become ordinary direct calls; pack segments retain
only that generated function identity, and the plan table is discarded before `MachineProgram` is
returned. The generated
function expands struct, active enum/outcome payload,
closure, and opaque traversal into machine CFG. Fixed arrays use one reverse loop with a
compiler-validated dynamic byte-offset step, so code size is independent of array length. Only
authored drop bodies remain direct nested calls. The allocation-context fixed point follows those
calls through the generated function. A known-empty plan remains a validated no-op primitive. No
ARM64 component receives or recursively interprets a destruction plan.

Compiler-provided structural behavior does not cross a call boundary. Machine lowering replaces
primitive equality and ordering with comparisons that name exact scalar signedness or an enum tag
offset. Built-in indexing becomes one checked borrow operation containing its fixed bound or view
pointer/length offsets and element stride. Readwrite-to-readonly borrow weakening becomes an
explicit representation-preserving operation. The machine program therefore retains neither a
structural dispatch target nor a reason to reopen type selection.

Every `MachineFunction` owns one `MachineFunctionDataflow` derived while its body is closed.
Operation inputs include dependencies nested inside checked addresses, dynamic indexes, explicit
allocation addresses, and fixed or spread argument-pack segments; target lowering never needs a
second operation-shape walker to recover them. The same authority validates that operation results
and block parameters point back to their exact definition sites, that every operation has one
block owner, and that branch arguments match destination parameters in arity and type.

CFG liveness treats block parameters as definitions local to the destination and predecessor edge
arguments as terminator inputs. A deterministic backwards fixed point records block `live_in` and
`live_out`, then a reverse block walk records the exact live set immediately after every operation.
This operation-level boundary is required for calls: a value defined before a call and live after
it must use a callee-saved register or spill, while the call's own result starts at the call and is
not incorrectly classified as surviving its clobber. ARM64 allocation consumes these facts; it
does not infer liveness from linear instruction order or reopen machine addresses and packs.

## ARM64 and Mach-O Boundaries

ARM64 lowering receives only validated machine operations and completed transport plans. It owns
register allocation, spills, callee-saved preservation, frame offsets, instruction selection,
literal pools, and branch relaxation. Those decisions cannot change Nocter type layout or ABI
classification.

`nocter-arm64` is a separate crate whose dependency edge points only to `nocter-machine`. Its first
closed layer represents physical general registers separately from the instruction-specific `sp`
and zero-register roles. Instruction encoding accepts typed arithmetic, multiplication, division,
variable shifts, scaled loads/stores, conditions, branches, returns, traps, and supervisor calls.
It rejects out-of-range immediates, invalid wide-move shifts, misaligned branches, and offsets that
would otherwise be silently truncated. Encoding tests are cross-checked against the platform ARM64
assembler rather than treating hand-written hexadecimal values as their own authority.

Local control flow uses dense `Arm64LabelId` values until final function layout. One builder binds
each label once, rejects unresolved identities, computes byte offsets with checked arithmetic, and
encodes only after placement stabilizes. A conditional branch outside its signed 19-bit word range
is expanded to an inverted short condition over one signed 26-bit unconditional branch. Relaxation
is monotonic and recomputes every affected label; it never patches a truncated displacement.

The target ABI register partition is one closed authority shared by allocation and frame planning.
The fixed `x9` lane carries the compiler-propagated allocation-context pointer and never enters
general allocation. Fixed argument/result lanes are likewise boundary-only. Virtual values use
`x11`-`x15` or `x19`-`x28`; a range live across a call may use only the latter or a spill slot.
`Arm64ValuePlan` first partitions machine values into omitted storage, one or two virtual word
lanes, or memory storage for values larger than the direct ABI limit. It uses machine operation
inputs and block liveness to extend deterministic intervals. Call crossing is marked directly from
the call operation's `live_after` set, excluding the call result, rather than inferred from a
flattened interval that may contain unrelated sibling blocks. Every direct lane then uses the
common linear-scan allocator; memory values remain explicit requests for later frame placement.
`Arm64FunctionFrame` is the sole placement boundary. In fixed category order it reserves outgoing
arguments, machine stack objects, drop flags, memory-backed values, one reusable direct-aggregate
construction object, one maximum-size memory-edge cycle object, pack descriptor/state pairs, spill
words, hidden indirect-result and pack-input pointers, and allocation-context storage before
adding preserved registers and the frame record. Root context storage is two words; an incoming
context or hidden ABI pointer receives its own saved pointer word.

Every prepared pack call site owns a four-word descriptor containing its state pointer, immutable total
length, next callback, and residual-destruction callback. Its separate state object starts with a
segment cursor and stores fixed values or spread remaining-count/iterator pairs in source order
under their checked machine size and alignment. The target declares two stable functions for every
body-local pack and stores their relocated addresses in the descriptor. Fixed-segment next
callbacks construct the planned direct or caller-owned `Optional<T>` result and advance the cursor;
fixed residual callbacks first make the state consumed, then call ordinary generated destruction
functions for unconsumed values in reverse order. Spread callbacks invoke the frozen direct next
function through its unique callable ABI. They stage direct or caller-owned `Optional<Item>`
results, copy readonly referents when required, decrement the exact remaining count, skip exhausted
segments, and destroy iterator state before advancing. A contradictory early `none` reaches the
compiler-owned exact-size trap. A literal body therefore consumes one stable descriptor ABI
without erasing the ownership layout of each caller's pack. `MachineCallPack::Forwarded` has no
body-local `MachinePackId`: ARM64 validates the caller and callee pack ABI contracts, loads the
saved incoming descriptor pointer, and places that exact pointer in the callee's hidden lane.

The fixed-frame planner reserves the maximum outgoing stack-argument area at the post-prologue
stack pointer, places selector and allocator objects in stable insertion order, preserves requested
`x19`-`x28` registers in numeric order, and ends with the canonical saved `x29`/`x30` frame record.
It rejects invalid alignment, non-callee-saved preservation, and every size or offset overflow.
Zero-sized objects retain an aligned address without consuming bytes. The completed frame size is
always a multiple of the 16-byte call-boundary alignment.

Whole-program code and data use distinct dense `Arm64FunctionId` and `Arm64DataId` domains. Function
bodies may emit typed function-branch and data-address fixups, but the function-local code builder
never guesses a global displacement. `Arm64ProgramBuilder` lays functions out in declaration order,
rejects missing or duplicate definitions, validates every target, and resolves direct and tail
branches only after all text offsets are stable. Read-only data is aligned in deterministic input
order. Its address materialization remains one typed `adrp`/`add` pair until the Mach-O writer
supplies final section virtual addresses; the shared relocation method validates page arithmetic,
the signed 21-bit page displacement, the 12-bit page offset, and both instruction locations before
returning relocated text.

Frame instruction materialization consumes only `Arm64FrameLayout`. It emits the stack adjustment,
saved `x29`/`x30` record, sorted callee saves, frame-pointer establishment, exact reverse restore,
and return. Small offsets use checked immediate or scaled load/store forms. Larger frame sizes and
distant slots materialize their full 64-bit offset in the ABI-reserved `x16` scratch register and
use the ARM64 extended-register stack-pointer form. Frame size therefore has no accidental
12-bit-immediate ceiling, and the general allocator never competes for the scratch register.

`Arm64SelectedFunction` is the consuming boundary between machine operations and physical code.
Its register operands are either allocated virtual lanes or explicit ABI registers; memory
operands name checked frame objects, outgoing/incoming offsets, or selected runtime bases. Its
blocks retain only machine CFG identities and target-selected instructions. The executable slice
covers integer and boolean constants, checked static/dynamic addresses and scalar loads/stores,
integer and boolean operations,
lossless integer widening with source-signedness-preserving extension,
raw-value comparison, representation-exact readonly-borrow comparison, direct scalar call
transport, indirect caller-owned aggregate transport, local branches, value and stored-tag
switches, direct returns, traps, and process exit. Signed narrow loads
have an explicit sign-extending instruction form, so stored scalar width never loses signed
interpretation. Every other machine operation is a typed selection error rather than a retained
passthrough node.

Each selected CFG edge owns parallel-copy contracts for direct lanes and memory values. Copies are
materialized only after a conditional or switch edge has been chosen. The direct resolver removes
identities, schedules acyclic dependencies from leaves to roots, and saves one source in the
ABI-reserved boundary register when it encounters a register/spill cycle. The memory resolver
applies the same parallel-assignment rule to exact frame objects and uses the function's planned
maximum-size edge temporary only to break a cycle. Sequential copy order is never used as block
parameter semantics.

Value switches retain one or two selected subject lanes and compare the complete low/high `u128`
case representation before branching. Stored-tag switches load the exact byte at the
layout-provided tag offset through `Arm64SelectedAddressPlan`; they do not reproduce enum or outcome
layout. Cases and fallbacks carry the same edge object as ordinary branches, so register, spill,
and memory block parameters have one successor transport contract.

Static text is selected as one data-address lane plus one byte-length lane at the offsets already
owned by `MachineLayoutStore`. Selected code retains `MachineDataId`; whole-program lowering builds
one dense `MachineDataId` to `Arm64DataId` map before materialization, and the existing section
relocation authority resolves the address. Direct one- and two-word values share one raw-lane
transport across parameter registers, stack arguments, local storage, block edges, and result
registers. The register window never reopens after a value spills to outgoing stack, exactly as
specified by `MachineCallableAbi`. Direct lanes whose semantic byte width is not one native
load/store width are assembled from exact in-bounds fragments. Selection does not widen a 3-, 5-,
6-, or 7-byte tail into an adjacent frame object.

Aggregate selection consumes `MachineAggregate` as an exact byte-write recipe. It zero-initializes
the entire representation, then applies layout-owned tag and value offsets. A memory-backed result
is assembled directly in its value object. A direct result uses the function's single maximum-size
construction object and is loaded into its allocated lanes immediately, so staging lifetime does
not become value lifetime. Stack-to-stack movement has one exact non-overlapping copy instruction;
materialization validates both ranges and rejects overlap instead of silently selecting forward or
backward copy semantics.

`Arm64SelectedAddressPlan` normalizes every dense machine address before block selection. A path
with only stack and constant-offset steps becomes one bounds-checked frame address. Pointer, view,
dereference, and runtime-index paths retain only selected value registers plus layout-owned
offsets, bounds, and strides. Materialization evaluates them into the reserved address-boundary
lane, performs unsigned bounds checks, and traps an invalid index before access. Ordinary place
loads/stores/address formation and structural fixed/view index borrows call this same evaluator.
The selected memory-address type therefore replaces stack-only operation variants rather than
adding projected-memory exceptions.

A terminal `str` or slice place is a dynamic view address, not fictional stored bytes. Its address
extent retains the active pointer and length while stored addresses retain exact size and
alignment. Loads, stores, pack state, and destruction require a stored extent; borrowing a view
transports both lanes through the same address evaluator. This distinction keeps unsized types out
of the stored-layout table without special-casing slice or string reborrows in MIR.

Indirect call transport consumes `MachineValueClass::Indirect` without introducing a second ABI
planner. Each caller passes the address of its memory-backed argument value and provides a
memory-backed result object through the target's dedicated indirect-result lane. At entry, the
callee first saves that result pointer and every register-carried parameter before performing any
address materialization. It then copies indirect inputs into its own parameter objects, preserving
callee-local ownership and preventing an ABI pointer from becoming language-visible storage.
Return copies the exact stored-layout byte count into the caller object. The saved result pointer
survives nested calls; register-window closure carries later indirect pointers through the same
ordered outgoing and incoming stack slots as direct values.

Allocation-context selection is likewise independent from individual callees. A program or test
root initializes its compiler-owned two-word default header. A context-consuming callable saves
the incoming `x9` pointer in its frame before any call can clobber it. Each inherited call reloads
that saved pointer or forms the root-header address; each explicit call resolves its checked
machine address before ordinary argument lanes are staged and then writes the same hidden register.
The first closed primitive expansions read the state and kind words through the primitive's normal
argument/result ABI plan. Primitive selection dispatches only on the retained `PrimitiveRole` and
never on a module path or declaration spelling.

Pure pointer and view roles use this same selector. Representation-preserving pointer conversions,
raw view construction, and string-to-byte views leave the already staged ABI lanes intact. View
length and pointer observations select one lane, while unchecked string subviews adjust the pointer
and replace the length without reopening source types. Pointee size and alignment are constants
from `MachineLayoutStore`; the layout closure therefore includes concrete primitive type arguments
even when a pointee never appears as a by-value MIR value.

Memory transfer roles retain the same boundary. One machine-owned classifier determines zero,
direct, or indirect transport from the completed layout and selected target. Runtime-sized string
and pointer copies lower to a zero-safe forward byte loop. Indexed byte stores and generic
`store/take<T>` first form the runtime base, then reuse the existing exact-width lane operations or
the fixed-size indirect copy path. Target selection therefore does not repeat the ABI's direct-size
limit, invent generic value registers, or widen partial lanes.

Darwin system roles have their own closed selected instruction vocabulary. A generic syscall moves
the Nocter call's first direct lane into Darwin's syscall-number register, shifts its remaining
direct lanes into the platform argument window, emits one supervisor call, and converts the carry
flag into the declared `{ value, errno }` pair. The same exit emitter serves compiler-generated
root termination and the process-exit primitive. Trap, unreachable, and allocation abort emit
distinct break immediates, preserving a stable compiler-owned termination reason without a runtime
symbol or primitive-specific call convention.

Machine operations themselves state whether their target lowering may cross an ordinary callable
boundary. Value allocation consumes that fact for calls, direct user destruction, and pack
callbacks, preserving every live direct lane in a callee-saved register or spill. User-drop
selection validates the target's one-register readwrite-borrow input, completion result, and absent
pack/stack transport, then forms the checked place address in `x0`, materializes an inherited
allocation context only when required, and emits the common direct-call instruction. Conditional
ownership has one dense frame-byte representation: entry initialization, state updates, and
drop-flag branches all address the same `MachineDropFlagId` projection.

The materializer receives the completed selected function, value plan, frame, and dense function
mapping. It resolves virtual lanes, inserts spill traffic through the shared large-offset frame
access authority, binds local labels, and emits concrete code. `Arm64Program::lower_machine` then
uses the existing whole-program builder for functions and static data. The cross-crate
`nocter-conformance` suite compiles constant, scalar call/arithmetic, control, structural
comparison, and narrow signed-value processes from source through Mach-O and executes the images
natively on ARM64 macOS. A value-producing conditional case crosses machine block parameters and
their native parallel-copy edges. Static text and two-word view cases cross local storage, results,
the register window, and outgoing stack arguments. Direct 3-byte and two-lane aggregates plus a
24-byte memory aggregate cross construction, local storage, field projection, indirect arguments,
nested caller-owned results, and native execution. A ninth large argument also crosses the closed
register window through the outgoing stack area.
Dynamic fixed-array place loads/stores and structural indexing over a borrowed fixed array and
`&str` view exercise the shared checked address evaluator.
Pointer identity, pointee byte layout, raw string subviews, and slice/string view observations cross
their ordinary primitive ABI and execute natively as one combined conformance case.
Runtime string/pointer copies, indexed byte storage, a 24-byte generic store, and direct/indirect
generic takes likewise cross source, machine ABI, native selection, and Mach-O execution.
Generic syscall success and failure plus primitive process exit also execute across this complete
pipeline. Trap, unreachable, and allocation abort cross materialization but are not executed by the
test runner.
User drop calls and conditional drop flags execute natively; the case proves both skipped cleanup
for branch-local uninitialized storage and reverse cleanup for initialized temporaries.
Optional tag selection and both arms of a 24-byte value-producing conditional execute natively;
the memory edge scheduler separately proves identity removal, acyclic ordering, and cycle breaking.
The constant case also checks byte-for-byte determinism.

The separate `nocter-macho` crate receives only a completed `Arm64Program`. It owns the page-zero,
text, read-only-data, and link-edit layout; ARM64 section relocation; native entry metadata;
dyld/libSystem load commands; deterministic content-derived UUID; and complete byte serialization.
It emits the SHA-256 ad-hoc code directory and page hashes itself, so building an executable never
invokes an assembler, linker, or signing tool. A target test writes the generated image and proves
that macOS executes it with the requested status. The writer does not understand MIR, types,
declarations, primitives, or source linkage names. Runtime and system interfaces remain explicit
target-owned primitive expansions rather than backend spelling conventions.

## Validation and Determinism

Every lowering boundary has a consuming builder and an immutable validated result. Validation is
structural and source-independent:

- all referenced types have one completed stored layout
- every projection offset belongs to the referenced layout entry
- every call uses the callee's exact transport plan
- every context-consuming call has one fixed inherited or explicit allocation-context source
- every CFG edge supplies the destination's typed inputs
- every stack object has checked size, alignment, and lifetime
- every linkage key is unique and every referenced function or datum exists
- physical frame offsets satisfy target alignment and cannot overlap live storage
- encoded branch and relocation targets resolve before image serialization

Ordered maps and dense arenas provide canonical iteration. Hash iteration, filesystem order,
source discovery order, and first-use traversal order cannot affect layout, linkage, instruction
order, section order, or final bytes.

## Current Implementation State

The executable concrete-representation closure, immutable ARM64-Darwin stored-layout authority,
callable ABI planner, semantic linkage and static-data inventories, and the first complete
MachineProgram ownership spine are implemented. Conformance tests cover specialized generic struct
and enum members, scalar and pointer sizes, views and the one-word error handle, enum and recursive
outcome layout, `void!`, ordinary and zero-sized fixed arrays, closure capture order, exact opaque
witness representation, register-window closure, aligned stack placement, direct and indirect
results, the compiler-owned argument-pack lane, ordered test roots, static-text deduplication, dense
scalar/control/direct-call lowering, checked fixed-array indexing, layout-shared field access,
outcome tag control, explicit completion values, user destruction, region lifetime operations, and
process error reporting. Standard primitive calls also carry ordinary ABI plans and closed roles.
Argument packs now have dense identities, closed fixed/spread segments, explicit consumer
operations, and generated residual-cleanup function targets.
The completed allocation-context fixed point marks roots, context-independent callables, and
incoming-context callables in a dense function table. It follows inherited calls and hidden pack
callbacks or destruction, while explicit allocation selections terminate propagation into the
caller. Each machine function now also owns validated operation dependencies, typed CFG edges,
block liveness, and exact operation `live_after` sets. Tests cover call-surviving values, dynamic
index dependencies hidden in addresses, and edge-defined join parameters.

The typed ARM64 register, instruction-encoding, local-label, conditional-branch relaxation, ABI
register-role, fixed-frame placement, whole-program text/data ownership, and typed fixup foundations
are implemented. Fixed-frame prologue and epilogue materialization is also complete for both direct
and distant offsets. Deterministic Mach-O section placement, load commands, relocation, UUID,
ad-hoc signing, and executable serialization are implemented and pass a native execution test.
A deterministic virtual-register live-range builder and linear-scan allocator now reuse expired
caller-saved registers, restrict call-crossing ranges to callee-saved registers, record required
preservation, and assign dense spills under pressure. The machine-driven value plan classifies
zero, one-word, two-word, and memory values and feeds exact CFG call-survival facts into that
allocator. Complete function frames now place memory values, packs, spills, hidden ABI state, and
preserved registers through one deterministic authority. The selected and spill-materialized
scalar slice now carries simple stack storage, signed/unsigned scalar interpretation, arithmetic,
readonly-borrow comparison, scalar direct-call transport, control, return, and exit across the
signed Mach-O boundary and executes natively. Direct block-parameter lanes now cross typed CFG
edges through cycle-safe register/spill parallel copies. Layout-owned aggregate construction and
exact memory-value load/store are complete for direct and memory-backed values, including
deterministically initialized padding. Checked projected/dynamic memory and structural fixed/view
index borrows share one selected address evaluator. Direct and indirect callable transport are
complete, including caller-owned large results, callee-owned large parameters, nested calls, and
stack arguments after the register window closes. Root/incoming/explicit allocation-context
transport, current-context reads, pure pointer/view primitives, byte/value transfer primitives,
Darwin syscalls, process exit, trap, unreachable, allocation abort, direct user destruction, and
conditional drop flags, value and stored-tag switches, and cycle-safe direct/memory block-parameter
transport are complete. Concrete destruction now interns exact pointer and pack plans as ordinary
generated machine functions; recursive structs, active enum payloads, reverse fixed arrays, empty
pointer plans, and hidden allocation-context propagation execute natively through pointer calls.
Fixed argument packs now initialize and transfer their four-word descriptors, execute consuming-next
through the exact result ABI, and destroy unconsumed elements through those generated functions.
Spread packs execute the same native descriptor ABI for direct and copied-borrow contributions,
including direct and indirect optional transport, empty-segment advancement, inherited allocation
contexts, and iterator cleanup. Lexical regions now use a target-owned five-word frame resource:
ordinary context header, independent mapping-list head, and retained parent header. Calls and
authored destruction select the active resource statically, CFG validation enforces nested
lifetime balance, and release walks the mapping list through the target syscall boundary.
Test roots now lower to declaration-order `Arm64TestExecutable` values. Their immutable code and
data payload is materialized once and shared, while every case owns a distinct native entry for an
independent process image. The machine boundary closes test names to owned UTF-8 metadata and
dense `MachineTestId` values before target lowering. Fallible roots traverse the runtime-contract
error node as root code plus outer-to-inner messages; a target-owned frame buffer supplies fixed
punctuation without allocation, and failed stderr writes do not alter the required exit status.
The root then releases dynamic nodes iteratively. `new_error` and context attachment snapshot
source text into independently owned nodes through ordinary direct one-word result ABI. The
prebuilt allocation-failure leaf lives in immutable program data.

Process-entry state is complete. The root captures `argc`, `argv`, and `envp` before allocation
initialization can use platform argument registers, counts the null-terminated environment vector
once, and passes the four-word immutable context through fixed `x10` only when the machine process
plan requires it. Argument and environment indexes trap before an out-of-bounds load; successful
queries produce program-lifetime pointer/length views. Native conformance supplies an argument and
a controlled environment across a nested ordinary call.

The native I/O boundary is complete without operation-specific backend roles. The closed primitive
inventory exposes the existing generic syscall result only; the seven redundant open, read, write,
and close roles were removed rather than implemented as compatibility shims. `std/io` now owns
Darwin constants, interrupted-operation retry, partial and zero-progress writes, count validation,
errno mapping, NUL-path validation, and close-once policy in ordinary source. Native conformance
uses the generic boundary to write an exact byte sequence and to open, read, validate, and close a
temporary file.
