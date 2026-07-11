# Strings, Arrays, Views, and Pointers

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

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
pub(nocter) primitive from_addr<T>(address: usize): *T
```

`from_addr` is `pub(nocter)`, so it is visible only to trusted modules inside the active Nocter home: the common `std/` and the active target overlay `std/`. User project modules must not call it.

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

`&[T]`, `&+[T]`, and `&str` expose pointer and length methods.

```nct
impl &[T] {
    pub method (view: Self).ptr(): *T
    pub method (view: Self).len(): usize
}

impl &+[T] {
    pub method (view: Self).ptr(): *T
    pub method (view: Self).len(): usize
}

impl &str {
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
var bytes = Buffer<u8>.with_capacity(allocator, 4096)?
bytes.push(10)?

let read: &[u8] = bytes.view()
let write: &+[u8] = bytes.write_view()
```

Nocter uses built-in `[T]` type syntax for unsized contiguous array data. Array data is normally used behind a borrow:

```nct
[T]       // unsized contiguous array data
&[T]      // readonly contiguous array slice
&+[T]     // readwrite contiguous array slice
Vec<T>    // owned variable-length array
```

`[T]` describes the element sequence itself and is unsized. It cannot be used by value as a parameter, return value, field, local annotation, optional payload, fallible success payload, or generic argument.

`&[T]` allows reading contiguous `T` elements but does not own them.

`&+[T]` allows reading and writing contiguous `T` elements but does not own them.

The syntax mirrors borrow permissions: `&T` and `&[T]` are readonly, while `&+T` and `&+[T]` are readwrite.

```nct
func checksum(bytes: &[u8]): u32 {
    ...
}

func read_into(file: &+File, output: &+[u8]): usize! {
    ...
}
```

Important distinction:

- `&+[T]` means the viewed elements are readwrite.
- `&+T` means the `T` value itself is readwrite borrowed.

These are not the same thing.

### Borrow-Like Provenance

Adopted: borrows and views carry hidden provenance tracked by the compiler.

Borrow-like values:

- `&T`
- `&+T`
- `&str`
- `&[T]`
- `&+[T]`
- `ViewIter<T>`
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
- Derived views keep the same provenance as their source. For example, `text.bytes()` on an `&str` keeps the `&str` provenance.
- Aggregates containing borrow-like values carry the contained provenance.
- `static` provenance may escape any function or region.
- `local` provenance must not escape the local scope.
- `owned_param` provenance must not escape the function because the owned parameter is dropped at function scope end unless moved.
- `region` provenance must not escape the region.
- `param_borrow` provenance may be returned from the function, but the caller may not use the returned borrow-like value longer than the original input borrow remains valid.
- `unknown` provenance cannot be returned from a function or stored into a longer-lived place in safe v0 code.
- `&+[T]` carries readwrite permission and follows the exclusivity rules of `&+T` for the viewed storage.
- `&[T]` and `&str` carry readonly permission.
- A readonly borrow-like value may be derived from readonly or readwrite provenance.
- A readwrite borrow-like value may be derived only from readwrite provenance.
- If the compiler cannot prove the provenance and permission required for an escape or mutation, the program is invalid.

Examples:

```nct
func ok(): &str {
    return "hello" // static
}
```

```nct
func bad(allocator: &+Allocator): &str! {
    var text = String.copy(allocator, "hello")?
    return text.view() // error: local
}
```

```nct
func slice(input: &str): &str {
    return input // param_borrow-like provenance
}
```

```nct
func writable(input: &+[u8]): &+[u8] {
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
- `view(): &[T]` for owning collections that can expose readonly contiguous storage
- `write_view(): &+[T]` for owning collections that can expose readwrite contiguous storage
- readonly borrow iteration through ordinary `iter()` and `next()` methods

The compiler owns the layout and provenance rules for fixed-size arrays, `[T]`, `&[T]`, and `&+[T]`. `Buffer<T>`, `ViewIter<T>`, `get`, `len`, `ptr`, `view`, `write_view`, `iter`, and `next` remain ordinary API surface; the compiler must not special-case those names.

### Iteration

Adopted: v0 collection iteration is explicit readonly borrow iteration through standard-library iterator types.

The compiler does not lower `for item in collection` into calls to `iter` or `next`. The names `iter`, `next`, `ViewIter`, and `into_iter` are not special to the compiler.

Future iterator surface:

```nct
pub struct ViewIter<T> {
    ...
}

impl &[T] {
    pub method (view: Self).iter(): ViewIter<T>
}

impl ViewIter<T> {
    pub method (iter: &+Self).next(): (&T)?
}
```

`&[T].iter()` returns an iterator over readonly borrows into the viewed storage. `ViewIter<T>.next()` advances the iterator and returns an optional readonly borrow. The result type is written as `(&T)?` to mean "optional borrow"; it is not a borrow of an optional value.

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
}
```

Rules:

- `ViewIter<T>` is a standard-library type, not a compiler built-in.
- `ViewIter<T>` carries the same hidden provenance as the source `&[T]`.
- The `&T` returned from `next()` carries the same provenance and readonly permission as the source `&[T]`.
- The iterator must be stored in a `var` binding to call `next()` repeatedly because `next()` requires a `&+Self` receiver.
- `&+[T]` mutable element iteration is not part of v0.
- Owned iteration that moves elements out of a collection, such as `into_iter()`, is not part of v0.
- Range `for` remains the only `for` syntax in v0.

## Strings

String literals have the built-in type `&str`.

```nct
let name = "Nocter" // &str
```

Single-line and multi-line string literals are both string literals:

```nct
let one_line = "Nocter"
let many_lines = """
    first line
    second line
    """
```

`str` and `&str` are built-in type syntax and do not require an import. They are not exported by `std/string` or `std/prelude`.

The compiler places string literal bytes into the Mach-O image. A string literal is not an owned `String`, and the compiler must not allocate a heap object for it.

An interpolated string source form such as `"hello ${name}"` is not a string literal. It is an interpolated string expression and follows the separate interpolation rules below.

`str` is unsized UTF-8 string data. It describes the byte sequence itself and cannot be used by value as a parameter, return value, field, local annotation, optional payload, fallible success payload, or generic argument.

`&str` is the borrowed string slice type:

- It is a copy type.
- It is non-owning.
- It points to valid UTF-8 bytes.
- It does not run `drop`.
- It may point to static literal bytes or bytes owned by another object.
- It can expose its bytes as `&[u8]`.

`String` is the owning string type:

- It owns valid UTF-8 bytes.
- It is move-only.
- It is implemented in the standard library, likely on top of `Buffer<u8>`.
- It releases its buffer when dropped.
- It can produce a `&str`.

```nct
let view: &str = "README.md"
var owned = String.copy(allocator, view)?

open(view)
open(owned.view())

func open(path: &str): File! {
    ...
}
```

Adopted method surface direction:

```nct
impl String {
    pub func copy(allocator: &+Allocator, text: &str): String!
    pub method (text: &Self).view(): &str
}

impl &str {
    pub method (text: Self).ptr(): *u8
    pub method (text: Self).len(): usize
    pub method (text: Self).bytes(): &[u8]
}
```

`&[u8]` represents arbitrary borrowed bytes and is not necessarily valid UTF-8. Converting `&str` to `&[u8]` is allowed. Converting `&[u8]` to `&str` requires UTF-8 validation.

There is no implicit conversion from a string literal to `&String`. `&String` borrows an existing owned `String` object. A string literal is already a `&str`; creating an owned `String` from it requires an explicit copy.

The `char` type is deferred. Initial string APIs should operate on `&str` and bytes until Unicode scalar and grapheme behavior is specified.

## String and Byte Literals

Adopted: string literals use either single-line double-quoted syntax or multi-line triple-double-quoted syntax.

Rules:

- A single-line string literal has type `&str`.
- A multi-line string literal has type `&str`.
- Both forms are valid UTF-8 after escape processing.
- Both forms refer to static storage.
- Multi-line string literal indentation is removed by the lexical rules in [Lexical Grammar](13-lexical-grammar.md#string-and-byte-literals).
- Multi-line string literals do not add an implicit leading newline or trailing newline.
- A multi-line string literal can include line breaks in its value.
- The compiler must not allocate an owned `String` for a string literal.

Example:

```nct
let text = """
    alpha
    beta
    """
```

The value is equivalent to:

```nct
"alpha\nbeta"
```

## String Interpolation

Adopted direction: `${expr}` inside a string source form creates an interpolated string expression.

```nct
let message = "hello ${name}"?
let report = """
    user: ${name}
    count: ${count}
    """?
```

An interpolated string expression is not a string literal, even when every literal text segment is static. It constructs an owned `String` at runtime.

Rules:

- The result type of an interpolated string expression is `String!`.
- The expression is fallible because formatting or allocation can fail.
- Literal text segments are decoded with the same escape rules as string literals.
- Interpolation expressions are evaluated left to right with the surrounding literal text segments.
- Each `${expr}` expression is evaluated exactly once.
- Side effects in interpolation expressions occur at the interpolation position in left-to-right order.
- If any interpolation expression fails through `?`, `!`, or an explicitly fallible call, normal fallible propagation rules apply.
- If string construction fails, the expression fails with the error returned by the standard-library formatting operation.
- `String` remains an ordinary standard-library type. The compiler must not make the identifier `String` a built-in type name.
- The compiler must not treat user-defined names such as `to_string`, `format`, `append`, or `allocator` as magic.
- A bare string literal without `${...}` remains `&str` and does not allocate.

Formatting rules:

- `&str` values append their bytes.
- `String` values append their current string view.
- Integer and boolean values format with their canonical source spelling without extra whitespace.
- Optional, fallible, array, struct, enum, pointer, and user-defined nominal values are not interpolatable until a formatting trait or method protocol is adopted.
- Using a non-interpolatable expression inside `${...}` is a type error.

Allocator and lowering rules:

- Interpolation requires runtime storage for the resulting owned `String`.
- Nocter does not use GC and does not allow hidden compiler heap allocation for ordinary string literals.
- The exact standard-library lowering API for interpolated strings is part of the string formatting design and must remain explicit about allocation.
- A conforming implementation must not silently choose a process-global allocator for interpolation.
- Until the standard-library formatting API is finalized, an implementation may parse and type-check interpolation syntax but reject lowering with a diagnostic that the interpolation lowering API is not implemented.

The intended lowering is equivalent to constructing a `String` through ordinary standard-library operations, appending decoded text segments and formatted expression values in source order, then returning that owned value or a failure.

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
- Byte literal lexical syntax is specified in [Lexical Grammar](13-lexical-grammar.md#string-and-byte-literals).
- Plain single-quoted literals such as `'a'` are not part of the initial design.
- The `char` type remains deferred.
- Single quote syntax is reserved for a future `Char` or Unicode scalar design.
- String literals use `"..."` or `"""..."""` and have built-in type `&str`.
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
\$      dollar sign
\xNN    byte with two hexadecimal digits
```

In a byte literal, `\xNN` may produce any byte from `0x00` through `0xFF`.

In a string literal, `\xNN` inserts that byte into the literal byte sequence. The final string literal must still be valid UTF-8.
