# Machine Program and Native Target Design

This document assigns implementation responsibility for v0.14.0 Phase 5. It does not define
language behavior. Stored layout, argument and result transport, primitive behavior, and executable
image requirements remain owned by the public specification.

## Boundaries

Phase 5 extends the one-way compiler pipeline with four distinct authorities:

```text
MirProgram
  -> MachineLayoutStore
  -> MachineProgram
  -> Arm64Program
  -> MachOImage
```

`MachineLayoutStore` computes immutable stored representations from concrete MIR types and the ABI
identity already selected by `ToolchainSnapshot`. `MachineProgram` will own ABI-classified values,
stack objects, calls, control flow, and deterministic linkage without assigning physical ARM64
registers. `Arm64Program` will own instruction selection, physical registers, frame layout, branch
fixups, and encoded sections. `MachOImage` will own file-format tables and bytes.

No boundary may inspect syntax, source paths, source names, generic requirements, conformance
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

Each stored layout records its byte size, byte alignment, representation class, and every offset a
later stage needs. This includes view pointer and length offsets, built-in error member offsets,
enum and outcome tag and payload offsets, nominal fields, variant payload parameters, fixed-array
stride, closure captures, and opaque witnesses. Downstream lowering must query this table; it may
not repeat padding arithmetic or embed an independently remembered offset.

The builder starts from function signatures and all local, place, value, pack, process-root, and
test-root types in `MirProgram`, then closes recursively over stored children. `void` and `never`
remain completion types without stored layouts. Borrows to `str` and slices are two-word views;
other raw pointers and borrows are one-word pointers. Unsized or symbolic by-value types,
incomplete representations, recursive values, invalid alignments, and arithmetic overflow are
typed compiler-integrity failures.

Stored layout and call transport remain separate. A type has exactly one stored layout even when
the ABI later carries it in zero, one, or two words or indirectly. This separation is required for
locals, projections, aggregate construction, inactive outcome payloads, and caller-provided result
storage to agree without encoding call-site policy in type layout.

## Machine Program Authority

`MachineProgram` will consume `MirProgram` together with its completed `MachineLayoutStore`. Its
schema must use distinct dense identities for functions, blocks, values, stack objects, constants,
and linkage entries. Typed operations will make loads, stores, copies, address projections,
integer operations, tags, direct calls, primitive calls, and exits explicit. Aggregate movement and
destruction must use the stored-layout authority and MIR's active-payload control flow rather than
introducing a second semantic aggregate model.

ABI lowering belongs at this boundary. Each callable input and result will receive one immutable
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

Sequence-literal bodies remain outside the ordinary argument list. Because executable validation
already forbids ordinary parameters on those bodies, their machine signature reserves one
compiler-owned pack-descriptor pointer lane without inventing a source type or variadic ABI. The
machine program assigns every caller-owned descriptor a body-local `MachinePackId`. Each descriptor
retains its exact element and optional-next types, total-length value, and ordered fixed or spread
segments. A spread contains only a machine address, remaining-count value, closed call target,
contribution mode, and residual destruction plan. The literal body exposes only explicit length,
consuming-next, and destroy operations over its hidden pointer.

Residual destruction is lowered independently from normal address operations because a partially
consumed pack owns storage whose active member is selected at runtime. Its closed recipe contains
machine-function drop targets, layout-owned byte offsets, strides, tags, sizes, and alignments. It
does not retain MIR places, source fields, variants, captures, parameters, or executable-item IDs.

Linkage identities derive from dense executable item and compiler-owned root identities, not
source spellings. Human-readable names may be retained as presentation metadata only after a
collision-free semantic linkage key exists. Primitive lowering dispatches on the closed
`PrimitiveRole` carried by MIR and never searches a standard-library symbol name.

`MachineLinkageTable` now closes one code identity for every executable item, process root, and
isolated test root from typed semantic owner keys. Test presentation names remain metadata, while
the separate root table preserves declaration order independently of key order. Static UTF-8 data
uses a distinct `MachineDataId` domain. Its table deduplicates complete byte strings and assigns IDs
in byte order, so function traversal and first-use order cannot alter data identity or later image
layout.

`MachineProgram` now owns a separate dense function domain and body-local stack-object, drop-flag,
address, SSA-value, operation, literal-pack, and basic-block domains. MIR identities are translated
once while the program is built and are absent from the immutable result. Every address starts from
an abstract stack object, pointer value, or two-word view and then uses only byte offsets,
dereferences, and checked index steps. Fixed arrays retain their declared bound and layout-owned
stride. Slice and string indexes retain the length from the current view. Field, capture,
variant-payload, outcome, and opaque-witness identities are consumed while those steps are
constructed.

Function entries carry the single callable ABI object owned by `MachineAbiPlan`. One
`MachineCallTarget` domain distinguishes direct machine functions from compiler-known primitives;
arguments, allocation context, and the optional hidden pack identity use one call representation.
Direct targets therefore cannot invent a second call contract, and new target kinds cannot create
a parallel argument-transport path.

`MachineAllocationPlan` is the single whole-program authority for the compiler-propagated current
allocation context. Roots establish the program-lifetime default. Callable requirements are the
least fixed point over current-context primitives, inherited direct calls, user-drop calls, spread
iterator callbacks, and every recursively nested residual-destruction plan. An explicit `using`
selection supplies the callee without making its caller context-dependent. The plan is complete
before ARM64 selection, so the target layer neither scans operations nor guesses whether to emit
the hidden context lane.

Constants, loads, address formation, stores, aggregate writes, scalar operations, integer
conversion, drop-flag control, block arguments, scalar and stored-tag switches, returns, process
exits, and direct calls have closed machine forms. Aggregate writes and tag switches use the same
stored-layout offsets and tag values as address projection. An explicit call allocation context is
an address identity rather than a retained MIR place. Unsupported MIR operations fail construction
explicitly instead of surviving inside a generic passthrough operation.

SSA identity and stored representation are intentionally separate. A `MachineValue` is classified
as stored bytes, successful completion, or divergence. Only stored values require size and
alignment; explicit MIR completion values therefore survive control and call wiring without
inventing a zero-byte layout for `void` or `never`. User-drop invocation names a machine function
and machine address. Region creation and release name machine values and stack objects, and process
error reporting names the exact machine error value. None retain MIR-local identities.

Standard primitive targets retain only their closed `PrimitiveRole`, concrete layout-key type
arguments, and exact concrete signature. The surrounding common call retains machine-value
arguments and allocation context. That signature is passed through the same callable ABI planner
as source functions. A primitive cannot introduce a private register convention, and ARM64
selection never looks up a primitive by module or declaration spelling.

Compiler-provided structural behavior does not cross a call boundary. Machine lowering replaces
primitive equality and ordering with comparisons that name exact scalar signedness or an enum tag
offset. Built-in indexing becomes one checked borrow operation containing its fixed bound or view
pointer/length offsets and element stride. Readwrite-to-readonly borrow weakening becomes an
explicit representation-preserving operation. The machine program therefore retains neither a
structural dispatch target nor a reason to reopen type selection.

Every `MachineFunction` owns one `MachineFunctionDataflow` derived while its body is closed.
Operation inputs include dependencies nested inside checked addresses, dynamic indexes, explicit
allocation addresses, and fixed or spread literal-pack segments; target lowering never needs a
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
`x10`-`x15` or `x19`-`x28`; a range live across a call may use only the latter or a spill slot.
`Arm64ValuePlan` first partitions machine values into omitted storage, one or two virtual word
lanes, or memory storage for values larger than the direct ABI limit. It uses machine operation
inputs and block liveness to extend deterministic intervals. Call crossing is marked directly from
the call operation's `live_after` set, excluding the call result, rather than inferred from a
flattened interval that may contain unrelated sibling blocks. Every direct lane then uses the
common linear-scan allocator; memory values remain explicit requests for later frame placement.
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
and enum members, scalar and pointer sizes, view and built-in error offsets, enum and recursive
outcome layout, `void!`, ordinary and zero-sized fixed arrays, closure capture order, exact opaque
witness representation, register-window closure, aligned stack placement, direct and indirect
results, the compiler-owned literal-pack lane, ordered test roots, static-text deduplication, dense
scalar/control/direct-call lowering, checked fixed-array indexing, layout-shared field access,
outcome tag control, explicit completion values, user destruction, region lifetime operations, and
process error reporting. Standard primitive calls also carry ordinary ABI plans and closed roles.
Literal packs now have dense identities, closed fixed/spread segments, explicit consumer
operations, and layout-owned residual destruction recipes.
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
allocator. Selected virtual instructions, fixed-frame placement for memory values and spills, and
spill materialization remain Phase 5 implementation areas.
