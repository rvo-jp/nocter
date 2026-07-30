# ABI and Layout

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Nocter ABI v0

Adopted: Nocter uses its own internal ABI for Nocter functions and primitives.

Nocter ABI v0 is defined only for the initial `arm64-darwin` target. It is not a C ABI, and it does not guarantee binary compatibility with C, Swift, Objective-C, or the platform dynamic linker ABI. Future C interop, if added, should use an explicit separate ABI form such as an `extern "c"`-style declaration.

Scope:

- target: `arm64-darwin`
- word size: 64-bit
- endian: little-endian
- stack alignment: 16 bytes at call boundaries
- compilation model: whole-program compilation in the initial implementation

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

Nocter ABI v0 uses the following ARM64 register roles:

```text
x0-x7    argument registers and direct return registers
x8       indirect return pointer
x9-x15   caller-saved scratch registers
x16-x17  compiler / primitive scratch registers
x18      reserved
x19-x28  callee-saved registers
x29      frame pointer
x30      link register
sp       stack pointer, 16-byte aligned at call boundaries
```

The caller must assume `x0-x17` may be clobbered by a call, except for return values. The callee must preserve `x19-x28`, `x29`, and stack alignment according to this ABI.

### ABI Values

ABI classification uses 64-bit words.

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

- Values of 16 bytes or less are passed directly.
- Direct values use `x0-x7` when enough consecutive argument registers remain.
- A two-word direct value must fit entirely in registers; otherwise the whole argument is passed on the stack.
- Values larger than 16 bytes are passed indirectly by pointer.
- Stack arguments are placed in ABI-sized slots and keep their natural alignment, with the stack 16-byte aligned at the call boundary.
- For indirect arguments, ownership and borrowing rules are still source-level Nocter rules. The pointer passing mechanism does not by itself transfer ownership.

Small integer arguments are extended to one ABI word. Unsigned integers are zero-extended. Signed integers are sign-extended. `bool` uses `0` for false and `1` for true; other bit patterns are invalid for a live `bool` value.

When a live `bool` or enum value can enter from a primitive or ABI boundary and the compiler cannot prove that its bit pattern or tag is valid, the required validation is an always-on safety check. The general safety-check policy is specified in [Control Flow](03-control-flow.md#safety-checks-and-build-modes).

### Returns

Return rules:

- `void` returns no value.
- `never` does not return to the caller.
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
- `copy struct` and ordinary `struct` use the same layout rules.
- `drop` presence does not change layout.

Field alignment follows the field's ABI type. Initial alignments:

```text
bool, u8, i8       align 1
u16, i16           align 2
u32, i32           align 4
u64, i64           align 8
usize, isize       align 8
pointer, borrow    align 8
```

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

- All enum values use a `u8` tag and may have at most 256 variants.
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
- Drop glue drops only the active payload. Multi-field payloads drop their
  fields according to ordinary aggregate drop order.
- No niche optimization is part of ABI v0.

### Built-In Error Layout

The built-in `error` payload is represented as two borrowed string slices:

```text
error:
  code:    &str
  message: &str
```

Layout:

```text
word 0: code.ptr
word 1: code.len
word 2: message.ptr
word 3: message.len
```

Rules:

- `error.code` and `error.message` are the user-facing fields of `error`.
- Both fields have type `&str`.
- The field order is `code`, then `message`.
- The built-in `error` type has size 32 bytes and alignment 8 on ABI v0.
- `error` does not own its string storage and has no drop member.
- `error` is copyable because both fields are copyable borrowed views.
- An `error` value carries borrow-like provenance from both `&str` fields.
- Returning or storing an `error` must satisfy the same borrow-like escape rules
  as any aggregate containing `&str`.
- The compiler-generated entry wrapper may report `error.code` and
  `error.message` directly from these fields without allocating or calling a
  fallible standard-library API.

### Optional Layout

`T?` is represented as a tag plus a payload.

```text
T?:
  tag 0 = none
  tag 1 = value
```

Rules:

- The tag type is `u32`.
- The payload layout is the layout of `T`.
- The payload is live only when the tag is `1`.
- `none` has no live payload.
- Drop code drops the payload only when the tag is `1`.

No niche optimization is part of ABI v0. Even if a type has unused bit patterns, `T?` still uses the explicit tag representation.

### Fallible Layout

`T!` is represented as a tag plus a payload union.

```text
T!:
  tag 0 = success
  tag 1 = failure
```

Rules:

- The tag type is `u32`.
- Success payload layout is the layout of `T`.
- Failure payload layout is the layout of the built-in `error` type.
- The payload area is large enough and aligned enough for either payload.
- The payload is live only for the active tag.
- Drop code drops only the active payload.

Nocter source uses `return value` for success and `return error_value` for failure. ABI v0 does not reserve the identifier `success`; it defines only the binary tag meaning.

Composed optional and fallible values use the same layout rules recursively. For example, `T?!` is laid out as a fallible value whose success payload is the explicit-tag layout of `T?`.

### Drop ABI

Source syntax:

```nct
drop &+self {
    ...
}
```

ABI form:

```text
x0     &+Self
return void
```

Rules:

- `drop` is not fallible.
- `drop` must not return a value.
- The caller passes a readwrite borrow of the value being destroyed.
- After `drop` returns, the caller treats that value as dead.
- Drop glue for structs, fixed arrays, enums, optionals, and fallible values must
  follow active-field, initialized-element, and active-payload rules.

### Primitive ABI

`primitive` declarations use Nocter ABI at the Nocter call boundary.

```nct
pub(nocter) primitive syscall3(...): SyscallResult
pub(nocter) primitive trap(): never
```

Rules:

- After visibility and trusted-boundary restrictions have allowed the call, the source-level call to a primitive is type checked like a normal function call.
- The primitive's parameters and return type are lowered using Nocter ABI.
- The backend then lowers the primitive body to target-specific machine code or target-specific calling sequences.
- A primitive may internally use OS syscall conventions, but those conventions are not exposed as the source-level ABI.
- The compiler validates primitive declarations against either the target-independent core primitive set or the active target's closed primitive set.

### ABI Stability

ABI v0 is an internal compiler ABI. It is stable enough for the initial compiler, standard library, and primitive boundary, but it is not a public binary compatibility promise across compiler versions.

The ABI should not be changed casually. Changes require updating:

- code generation
- data layout
- type checking assumptions for layout-sensitive constructs
- primitive lowering
- target-gated standard-library internals
- tests for calls, returns, aggregate layout, optionals, fallible values, and drop glue
