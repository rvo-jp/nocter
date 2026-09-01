# ABI and Layout

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Nocter ABI

Nocter uses its own internal ABI for Nocter functions and primitives.

The current Nocter ABI is defined only for `arm64-darwin`. It is not a C ABI and does not guarantee binary compatibility with C, Swift, Objective-C, or the platform dynamic linker ABI. Future C interop, if added, requires an explicit separate ABI form.

Scope:

- target: `arm64-darwin`
- word size: 64-bit
- endian: little-endian
- stack alignment: 16 bytes at call boundaries
- compilation model: whole-program compilation

Rules:

- All ordinary Nocter functions use Nocter ABI.
- Associated functions and methods use Nocter ABI.
- `drop` functions use Nocter ABI.
- `primitive` declarations use Nocter ABI at the Nocter boundary.
- OS syscall ABI details stay inside target-specific primitive lowering.
- C ABI compatibility is not promised.
- ABI definitions are target-specific; future targets define their own Nocter ABI variant.
- Type aliases do not affect ABI. ABI classification always uses the alias target type.

### Registers

The Nocter ABI uses the following ARM64 register roles:

```text
x0-x7    argument registers and direct return registers
x8       indirect return pointer
x9       compiler-propagated allocation-context pointer
x10-x15  caller-saved registers available to virtual-register allocation
x16-x17  compiler / primitive scratch registers
x18      reserved
x19-x28  callee-saved registers
x29      frame pointer
x30      link register
sp       stack pointer, 16-byte aligned at call boundaries
```

The caller must assume `x0-x17` may be clobbered by a call, except for return values. The callee
must preserve `x19-x28`, `x29`, and stack alignment according to this ABI. General virtual-register
allocation uses `x10-x15` and `x19-x28`; fixed argument, result, context, compiler-scratch, and
platform-reserved lanes do not compete with virtual values.

`x9` is not an authored parameter. For a callable whose compiler-owned execution requirement needs
the current allocation context, it points to an opaque context header whose first word is allocator
state and whose second word is allocator kind. The ordinary argument window remains `x0-x7` and is
unchanged by this hidden lane. A root supplies the program-lifetime context. An inherited call
passes the caller's current pointer, while an explicit `using` call supplies the selected allocator
or region-context address for that call. The caller must retain or rematerialize its own current
pointer across a call because `x9` is caller-saved. A callable without the execution requirement
cannot read the lane.

A lexical region is a non-movable target-owned frame resource. Its first two words are the same
state/kind header exposed through `x9`; its state points to a private allocation-list head in the
same frame object, its kind is the standard region kind, and it retains the selected parent's
state/kind header after that head. The source-level `AllocationContext` declaration supplies the
two-word header contract but does not set the complete physical size of this compiler-owned
resource. Region-backed mappings start with a private
previous-mapping pointer and mapping-byte count. Normal region release advances the head before
unmapping each entry, then leaves the list empty. This representation is part of the
`arm64-darwin` Nocter ABI rather than an inferred property of current standard-library source.

### ABI Values

ABI classification uses 64-bit words.

Zero-word values:

- any sized type whose computed size is `0`

A zero-word value still exists for source ownership, initialization, destruction, and evaluation
order. Its transport consumes no argument register, return register, or stack slot.
`void` is not a zero-word value and has no standalone stored layout; it denotes completion without
a value.

Creating a borrow of a zero-sized place still produces the ordinary one-word borrow ABI value. Its
address is non-null and satisfies the pointee alignment, but different logical places may share
that address. Machine storage and pointer equality do not define source place identity.

One-word values:

- `bool`
- integer types up to 64 bits
- raw pointers
- `&T`
- `&+T`

Two-word values:

- `&str`
- `&[T]`
- `&+[T]`

Layout of these view types:

```text
word 0: ptr
word 1: len
```

`ptr` points to the first byte or element. `len` is a `usize` count. For `&str`, `len` is the number of UTF-8 bytes.

The unsized data forms `str` and `[T]` do not have a standalone by-value ABI. They are passed and returned only through an indirection such as `&str`, `&[T]`, `&+[T]`, or an owning standard-library type such as `String` or `Vec<T>`.

Values larger than 16 bytes are classified as indirect values.

### Arguments

Arguments are assigned left to right.

Rules:

- A zero-sized argument expression is evaluated and ownership is transferred normally, but it
  consumes no argument register or stack slot.
- Values of 16 bytes or less are passed directly.
- Direct values use `x0-x7` when enough consecutive argument registers remain.
- A two-word direct value must fit entirely in registers; otherwise the whole argument is passed on the stack.
- Values larger than 16 bytes are passed indirectly by pointer. That pointer is one ABI word and
  participates in the same left-to-right register-or-stack assignment as a direct word.
- The first non-zero-word argument whose complete transport does not fit in the remaining argument
  registers closes the register window. That argument and every later non-zero-word argument are
  passed on the stack; an otherwise unused register is not reused by a later smaller argument.
- Zero-word arguments neither consume a location nor close an open register window. After the
  register window has closed, they still consume no stack slot.
- Stack arguments are placed in left-to-right order. A direct argument reserves its one- or
  two-word transport size. An indirect argument reserves one pointer word. Each slot starts at the
  next offset aligned to at least one ABI word and to the stored value's natural alignment; padding
  belongs to neither adjacent argument. The complete outgoing argument area is rounded up so that
  the stack remains 16-byte aligned at the call boundary.
- For indirect arguments, ownership and borrowing rules are still source-level Nocter rules. The pointer passing mechanism does not by itself transfer ownership.

Small integer arguments are extended to one ABI word. Unsigned integers are zero-extended. Signed integers are sign-extended. `bool` uses `0` for false and `1` for true; other bit patterns are invalid for a live `bool` value.

When a live `bool`, enum, optional, or fallible value can enter from a primitive or ABI boundary
and the compiler cannot prove that its bit pattern or tag is valid, the required validation is an
always-on safety check. Validation recursively follows only active aggregate and outcome payloads.
The general safety-check policy is specified in
[Control Flow](03-control-flow.md#safety-checks-and-build-modes).

### Returns

Return rules:

- `void` returns no value.
- `never` does not return to the caller.
- A zero-sized return expression is evaluated and transfers one logical initialized value to the
  caller, but consumes no return register and requires no indirect return storage.
- Values of 16 bytes or less return directly in `x0` and `x1`.
- Values larger than 16 bytes return indirectly.

For indirect returns, the caller allocates the destination storage and passes its pointer in `x8`. The callee writes the result into that storage and returns normally. The callee does not allocate the return storage.

### Struct Layout

`struct` layout is deterministic.

Rules:

- Fields are laid out in declaration order.
- The compiler does not reorder fields.
- Each field is placed at the next offset satisfying that field's alignment.
- The struct alignment is the maximum alignment of its fields.
- The final struct size is rounded up to the struct alignment.
- A struct with zero fields has size `0` and alignment `1`.
- `copy struct` and ordinary `struct` use the same layout rules.
- `drop` presence does not change layout.

Field size and alignment follow the field's stored ABI layout. Current target scalar and view
layouts:

| Type | Size | Alignment |
| --- | ---: | ---: |
| `bool`, `u8`, `i8` | 1 | 1 |
| `u16`, `i16` | 2 | 2 |
| `u32`, `i32` | 4 | 4 |
| `u64`, `i64` | 8 | 8 |
| `usize`, `isize` | 8 | 8 |
| `*T`, `&T`, `&+T` | 8 | 8 |
| `&str`, `&[T]`, `&+[T]` | 16 | 8 |

The stored size of `bool` is one byte, and its only valid live representations are byte values `0`
and `1`. Small scalar argument and return extension to a 64-bit ABI word does not change stored
size or aggregate field layout.

Every signed N-bit integer uses N-bit two's-complement representation. Signed and unsigned types
of the same width occupy the same number of bytes; they interpret those bits with their respective
signed or unsigned value ranges. Multi-byte integers, `usize`, `isize`, pointers, and view words
are stored least-significant byte first under the target's little-endian rule. Signed ABI-word
extension replicates the two's-complement sign bit.

Aggregates use their computed aggregate alignment.

### Fixed-Size Array Layout

`[T; N]` stores `N` elements of `T` contiguously in index order.

Rules:

- The element type `T` must have a sized ABI layout.
- The array alignment is the element alignment.
- The element stride is the element size rounded up to the element alignment.
- Element `i` starts at byte offset `i * stride`.
- The array size is `stride * N`.
- `[T; 0]` has size `0` and the same alignment as `T`.
- When `T` is zero-sized, every logical element has the same byte offset and the array size is
  `0` for every `N`. Bounds, initialization, ownership, evaluation, and per-element destruction
  still use the declared logical element count.
- Array ABI classification uses the total array size and alignment, following the
  ordinary direct-versus-indirect rules.
- Drop glue for a fixed array drops initialized owned elements in reverse index
  order.
- Fixed arrays of copy element types are copyable. Fixed arrays of move-only
  element types are move-only.

### Enum Layout

Payloadless enum values are represented as a single tag byte.

```text
payloadless enum = u8 tag
```

Payload-carrying enum values use the same tag byte followed by a payload union.

```text
enum = tag + payload union
```

Rules:

- Every enum declares between 1 and 256 variants, inclusive, and all enum values use a `u8` tag.
- Variant tag values are assigned by declaration order starting at `0`.
- For a payload-carrying enum, the tag is stored at byte offset `0`.
- The payload union starts at the next offset after the tag that satisfies the
  maximum payload alignment of all variants.
- The enum alignment is the maximum of `1` and the maximum payload alignment.
- The enum size is the payload-union end offset rounded up to the enum
  alignment.
- A payloadless variant has no live payload.
- A variant with one payload field uses that field's layout as its payload
  layout.
- A variant with multiple payload fields uses an anonymous struct payload with
  fields laid out in declaration order using normal struct layout.
- The payload union size is the maximum payload size across variants.
- Payload and padding bytes outside the active payload have unspecified
  contents and must not be inspected by safe source code.
- Drop glue invokes the enum's own drop declaration first when present, then drops only the active
  payload. Multi-field payloads drop their fields according to ordinary aggregate drop order.
  Recursive payload drop glue includes fixed arrays, whose initialized elements drop in reverse
  index order.
- The ABI does not use niche optimization.

### Stored Optional and Fallible Layout

The stored representation is recursive and distinct from callable register
passing. Optional and fallible layers share one binary tagged-union layout:

| Layer | `u8` tag `0`: primary | `u8` tag `1`: alternate |
| --- | --- | --- |
| `T?` | present `T` payload | `none`, no payload |
| `T!` | successful `T` payload | built-in `error` payload |

For `void!`, tag `0` has no payload and denotes successful completion. Tag `1` carries `error`.
The union alignment and size therefore come only from the failure payload.
When a `void` expression contextually constructs `void!`, tag `0` becomes live only after the
expression completes normally. A terminating expression produces no outcome value.

The tag is stored at byte offset `0`. Let the layer alignment be the maximum of `1` and the
alignment of every payload-carrying branch. The payload union begins at the first offset after the
tag that satisfies that alignment. Its size is the maximum branch-payload size, treating optional
absence as size zero. The layer size is the end of that union rounded up to the layer alignment.
Nested supported outcomes apply this complete rule recursively.

Only tag values `0` and `1` are valid for a live outcome. Optional absence has no initialized
payload. Fallible failure initializes `error` instead of the success payload. The ABI does not use
niche optimization and does not widen an outcome tag to `u32` or `usize` for storage.

Inactive union bytes and padding have unspecified contents. Copy, move, drop, and control flow may
inspect the tag but must not read or destroy an inactive payload. Callable entry and return lowering
explicitly bridge this one stored layout and the target-specific register ABI.

Copyability follows the source-type rule rather than the active runtime tag. `T?` is copyable
exactly when `T` is copyable. Every `T!`, including `void!`, is move-only because failure carries
an owned `error`; mixed outcomes containing a fallible layer are therefore move-only. A move-only
outcome remains move-only even while success or absence is active.

Nocter source uses contextual outcome injection to construct tags. The ABI does not reserve the
identifiers `value`, `success`, `none`, or `failure`; the table defines binary representation, not
additional source names. For example, `T?!` is a fallible layer whose tag-`0` success payload is
the recursively laid-out `T?` layer.

### Built-In Error Layout

The built-in `error` payload is one pointer-sized handle to an immutable runtime node:

```text
error: *runtime_error_node
```

Layout:

```text
word 0: node address
```

Rules:

- The built-in `error` type has size 8 bytes and alignment 8 on the current ABI.
- A dynamic leaf owns snapshots of its code and message. A dynamic context owns its message and a
  cause handle. A static leaf may point into immutable image data and requires no release.
- Dynamic node destruction iteratively releases the complete cause chain. It does not recurse on
  the native call stack.
- `error` is move-only. Moving transfers the handle; copying it would duplicate ownership and is
  invalid.
- `code()`, `message()`, and reporting inspect the node through the handle. The source type exposes
  no representation fields.
- The compiler-generated entry wrapper writes the root code, `: `, outer-to-inner context and leaf
  messages separated by `: `, and a trailing newline. Reporting allocates no storage. Each write is
  best-effort; a reporting failure is ignored and the wrapper still releases the error and exits
  with status `1`.
- The static `std.mem.out_of_memory` leaf is available without dynamic allocation, preventing
  recursive allocation failure while constructing the failure value.

### Drop ABI

Source syntax:

```nct
drop Resource(&+self) {
    ...
}
```

ABI form:

```text
x0     &+Self
return void
```

Rules:

- destruction is not fallible.
- a drop body must not return a value.
- The caller passes a readwrite borrow of the value being destroyed.
- After `drop` returns, the caller treats that value as dead.
- Copyable types have no source-defined drop ABI entry. A drop declaration never changes the
  target type's copyability or stored layout.
- A source-defined drop entry always receives a complete initialized `Self`. Partial field moves
  are rejected for every enclosing struct that owns one, so ABI lowering needs no partial-self
  calling convention or skip flag.
- Drop glue for structs, fixed arrays, enums, optionals, and fallible values must
  follow active-field, initialized-element, and active-payload rules.

### Primitive ABI

`primitive` declarations use Nocter ABI at the Nocter call boundary.

```nct
pub(/) noalloc primitive func syscall3(...): SyscallResult
pub(/) noalloc primitive func trap(): never
```

Rules:

- After visibility and trusted-boundary restrictions have allowed the call, the source-level call to a primitive is type checked like a normal function call.
- The primitive's parameters and return type are lowered using Nocter ABI.
- The backend then lowers the primitive body to target-specific machine code or target-specific calling sequences.
- A primitive may internally use OS syscall conventions, but those conventions are not exposed as the source-level ABI.
- The compiler validates primitive declarations against either the target-independent core primitive set or the active target's closed primitive set.

### Static Opaque Result ABI

`some Interface` has exactly the ABI, layout, alignment, return transport, and destruction behavior
of its statically selected concrete witness. It adds no tag, pointer, vtable, metadata field,
boxing, or allocation. Optional and fallible wrappers apply their normal representation to the
witness payload. The witness is available to ABI and lowering only; source-level type checking and
tooling retain the declaration-scoped opaque identity.

### ABI Stability

The Nocter ABI is internal to the compiler. It is not a public binary-compatibility promise across compiler versions.

The ABI should not be changed casually. Changes require updating:

- code generation
- data layout
- type checking assumptions for layout-sensitive constructs
- primitive lowering
- target-gated standard-library internals
- tests for calls, returns, aggregate layout, optionals, fallible values, and drop glue
