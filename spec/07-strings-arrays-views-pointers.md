# Strings, Arrays, Views, and Pointers

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Raw Pointers and Address API

Adopted: Nocter has raw pointer values, but raw pointer dereference is not available to general user code in the initial design.

Raw pointer type syntax:

```nct
*T
```

Examples:

```nct
*u8
*File
*void
```

`*T` is an address-carrying value. It is not an owning pointer and it is not a borrow.

Rules:

- `*T` is copyable.
- `*T` does not own the pointee.
- `*T` does not extend the lifetime of the pointee.
- `*T` does not prove the pointee is valid.
- `*T` does not grant read or write permission.
- `*T` has no `drop`.
- `*T` is non-null.
- If null is needed, use `*T?`.
- `*void` is allowed as an opaque raw pointer type.
- Raw pointer dereference has no user-facing escape hatch in v0. There is no `unsafe` block that enables it.

Raw pointer dereference is not part of the initial user-facing language.

Not adopted in v0:

```nct
*pointer
pointer.*
pointer.load()
pointer.store(value)
```

These operations may be reconsidered only if Nocter later adopts an explicit unsafe or trusted-code model. They are not enabled by v0's Nocter-home trusted boundary.

### `std/ptr`

Pointer and address conversion APIs live in `std/ptr`.

Initial public APIs:

```nct
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
```

Restricted API:

```nct
pub primitive from_addr<T>(address: usize): *T
```

`from_addr` is restricted to trusted modules inside the active Nocter home: the common `std/` and the active target overlay `std/`. User project modules must not call it. The declaration is public only so distributed standard-library modules can import it across module boundaries; the compiler enforces the caller restriction.

Rules:

- `addr` converts a raw pointer to a `usize` address.
- Pointer-to-integer conversion uses `std/ptr`'s `addr`, not `as usize`.
- `from_ref` creates a raw pointer from a readonly borrow.
- `from_ref_mut` creates a raw pointer from a readwrite borrow.
- `from_addr` creates a raw pointer from a `usize` address.
- `from_addr<T>(0)` is invalid for `*T`; use `none` for a `*T?` null-like absence.
- A raw pointer created from a borrow may outlive the borrow as a value, but using it as if it were valid is not guaranteed by the compiler.
- Because dereference is not available in v0, general user code can carry and pass raw pointers but cannot read or write through them.

Example:

```nct
import std/ptr as ptr

func address_of(value: &u8): usize {
    let pointer = ptr.from_ref(value)
    return ptr.addr(pointer)
}
```

### View Pointer APIs

`View<T>`, `WriteView<T>`, and `StringView` expose pointer and length methods.

```nct
impl View<T> {
    pub method (view: Self).ptr(): *T
    pub method (view: Self).len(): usize
}

impl WriteView<T> {
    pub method (view: Self).ptr(): *T
    pub method (view: Self).len(): usize
}

impl StringView {
    pub method (text: Self).ptr(): *u8
    pub method (text: Self).len(): usize
}
```

`ptr()` returns a raw pointer. It does not grant dereference permission.

Trusted standard-library implementation example:

```nct
import std/ptr as ptr
import std/os/macos as os

let bytes = text.bytes()
let result = os.syscall3(
    SYS_write,
    fd as usize,
    ptr.addr(bytes.ptr()),
    bytes.len(),
)
```

User project modules must not call syscall primitives directly.

### Pointer Intrinsics

The `std/ptr` functions above are target-independent core primitive declarations. They are separate from the active target's OS primitive set such as `std/os/macos`'s `syscall0`.

The compiler validates them by module path, name, and exact signature:

```text
std/ptr.addr
std/ptr.from_ref
std/ptr.from_ref_mut
std/ptr.from_addr
```

These core pointer primitives exist because address conversion and borrow-to-pointer conversion cannot be implemented in ordinary Nocter code. They do not make `print`, `exit`, `abort`, allocation, strings, buffers, or file APIs compiler primitives.

## Arrays and Views

Adopted: fixed-size arrays use `[T; N]`.

```nct
let header: [u8; 4] = [0x7F, 0x45, 0x4C, 0x46]
let numbers = [1, 2, 3] // [i32; 3]
```

Array literals use `[a, b, c]`.

Rules:

- If there is an expected `[T; N]` type, the literal is checked against that element type and length.
- Without an expected type, the compiler infers the element type from the elements.
- Integer-only array literals use `i32` unless context provides another integer type.
- The inferred length is part of the array type.

Owned growable memory is represented by standard-library types such as `Buffer<T>`. `Buffer<T>` is not a compiler builtin.

```nct
var bytes = try Buffer<u8>.with_capacity(allocator, 4096)
try bytes.push(10)

let read: View<u8> = bytes.view()
let write: WriteView<u8> = bytes.write_view()
```

Nocter uses `View<T>` and `WriteView<T>` for non-owning views over contiguous elements.

Their canonical standard-library module path is `std/view`.

```nct
View<T>       // readonly contiguous view
WriteView<T> // readwrite contiguous view
```

`View<T>` allows reading contiguous `T` elements but does not own them.

`WriteView<T>` allows reading and writing contiguous `T` elements but does not own them.

The names are chosen to align with `StringView`.

```nct
func checksum(bytes: View<u8>): u32 {
    ...
}

func read_into(file: &+File, output: WriteView<u8>): usize!IOError {
    ...
}
```

Important distinction:

- `WriteView<T>` means the viewed elements are readwrite.
- `&+View<T>` means the `View<T>` value itself is readwrite borrowed.

These are not the same thing.

### Borrow-Like Provenance

Adopted: borrows and views carry hidden provenance tracked by the compiler.

Borrow-like values:

- `&T`
- `&+T`
- `StringView`
- `View<T>`
- `WriteView<T>`
- aggregates containing any borrow-like value

Provenance is compile-time information. It is not stored in the runtime value, does not affect ABI, and does not change the `ptr + len` layout of views.

Initial provenance source kinds:

```text
static       string literals and other static data
local        local owned values and stack storage
param_borrow storage reached through an input borrow-like parameter
owned_param  owned parameter storage
region       storage allocated through a region allocator
unknown      storage the compiler cannot prove
```

Rules:

- Borrow-like values keep the provenance of the storage they refer to.
- Derived views keep the same provenance as their source. For example, `StringView.bytes()` keeps the `StringView` provenance.
- Aggregates containing borrow-like values carry the contained provenance.
- `static` provenance may escape any function or region.
- `local` provenance must not escape the local scope.
- `owned_param` provenance must not escape the function because the owned parameter is dropped at function scope end unless moved.
- `region` provenance must not escape the region.
- `param_borrow` provenance may be returned from the function, but the caller may not use the returned borrow-like value longer than the original input borrow remains valid.
- `unknown` provenance cannot be returned from a function or stored into a longer-lived place in safe v0 code.
- `WriteView<T>` carries readwrite permission and follows the exclusivity rules of `&+T` for the viewed storage.
- `View<T>` and `StringView` carry readonly permission.
- A readonly borrow-like value may be derived from readonly or readwrite provenance.
- A readwrite borrow-like value may be derived only from readwrite provenance.
- If the compiler cannot prove the provenance and permission required for an escape or mutation, the program is invalid.

Examples:

```nct
func ok(): StringView {
    return "hello" // static
}
```

```nct
func bad(allocator: &+Allocator): StringView!AllocError {
    var text = try String.copy(allocator, "hello")
    return text.view() // error: local
}
```

```nct
func slice(input: StringView): StringView {
    return input // param_borrow-like provenance
}
```

```nct
func writable(input: WriteView<u8>): WriteView<u8> {
    return input // readwrite param_borrow-like provenance
}
```

Indexing uses bounds checks.

```nct
let first = read[0]      // traps if out of bounds
let maybe = read.get(0)  // u8?
```

`x[i]` traps on out-of-bounds access. Bounds checks are always-on for every build mode; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes). Trap semantics are specified in [Control Flow](03-control-flow.md#never-and-reachability). `x.get(i)` returns `T?` and is used when absence should be handled as a value.

Length is exposed through normal methods, not special fields.

```nct
let count = read.len()
```

Collection operations are ordinary standard-library methods.

Initial collection method direction:

- `len(): usize`
- `get(index: usize): T?`
- `ptr(): *T` for contiguous views
- `view(): View<T>` for owning collections that can expose readonly contiguous storage
- `write_view(): WriteView<T>` for owning collections that can expose readwrite contiguous storage
- `iter()` and `next()` may exist as ordinary standard-library APIs later, but `for` does not call them implicitly

The compiler may need built-in knowledge for fixed-size array layout and array literal typing. `Buffer<T>`, `View<T>`, `WriteView<T>`, `get`, `len`, `ptr`, `view`, `write_view`, and any future `iter` / `next` APIs should remain standard-library surface where possible.

## Strings

String literals have the canonical standard-library type `std/string.StringView`.

```nct
let name = "Nocter" // std/string.StringView
```

Source code can write the unqualified name `StringView` only when that name has been introduced by `use std/prelude` or explicitly imported from `std/string`.

The compiler places string literal bytes into the Mach-O image. A string literal is not an owned `String`, and the compiler must not allocate a heap object for it.

`StringView` is the borrowed string view type:

- It is a copy type.
- It is non-owning.
- It points to valid UTF-8 bytes.
- It does not run `drop`.
- It may point to static literal bytes or bytes owned by another object.
- It can expose its bytes as `View<u8>`.

`String` is the owning string type:

- It owns valid UTF-8 bytes.
- It is move-only.
- It is implemented in the standard library, likely on top of `Buffer<u8>`.
- It releases its buffer when dropped.
- It can produce a `StringView`.

```nct
let view: StringView = "README.md"
var owned = try String.copy(allocator, view)

open(view)
open(owned.view())

func open(path: StringView): File!IOError {
    ...
}
```

Adopted standard library surface:

```nct
impl String {
    pub func copy(allocator: &+Allocator, text: StringView): String!AllocError
    pub method (text: &Self).view(): StringView
}

impl StringView {
    pub method (text: Self).ptr(): *u8
    pub method (text: Self).len(): usize
    pub method (text: Self).bytes(): View<u8>
}
```

`View<u8>` represents arbitrary bytes and is not necessarily valid UTF-8. Converting `StringView` to `View<u8>` is allowed. Converting `View<u8>` to `StringView` requires UTF-8 validation.

There is no implicit conversion from a string literal to `&String`. `&String` borrows an existing owned `String` object. A string literal is already a `StringView`; creating an owned `String` from it requires an explicit copy.

The `char` type is deferred. Initial string APIs should operate on `StringView` and bytes until Unicode scalar and grapheme behavior is specified.

## Byte Literals and Escapes

Adopted: byte literals use `b'...'` and have type `u8`.

```nct
let a: u8 = b'a'
let newline: u8 = b'\n'
let raw: u8 = b'\xFF'
```

Rules:

- `b'...'` is a byte literal.
- A byte literal has type `u8`.
- A byte literal must decode to exactly one byte.
- Plain single-quoted literals such as `'a'` are not part of the initial design.
- The `char` type remains deferred.
- Single quote syntax is reserved for a future `Char` or Unicode scalar design.
- String literals use `"..."` and have canonical type `std/string.StringView`.
- String literals are UTF-8.
- String literal length APIs report byte length unless a future Unicode API explicitly says otherwise.
- Escapes are interpreted by the compiler before placing literal bytes into the Mach-O image.

Initial escapes:

```text
\n      newline, byte 0x0A
\r      carriage return, byte 0x0D
\t      horizontal tab, byte 0x09
\0      NUL, byte 0x00
\\      backslash
\"      double quote
\'      single quote
\xNN    byte with two hexadecimal digits
```

In a byte literal, `\xNN` may produce any byte from `0x00` through `0xFF`.

In a string literal, `\xNN` inserts that byte into the literal byte sequence. The final string literal must still be valid UTF-8.
