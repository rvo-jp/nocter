# Strings, Arrays, Views, and Pointers

## Raw Pointers and Address API

Nocter has raw pointer values, but raw pointer dereference is not available to general user code.

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
- Raw pointer dereference has no user-facing escape hatch. There is no `unsafe` block that enables it.

Raw pointer dereference is not part of the user-facing language.

Invalid operations:

```nct
*pointer
pointer.*
pointer.load()
pointer.store(value)
```

These operations may be reconsidered only if Nocter later adopts an explicit unsafe or trusted-code model. The Nocter-home trusted boundary does not enable them in user source.

### `std/ptr`

Pointer and address conversion APIs live in the compiler-checked
[`std/ptr` contract](../../development/std/ptr/index.nct). Their observable conversion and
zero-sized-address behavior belongs to the [Pointer and Address Conversion](../../development/std/ptr/README.md)
guide. Pointer-to-integer conversion is an ordinary library call, not `as` syntax. Package-internal
address-to-pointer and raw-view construction remain inaccessible to user packages.

Example:

```nct
use std/ptr

func address_of(value: &u8): usize {
    let pointer = ptr.from_ref(value)
    return ptr.addr(pointer)
}
```

### View Pointer APIs

The active Nocter home declares observation methods for `&[T]`, `&+[T]`, and `&str` in the
compiler-checked [`std/slice`](../../development/std/slice/index.nct) and
[`std/str`](../../development/std/str/index.nct) contracts. The compiler built-in types own the
method identities, while declarations and ordinary bodies remain standard-library source. A
readwrite slice may call readonly `[T]` methods by capability weakening. Observing a pointer grants
no dereference permission.

### Pointer Intrinsics

The `std/ptr` declarations are target-independent core primitives. Raw memory projection belongs to
the package-internal `std/internal/ptr` contract and target-gated operating-system operations belong
to `std/internal/os`. The compiler validates every primitive against the closed registry described
in [Standard-Library Primitive and OS Boundary](../platform/primitives-and-os.md); none of these
roles makes higher-level output, process, allocation, string, buffer, or file APIs intrinsic.

## Arrays and Views

Fixed-size arrays use `[T; N]`.

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
- The length `N` in `[T; N]` is a compile-time constant expression of type `usize`, as specified in
  [Compile-Time Constants](constants.md).
- `[T; 0]` is valid and contains no initialized elements.
- A fixed array of a zero-sized element type still contains its declared number of logical
  elements even though its stored size is zero. Element evaluation, bounds, ownership, and drop
  behavior use that logical count.
- The element type `T` must be sized. Unsized `str` and `[T]` elements must be
  used behind an indirection such as `&str` or `&[T]`.
- Array literal elements are evaluated left to right.
- Array literal elements are comma-delimited and may use one trailing comma on any layout.
- If a later element expression fails through postfix `?`, already initialized
  owned elements are dropped in reverse index order before the failure propagates.
- A fixed array is copyable only when its element type is copyable.
- A fixed array is move-only when its element type is move-only.

Owned growable memory is represented by standard-library types such as `Vec<T>`. `Vec<T>` is not a
compiler builtin. Declaration-driven typed literals provide forms such as `Vec [1, 2, 3]`, while
bare `[1, 2, 3]` remains a fixed-size array literal. See
[Argument Packs, Literal Definitions, and Sequence Spread](literals-and-packs.md).

```nct
var bytes = Vec<u8>.with_capacity(4096)
bytes.push(10)

let read: &[u8] = &bytes as &[u8]
let write: &+[u8] = &+bytes as &+[u8]
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

Borrows and views carry hidden provenance tracked by the compiler.

Borrow-like values:

- `&T`
- `&+T`
- `&str`
- `&[T]`
- `&+[T]`
- aggregates containing any borrow-like value

Provenance is compile-time information. It is not stored in the runtime value, does not affect ABI, and does not change the `ptr + len` layout of views.

Provenance source kinds:

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
- `unknown` provenance cannot be returned from a function or stored into a longer-lived place.
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
func bad(): &str {
    var text = String.copy("hello")
    return &text as &str // error: local
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

`x[i]` traps on out-of-bounds access. Bounds checks are always-on for every build mode; see [Safety Checks and Build Modes](control-flow.md#safety-checks-and-build-modes). Trap semantics are specified in [Control Flow](control-flow.md#never-and-reachability). `x.get(i)` returns `T?` and is used when absence should be handled as a value.

Length is exposed through normal methods, not special fields.

```nct
let count = read.len()
```

Collection operations are ordinary standard-library methods.

The compiler owns the layout and provenance rules for fixed-size arrays, `[T]`, `&[T]`, and
`&+[T]`. The active Nocter home exclusively owns `instance` declarations for built-in `[T]`; their
exact API and behavior belong to the [slice contract](../../development/std/slice/index.nct) and
[guide](../../development/std/slice/README.md). Owning collection and iterator names remain
declaration-resolved API surface; the compiler does not infer behavior from member spelling.

### Iteration

Readonly, readwrite, and owned iteration use ordinary standard-library iterator declarations. Their
exact source, yielded types, provenance, exhaustion, and destruction behavior belongs to the
compiler-checked [iteration contract](../../development/std/iter/index.nct) and its
[behavior guide](../../development/std/iter/README.md). Expansion declarations, rather than an
iterator type or method spelling, connect a source type to `for`.

```nct
for i in 0..<bytes.len() {
    let byte = bytes[i]
    consume(byte)
}
```

Collection `for` loops dispatch through
[Expansion Operators](literals-and-packs.md#expansion-operators). Iterator and method names are not
compiler-recognized substitutes.

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

`str` is declared by `pub primitive type str` in `std/str`, and `&str` applies the ordinary
readonly-borrow type constructor to it. The compiler-selected declaration is available in every
type context without an import; it is not exported by `std/string` or `std/prelude`.

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
- It is an ordinary standard-library type rather than a compiler built-in.
- It releases its owned storage when dropped.
- It can produce a `&str`.

```nct
let view: &str = "README.md"
var owned = String.copy(view)

open(view)
open(&owned as &str)

func open(path: &str): File! {
    ...
}
```

The compiler-checked standard-library contracts own the exact `str`, `String`, slice, and `Vec`
operations and their observable behavior. See [Borrowed Text](../../development/std/str/README.md),
[Owned Strings](../../development/std/string/README.md), [Vectors](../../development/std/vec/README.md),
and [Slices](../../development/std/slice/README.md).
This language chapter owns only the built-in `str` data meaning, borrow types, literal behavior,
and the coercion and method-selection rules that make those library declarations usable.

`&[u8]` represents arbitrary borrowed bytes and is not necessarily valid UTF-8. Converting `&str` to `&[u8]` is allowed. Converting `&[u8]` to `&str` requires UTF-8 validation.

There is no implicit conversion from a string literal to `&String`. `&String` borrows an existing owned `String` object. A string literal is already a `&str`; creating an owned `String` from it requires an explicit copy.

The contract uses type-owned coercions both at expected-type boundaries and for
one-step method-receiver lookup. It does not change literal types or insert a source borrow outside
method receiver preparation. See
[Borrow Coercions](borrow-coercions.md).

Borrowed observation, forwarding policy, and explicit iterator construction belong to the standard
text and collection contracts linked above. At the language level, callers use ordinary method
lookup, expected-type coercion, or an explicit expression such as `(&text) as &str`; the compiler
does not synthesize forwarding members on owning types.

Unicode scalar construction, observation, and iteration belong to
[Unicode Scalar Values](unicode-scalars.md). Unicode properties, Unicode whitespace trimming,
and default case conversion belong to
[Unicode Text and Scalars](../../development/std/char/README.md). Byte-oriented APIs in this chapter keep
their existing meanings and do not silently adopt scalar, grapheme, or normalization behavior.

## String and Byte Literals

String literals use either single-line double-quoted syntax or multi-line triple-double-quoted syntax.

Rules:

- A single-line string literal has type `&str`.
- A multi-line string literal has type `&str`.
- Both forms are valid UTF-8 after escape processing.
- Both forms refer to static storage.
- Each compiled literal occurrence evaluates to a stable static `&str` for the program lifetime.
  The compiler may pool identical decoded bytes or share overlapping prefix or suffix storage
  between different occurrences. It may also keep occurrences separate.
- No address equality or inequality is guaranteed between distinct literal occurrences, even when
  their decoded contents are equal. String identity and equality use decoded bytes, not `ptr()`.
- An empty string literal still carries a non-null `*u8`-compatible pointer and length zero. No byte
  is live or readable through that pointer.
- Multi-line string literal indentation is removed by the lexical rules in [Lexical Grammar](lexical-grammar.md#string-character-and-byte-literals).
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

`${expr}` interpolates values inside string source forms.

```nct
let message = "hello ${name}"
let report = """
    user: ${name}
    count: ${count}
    """
```

An interpolated string expression is not a string literal, even when every literal text segment is static. It constructs an owned `String` at runtime.

Rules:

- The result type of an interpolated string expression is `String`.
- Ordinary interpolation uses the current aborting allocation context. Allocation
  failure terminates according to the standard allocator policy.
- Literal text segments are decoded with the same escape rules as string literals.
- Interpolation expressions are evaluated left to right with the surrounding literal text segments.
- Each `${expr}` expression is evaluated exactly once.
- Side effects in interpolation expressions occur at the interpolation position in left-to-right order.
- If an interpolation expression propagates through postfix `?` or explicit `return`, the partial
  owned `String` is dropped by normal scope-exit cleanup before control leaves.
- Postfix `!` inside interpolation uses the ordinary non-recoverable safety trap and performs no
  cleanup. The partial `String` is not dropped on that path.
- A bare fallible call does not propagate. Its complete outcome value must itself be legally
  formattable or the interpolation is a type error; use `?` to interpolate only its success value.
- Unsupported interpolation values are rejected statically. Recoverable allocation uses
  `Format.try_format_into` rather than changing interpolation to `String!`.
- `String` remains an ordinary standard-library type. The compiler must not make the identifier `String` a built-in type name.
- The compiler must not treat user-defined names such as `to_string`, `format`, `append`, or `allocator` as magic.
- A bare string literal without `${...}` remains `&str` and does not allocate.

Formatting rules:

Interpolation requires implementation of the exact `std/fmt.Format` interface selected from the
active Nocter home. Its exact declaration and standard implementations belong to the
compiler-checked [`std/fmt` contract](../../development/std/fmt/index.nct); destination mutation,
failure, and formatting behavior belongs to the [Formatting](../../development/std/fmt/README.md)
guide.

- A project-owned struct or enum becomes interpolatable only through an explicit implementation of
  that exact standard interface.
- Formatting borrows the value. An existing value remains usable after interpolation, and a
  temporary remains live through the selected formatting operation before it is destroyed exactly
  once.
- Generic code may interpolate `T` when its active requirements include `T impl Format` for the
  exact selected declaration.
- A project interface with the same name does not grant interpolation behavior.
- Optional, fallible, array, pointer, callable, and opaque values are rejected unless they acquire a
  legal explicit implementation under the normal interface-implementation rules.
- Missing or ambiguous implementation is a type error at the `${...}` expression.

Allocator and lowering rules:

- Interpolation requires runtime storage for the resulting owned `String`.
- Nocter does not use GC and does not allow hidden compiler heap allocation for ordinary string literals.
- The lowering uses the compiler-propagated current allocation context. It must
  not read a mutable process-global allocator.
- Interpolation participates in the same selected-target buildability validation for `check`,
  `build`, and `run`; none may report success when the required lowering capability is absent.

The intended lowering is equivalent to constructing a `String` through ordinary
standard-library operations in the current context, appending decoded text
segments and formatted expression values in source order, then returning that
owned value.

## String, Byte, and Character Literals

Byte literals use `b'...'` and have type `u8`.

```nct
let a: u8 = b'a'
let newline: u8 = b'\n'
let raw: u8 = b'\xFF'
```

Rules:

- `b'...'` is a byte literal.
- A byte literal has type `u8`.
- A byte literal must decode to exactly one byte.
- Byte and character lexical syntax is specified in
  [Lexical Grammar](lexical-grammar.md#string-character-and-byte-literals).
- Plain single-quoted literals such as `'a'` have built-in type `char` and decode exactly one
  Unicode scalar value.
- The full `char` contract belongs to [Unicode Scalar Values](unicode-scalars.md).
- String literals use `"..."` or `"""..."""` and have built-in type `&str`.
- String literals are UTF-8.
- String literal length APIs report byte length. Scalar-aware APIs are named explicitly by the
  Unicode text chapters.
- Escapes are interpreted by the compiler before placing literal bytes into the Mach-O image.

Escapes:

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
