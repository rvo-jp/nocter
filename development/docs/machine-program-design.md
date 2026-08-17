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
future machine-program pack representation must implement length, consuming next, and residual
destruction through that pointer while retaining the MIR-owned element and optional-result types.

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
address, SSA-value, operation, and basic-block domains. MIR identities are translated once while the
program is built and are absent from the immutable result. Every address starts from an abstract
stack object, pointer value, or two-word view and then uses only byte offsets, dereferences, and
checked index steps. Fixed arrays retain their declared bound and layout-owned stride. Slice and
string indexes retain the length from the current view. Field, capture, variant-payload, outcome,
and opaque-witness identities are consumed while those steps are constructed.

Function entries carry the single callable ABI object owned by `MachineAbiPlan`; direct-call
operations name the target machine function and therefore cannot invent a second call contract.
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

## ARM64 and Mach-O Boundaries

ARM64 lowering receives only validated machine operations and completed transport plans. It owns
register allocation, spills, callee-saved preservation, frame offsets, instruction selection,
literal pools, and branch relaxation. Those decisions cannot change Nocter type layout or ABI
classification.

The Mach-O writer receives encoded code, read-only data, relocation/fixup records, and deterministic
linkage metadata. It owns section order, load commands, symbol/string tables, entry metadata, and
byte serialization. It does not understand MIR, types, declarations, or primitives. Runtime and
system interfaces are explicit machine-program imports or target-owned primitive expansions rather
than undeclared backend name conventions.

## Validation and Determinism

Every lowering boundary has a consuming builder and an immutable validated result. Validation is
structural and source-independent:

- all referenced types have one completed stored layout
- every projection offset belongs to the referenced layout entry
- every call uses the callee's exact transport plan
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
process error reporting.

Primitive calls, literal-pack descriptors, ARM64 instruction lowering, and Mach-O serialization are
the remaining Phase 5 implementation areas.
