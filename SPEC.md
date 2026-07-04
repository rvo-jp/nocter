# Nocter Language Specification

This document defines the current design direction of the Nocter language. It is a living specification: syntax may still change, but decisions marked as adopted are treated as the default direction unless a later design note replaces them.

## Status

- Language name: Nocter
- Source extension: `.nct`
- Target: Apple Silicon macOS
- Output: ARM64 Mach-O executable
- Runtime GC: none
- Entry syntax: `program`
- Distribution directory: `~/.nocter`
- Compiler command: `nocter`

## Core Principles

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.

The language avoids giving special meaning to ordinary identifier names. Names such as `self`, `this`, `init`, and `main` are not magic. Special behavior must be represented by syntax, types, attributes, or explicit declarations.

Nocter prioritizes:

- direct compilation from `.nct` to ARM64 Mach-O
- no dependency on `clang`, `as`, `ld`, Xcode Command Line Tools, or external runtime libraries
- simple and readable high-level syntax
- value-centered program structure using `struct`, `enum`, `func`, `impl`, and modules
- memory management without GC
- standard-library implementation in Nocter, with limited low-level `asm` escape hatches

## Program Entry

Adopted: Nocter uses a dedicated top-level `program` construct for executable entry points.

```nct
program(): i32 {
    return 0
}
```

`program` is not a function name. It is a reserved top-level construct that defines the source-level entry point for an executable.

The compiler generates the real Mach-O entry code and connects it to the `program` body. The generated low-level entry code is an implementation detail.

### Allowed Forms

Initial allowed forms:

```nct
program(): void {
    ...
}
```

```nct
program(): i32 {
    return 0
}
```

Future candidate:

```nct
program(args: View<StringView>): i32 {
    ...
}
```

Rules:

- An executable must contain exactly one `program` construct.
- Library modules must not define `program`.
- `program` is not imported or exported as a normal function.
- `program(): void` exits with status code `0`.
- `program(): i32` uses the returned value as the process exit status.
- `func main()` has no special meaning. `main` is an ordinary identifier if used.

Rationale:

- avoids making the identifier `main` magical
- avoids requiring a general attribute system before the language needs one
- makes executable source files visually clear
- keeps the entry point explicit without adding project configuration

## Modules

Adopted: Nocter modules are derived from file paths. The language does not have a `module` declaration.

A module name is derived from the source file path relative to an import root:

```text
examples/word_count.nct => examples.word_count
std/io.nct              => std.io
```

The module name is a namespace. It groups related definitions and prevents accidental name collisions. The file path is the source of truth; there is no separate module name inside the file.

Imports make names from another module available.

```nct
import std.io.{File, stdout}
import std.mem.Allocator
```

Adopted import forms:

```nct
import std.mem.Allocator
import std.io.{File, stdout, stderr}
import std.io as io
import std.io.File as StdFile
```

Meaning:

- `import module.Name` imports a single exported name into the local import scope.
- `import module.{NameA, NameB}` imports multiple exported names from one module.
- `import module as alias` imports the module under an alias.
- `import module.Name as Alias` imports one exported name under an alias.

Examples:

```nct
import std.io as io
import std.io.File as StdFile

var out = io.stdout()
let file = try StdFile.open(path)
```

Name collisions are compile errors.

```nct
import std.io.File
import my.fs.File
// error: File is imported twice
```

Use aliases to resolve collisions.

```nct
import std.io.File as StdFile
import my.fs.File as MyFile
```

Not adopted:

```nct
import std.io.*
import ./foo
import ../bar
```

Wildcard imports, relative imports, and implicit-all imports are not part of the initial language.

## Visibility

Adopted: definitions are private by default. Public API is marked with `pub`.

```nct
pub struct File {
    fd: i32
}

impl File {
    pub func open(path: StringView): File!IOError {
        ...
    }

    method (file: &Self).raw_fd(): i32 {
        return file.fd
    }
}

pub func stdout(): File {
    ...
}
```

Rules:

- Top-level definitions are private to their module by default.
- `pub` on a top-level definition makes it importable from other modules.
- `import` can import only public names from another module.
- Struct fields are private by default.
- Public struct fields must be marked with `pub`.
- Functions and methods inside `impl` blocks are private by default.
- Public associated functions and methods must be marked with `pub`.
- `impl` blocks themselves are not marked `pub`.
- Enum variants follow the visibility of their enum in the initial design.
- Trait items follow the visibility of their trait in the initial design.
- There is no `private` keyword in the initial design.
- There is no `export` declaration in the initial design.

Example:

```nct
pub struct Point {
    pub x: Int
    pub y: Int
}

pub enum Direction {
    north
    south
    east
    west
}
```

Initial rules:

- One `.nct` file defines one module.
- `/` in the relative path becomes `.` in the module name.
- The `.nct` extension is removed.
- File and directory names used for modules must be snake_case identifiers.
- `module` is not a keyword.
- Initial design does not support `mod.nct` directory modules.
- Standard library modules live under `std`.
- `.nocter/std/io.nct` resolves as `std.io`.

Import roots:

1. The current project root.
2. The Nocter home directory, normally `~/.nocter`, for standard library modules.

The compiler locates Nocter home in this order:

1. `NOCTER_HOME`, if set.
2. The directory containing the running `nocter` executable.
3. Otherwise, report a clear configuration error.

## Distribution Layout

Adopted: the distributed toolchain is a single `.nocter/` directory.

Users install Nocter by placing the directory at `~/.nocter` or another location, then adding that directory to `PATH`.

```text
~/.nocter/
    nocter
    std/
        io.nct
        string.nct
        process.nct
```

Example shell setup:

```sh
export PATH="$HOME/.nocter:$PATH"
```

The repository also uses `.nocter/` as the build output directory for the distributable compiler and standard library.

## Bindings and Mutability

Bindings are immutable by default.

```nct
let count = 0
```

Mutable bindings use `var`.

```nct
var count = 0
count += 1
```

Borrows distinguish readonly access from readwrite access.

```nct
func inspect(file: &File): void {
    ...
}

func write(file: &+File, data: StringView): void!IOError {
    ...
}
```

Rules:

- `let` creates an immutable binding.
- `var` creates a mutable binding.
- `&T` is a readonly borrow type.
- `&+T` is a readwrite borrow type.
- `&value` creates a readonly borrow.
- `&+value` creates a readwrite borrow.
- `&+value` may be created only from a writable place, such as a `var` binding, a writable field, a writable index, or an existing `&+T` reborrow.
- Readonly borrows may coexist with other readonly borrows.
- A readwrite borrow is exclusive and cannot coexist with other readonly or readwrite borrows of the same value.
- A value cannot be moved while it is borrowed.
- A value cannot be explicitly dropped while it is borrowed.
- A borrow cannot outlive the value it refers to.
- A borrow of a stack value cannot escape the stack value's scope.
- A borrow of region-allocated memory cannot escape that region.
- Ordinary function calls require explicit borrow syntax at the call site.
- Method receivers may create the required borrow automatically.
- Lifetime annotations are not part of the initial design.
- `&+` is a single lexical token.
- Unary `+x` is not part of the language. This avoids ambiguity with `&+x`.

Examples:

```nct
var file = try File.open(path)

let a = &file
let b = &file       // OK: multiple readonly borrows
let c = &+file      // error: a and b are used below

inspect(a)
inspect(b)
```

```nct
var file = try File.open(path)

let w = &+file
drop file           // error: w is used below

write_more(w)
```

Ordinary function calls are explicit:

```nct
func inspect(file: &File): void {
    ...
}

inspect(&file)
```

Method receiver borrows are automatic:

```nct
impl File {
    pub method (file: &+Self).write(text: StringView): void!IOError {
        ...
    }
}

try file.write("hello")
```

The method call above creates a temporary readwrite borrow of `file` for the call. This does not enable UFCS-style calls:

```nct
File.write(&+file, "hello") // error
```

## Values and Types

Nocter is value-centered. Data is represented with explicit value types.

Initial primitive and built-in type names:

```text
bool
i8 i16 i32 i64
u8 u16 u32 u64
usize isize
Int
void
never
```

`Int` is adopted as the default general-purpose integer type.

```nct
type Int = i32
```

`Int` is an alias of `i32`, not a distinct type. Fixed-width integer types such as `i32` and `u64` remain available for ABI, binary format, pointer arithmetic, and low-level standard-library code.

Integer literal rules:

- Integer literals start as untyped integer literals.
- If an integer literal has an expected integer type, it takes that type when the value fits.
- If no context fixes the type, the literal becomes `Int`.
- Assigning an out-of-range literal is a type error.
- Non-literal integer values are not implicitly converted between integer types.

Examples:

```nct
let a = 10        // Int
let b: u64 = 10   // u64
let c: u8 = 300   // error: literal out of range

let x: Int = 10
let y: u64 = x    // error: no implicit integer conversion
```

```nct
struct WordStats {
    bytes: u64
    lines: u64
    words: u64
}
```

Constructors are ordinary expressions, not magic initializer names.

```nct
let stats = WordStats{
    bytes: 0,
    lines: 0,
    words: 0,
}
```

Adopted: enums represent finite variants and may carry data.

```nct
enum AppError {
    missing_path
    open_failed(path: StringView)
}
```

Rules:

- Enum variant names use snake_case.
- Variants may carry zero or more payload values.
- Variant constructors are qualified with the enum name, such as `AppError.open_failed(path)`.
- If an enum is public, its variants are public in the initial design.
- Per-variant visibility is not part of the initial design.

Adopted: `match` is the initial control flow form for enum pattern matching.

```nct
match error {
    is AppError.missing_path {
        ...
    }
    is AppError.open_failed(path) {
        ...
    }
    else {
        ...
    }
}
```

Rules:

- `match` is a statement in the initial design.
- Match arms use `is Pattern { ... }`.
- Fallback uses `else { ... }`.
- Enum matches without `else` must be exhaustive.
- Payload names in a pattern are bound only inside that arm block.
- `match` expressions that return values are deferred.
- `_` wildcard patterns are not part of the initial design; use `else`.

Adopted: `if expr is Pattern` checks one pattern.

```nct
if error is AppError.open_failed(path) {
    report(path)
}
```

Rules:

- `if expr is Pattern` uses the same pattern syntax as `match`.
- Payload names are bound only inside the `if` body.
- `else` may be used for the non-matching case.

## Functions

Functions are declared with `func`.

```nct
func scan_words(text: StringView): WordStats {
    ...
}
```

Names do not define special behavior. A function named `main`, `init`, `drop`, or `new` is ordinary unless the language later defines a syntactic rule around a trait or declaration.

Adopted: failure is represented with fallible types, not exceptions.

```nct
func open(path: StringView): File!IOError {
    if failed {
        fail IOError.not_found(path)
    }

    return file
}
```

`T!E` is a fallible type. It means the expression or function succeeds with `T` or fails with `E`.

```text
T!E = fallible T with error E
```

Inside a function returning `T!E`, `return value` returns the success value and `fail error` returns the failure value.

```nct
func write(file: &+File, text: StringView): void!IOError {
    if failed {
        fail IOError.write_failed
    }

    return
}
```

Adopted: the `try` operator unwraps successful fallible values and propagates failures.

```nct
let file = try File.open(path)
```

`try expr` requires `expr` to have type `T!E`. On success, the expression evaluates to the success value. On failure, the current function fails with the same error.

Example:

```nct
let file = try File.open(path)
```

This binds `file` to the successful `File` value. If `File.open(path)` fails, the current function fails with that error as if `fail error` had been executed.

Rules:

- `try` is not an exception mechanism.
- `try` does not perform stack unwinding.
- `try` can be used where the current function returns a compatible fallible or optional type.
- `fail` can be used only inside a function returning a fallible type.
- Scope-end cleanup and `drop` behavior still run as they would for an explicit `return` or `fail`.
- Error conversion is not implicit in the initial design. Use explicit mapping such as `map_error` when needed.
- `throw` is not part of the language.

Adopted: local handling of a fallible value uses `match` with `is ok` and `is fail` patterns.

```nct
match File.open(path) {
    is ok(file) {
        use(file)
    }
    is fail(error) {
        report(error)
    }
}
```

For `void!E`, the success arm has no payload:

```nct
match writer.write("Hello") {
    is ok {
        return
    }
    is fail(error) {
        report(error)
    }
}
```

Rules:

- Fallible match applies only to values of type `T!E`.
- `is ok(value)` handles the success value.
- `is ok` handles `void` success.
- `is fail(error)` handles the failure value.
- Both `ok` and `fail` arms are required in the initial design.
- `ok` and `fail` in these patterns are keywords, not imported names.

Fallible patterns can also be used with `if`.

```nct
if File.open(path) is ok(file) {
    use(file)
}

if File.open(path) is fail(error) {
    report(error)
}
```

## Optional Types

Adopted: optional values use the type syntax `T?`.

```text
T? = optional T
```

An optional value is either present with a `T` value or absent.

Inside a function returning `T?`, `return value` returns a present value and `return none` returns absence.

```nct
func env(name: StringView): StringView? {
    if missing {
        return none
    }

    return value
}
```

Optional patterns use `some` and `none`.

```nct
if env("HOME") is some(home) {
    use(home)
}

match env("HOME") {
    is some(home) {
        use(home)
    }
    is none {
        use_default_home()
    }
}
```

Rules:

- `T?` is not spelled as a special `Option<T>` type.
- `none` is an optional absent literal and pattern.
- `some(value)` is an optional present pattern.
- `return value` in a `T?` function returns the present value.
- `return none` in a `T?` function returns absence.
- `try expr` can unwrap `T?` inside a function returning `U?`.
- When `try` sees `none`, the current optional function returns `none`.
- Optional `match` without `else` must cover both `some` and `none`.
- `some` and `none` in these positions are keywords, not imported names.

Adopted: optional values support the optional default operator.

```nct
let value = maybe_value ?? default_value
```

Rules:

- `expr ?? default` applies only to optional values.
- If `expr` has type `T?` and is present, the result is the contained `T`.
- If `expr` is `none`, `default` is evaluated.
- The default expression may have type `T` or `T?`.
- If the default expression has type `T`, the whole expression has type `T`.
- If the default expression has type `T?`, the whole expression has type `T?`.
- The operator is right-associative.
- The default expression is evaluated only when needed.
- The operator does not apply to fallible `T!E` values.

Example:

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

This is parsed as:

```nct
let port = env_int("PORT") ?? (config.default_port ?? 8080)
```

## Conditional Operator

Adopted: Nocter has a ternary conditional operator.

```nct
let value = condition ? then_value : else_value
```

Rules:

- The condition expression must have type `bool`.
- The then and else expressions must have the same type in the initial design.
- Only the selected branch is evaluated.
- The conditional operator is an expression.
- The conditional operator does not apply to optional values; use `??` for optional defaults.
- The conditional operator is right-associative.

Example:

```nct
let label = count == 0 ? "empty" : "ready"
```

## Statements and Expressions

Adopted: the initial language is statement-centered.

`if`, `match`, and block bodies do not produce values in the initial design. Functions return values with explicit `return`. Fallible functions fail with explicit `fail`. Optional functions return absence with explicit `return none`.

```nct
func max(a: Int, b: Int): Int {
    if a > b {
        return a
    }

    return b
}
```

Rules:

- `if condition { ... }` is a statement.
- `if condition { ... } else { ... }` is a statement.
- `if expr is Pattern { ... }` is a statement.
- `match expr { ... }` is a statement.
- Blocks `{ ... }` do not return trailing expression values.
- `return value` is required to return a value from a function.
- `fail error` is required to return a failure from a fallible function.
- `return none` is required to return absence from an optional function.
- Expression-valued `if`, `match`, and block forms are deferred.

Invalid in the initial design:

```nct
let value = if condition {
    a
} else {
    b
}

return match result {
    is ok(value) { value }
    is fail(error) { 0 }
}
```

Use the ternary conditional operator for simple value selection.

```nct
let value = condition ? a : b
```

Statement separation:

- Semicolons are not part of the initial grammar.
- One statement per line is the normal style.
- A newline separates statements where the grammar can end a statement.
- A closing brace `}` ends the current block or arm.
- Multi-line expressions are allowed only where the expression syntax clearly continues, such as inside calls, literals, or parenthesized expressions.

## Loops

Adopted: the initial loop forms are `while`, `loop`, `break`, and `continue`.

```nct
var i: usize = 0

while i < bytes.len() {
    let byte = bytes[i]

    if byte == 0 {
        break
    }

    i += 1
}
```

```nct
loop {
    poll_once()

    if should_stop() {
        break
    }
}
```

Rules:

- `while condition { ... }` requires `condition` to have type `bool`.
- `loop { ... }` is an infinite loop unless exited by `break`, `return`, `fail`, or another terminating control flow.
- `break` exits the innermost loop.
- `continue` skips to the next iteration of the innermost loop.
- `break value` is not part of the initial design.
- Loops are statements and do not produce values.
- Exiting a loop runs the normal scope-end `drop` behavior for values whose scopes end.
- `break` and `continue` run the same cleanup for scopes they leave.

Deferred:

- `for item in expr { ... }`
- user-defined iteration protocols
- mutable element iteration over `WriteView<T>`
- iteration syntax that depends on ordinary names such as `iter` or `next`

`for` remains reserved for a future iteration construct, but it is not part of the initial executable grammar.

## Impl Blocks

Adopted: `impl` associates functions and methods with a type. It is not a class declaration and does not introduce inheritance.

`func` inside an `impl` defines an associated function. It has no receiver and is called through the type.

```nct
impl WordStats {
    pub func empty(): WordStats {
        return WordStats{
            bytes: 0,
            lines: 0,
            words: 0,
        }
    }
}
```

```nct
let stats = WordStats.empty()
```

`method` inside an `impl` defines a receiver method. The receiver is explicit and appears before the method name.

```nct
impl WordStats {
    pub method (stats: &+Self).add_byte(byte: u8): void {
        stats.bytes += 1
    }

    pub method (stats: &+Self).add_word(): void {
        stats.words += 1
    }
}
```

```nct
stats.add_word()
```

`Self` is a contextual type name inside an `impl` block. In `impl WordStats`, `Self` means `WordStats`.

Nocter does not reserve `self` or `this`. The receiver name is chosen by the author. `self` may be used as an ordinary receiver name, but it has no special meaning.

Initial receiver forms:

```nct
method (value: &Self).name(...): Return
method (value: &+Self).name(...): Return
method (value: Self).name(...): Return
```

Meaning:

- `&Self` is a readonly receiver.
- `&+Self` is a readwrite receiver.
- `Self` is a consuming receiver. It requires copy or explicit move according to the normal ownership rules.
- Calling a `&Self` method borrows the receiver readonly.
- Calling a `&+Self` method borrows the receiver readwrite and requires a writable receiver place.
- Calling a `Self` method consumes or copies the receiver according to the receiver type.

Call rules:

- `Type.function(args)` calls an associated `func`.
- `value.method(args)` calls a `method`.
- `Type.method(&value, args)` and `Type.method(&+value, args)` are invalid in the initial design.
- `value.function(args)` is invalid when `function` is only an associated `func`.
- `func` and `method` share the same member namespace for a type. Defining both with the same name for the same type is an error in the initial design.
- If method lookup finds multiple valid candidates, the call is ambiguous and is a compile error.
- The initial design has no qualified method-call escape hatch for ambiguity resolution.

```nct
try file.write("hello")          // OK: method call
try File.write(&+file, "hello")  // error: methods are not UFCS functions
```

Initial implementation order:

1. `impl Type { ... }`
2. `Self` inside `impl`
3. associated function calls such as `Type.function(...)`
4. method declarations
5. method calls such as `value.method(...)`

## Drop

Adopted: resource destruction uses a dedicated `drop` member inside `impl`, not a `Drop` trait.

```nct
impl File {
    drop(file: &+Self): void {
        std.os.close(file.fd).ignore()
    }
}
```

`drop` is not a normal function name. It is a special member allowed only inside an `impl` block.

Rules:

- A type may define at most one `drop` member.
- `drop` must return `void`.
- `drop` cannot be fallible.
- The first parameter must be `&+Self`.
- `drop` cannot be called as a normal associated function or method.
- `file.drop()` is invalid.
- `File.drop(&+file)` is invalid.
- Owned values are automatically dropped at scope end.
- Owned values are dropped in reverse declaration order.
- `return`, `fail`, and `try` propagation run the same scope-end drop behavior.
- A moved value is not dropped through the original binding.

Explicit early destruction uses a `drop` statement.

```nct
var file = try File.open(path)
drop file
```

After `drop file`, the binding is no longer valid.

```nct
file.read() // error
```

## Traits

Adopted: traits describe required behavior without class inheritance.

```nct
trait Writer {
    method (out: &+Self).write(text: StringView): void!IOError
}
```

`Self` is also available inside a trait declaration and means the implementing type.

Trait implementation uses `impl Trait for Type`.

```nct
impl Writer for File {
    method (file: &+Self).write(text: StringView): void!IOError {
        ...
    }
}
```

Generic functions may use trait bounds.

```nct
func print_line<W: Writer>(writer: &+W, text: StringView): void!IOError {
    try writer.write(text)
    try writer.write("\n")
    return
}
```

## Generics

Adopted: generic type parameters use angle brackets.

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: View<T>): T? {
    ...
}
```

Trait bounds are written inline with `:`.

```nct
func print<T: Format>(value: T): void
```

Multiple bounds use `+`.

```nct
func hash_key<K: Hash + Equal>(key: &K): u64
```

Generic implementation uses monomorphization. Each concrete instantiation is compiled as concrete code.

```nct
Buffer<Int>
Buffer<String>
```

This keeps generic dispatch static, avoids runtime type metadata for basic generics, and fits the no-runtime direction.

Initial generic scope:

- type parameters on structs
- type parameters on functions
- type parameters on impl blocks where needed
- inline trait bounds in the form `T: Trait`
- multiple bounds in the form `T: A + B`
- compile-time monomorphization

Deferred generic features:

- full `where` clauses
- higher-kinded types
- generic associated types
- const generics beyond the minimum needed for fixed-size arrays
- dynamic dispatch through `dyn Trait`

## Trait Scope

Initial trait scope:

- trait declarations
- `impl Trait for Type`
- generic bounds in the form `T: Trait`
- method declarations in traits
- method calls through trait bounds
- ambiguity is a compile error

Deferred trait features:

- trait objects such as `dyn Trait`
- trait inheritance
- associated types
- default methods
- blanket impls
- specialization
- full `where` clauses

Class inheritance is not part of the core language direction.

## Memory Management

Nocter does not use GC.

The memory model is based on:

- ownership
- moves
- borrowing
- automatic destruction at scope end
- explicit allocators
- region / arena allocation for bounded temporary memory

Owned values are destroyed when their scope ends unless ownership is moved.

```nct
let text = try file.read_to_string(allocator)
return move text
```

After `move`, the original binding cannot be used.

```nct
let a = Buffer.alloc(1024)
let b = move a
// a is no longer valid here
```

Temporary allocation may use regions.

```nct
region temp using allocator {
    let source = read_file(temp.allocator(), "main.nct")
    let tokens = lex(temp, source)
}
```

References to region-allocated memory must not escape the region.

## Copy and Move

Adopted: types are move-only by default. Only copy types may be copied implicitly.

Copyable structs are declared with `copy struct`.

```nct
copy struct Point {
    pub x: Int
    pub y: Int
}
```

Rules:

- Types are move-only by default.
- `copy struct` types are implicitly copyable.
- Every field of a `copy struct` must be copyable.
- A `copy struct` cannot define `drop`.
- A `copy struct` must not own resources that require destruction.
- Primitive numeric types, `bool`, and raw pointers are copyable.
- `Int` is copyable because it is an alias of `i32`.
- `&T` is copyable.
- `&+T` is not copyable.
- Non-copy values are not implicitly moved by assignment, argument passing, or return.
- Moving a non-copy value requires explicit `move`.

Examples:

```nct
let p1 = Point{x: 1, y: 2}
let p2 = p1 // OK: Point is copy

let text1 = String.new()
let text2 = text1      // error: String is not copy
let text3 = move text1 // OK
```

Function calls follow the same rule.

```nct
func consume(text: String): void {
    ...
}

let text = String.new()
consume(text)      // error
consume(move text) // OK
```

Returning non-copy owned values also uses explicit `move`.

```nct
func make_text(): String {
    let text = String.new()
    return move text
}
```

## Arrays and Views

Adopted: fixed-size arrays use `Array<T, N>`.

```nct
let header: Array<u8, 4> = [0x7F, 0x45, 0x4C, 0x46]
let numbers = [1, 2, 3] // Array<Int, 3>
```

Array literals use `[a, b, c]`.

Rules:

- If there is an expected `Array<T, N>` type, the literal is checked against that element type and length.
- Without an expected type, the compiler infers the element type from the elements.
- Integer-only array literals use `Int` unless context provides another integer type.
- The inferred length is part of the array type.

Owned growable memory is represented by standard-library types such as `Buffer<T>`. `Buffer<T>` is not a compiler builtin.

```nct
var bytes = try Buffer<u8>.with_capacity(allocator, 4096)
try bytes.push(10)

let read: View<u8> = bytes.view()
let write: WriteView<u8> = bytes.write_view()
```

Nocter uses `View<T>` and `WriteView<T>` for non-owning views over contiguous elements.

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

Indexing uses bounds checks.

```nct
let first = read[0]      // traps if out of bounds
let maybe = read.get(0)  // u8?
```

`x[i]` traps on out-of-bounds access. `x.get(i)` returns `T?` and is used when absence should be handled as a value.

Length is exposed through normal methods, not special fields.

```nct
let count = read.len()
```

The compiler may need built-in knowledge for fixed-size array layout and array literal typing. `Buffer<T>`, `View<T>`, `WriteView<T>`, `get`, and `len` should remain standard-library surface where possible.

## Strings

String literals have type `StringView`.

```nct
let name = "Nocter" // StringView
```

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
    pub method (text: Self).bytes(): View<u8>
}
```

`View<u8>` represents arbitrary bytes and is not necessarily valid UTF-8. Converting `StringView` to `View<u8>` is allowed. Converting `View<u8>` to `StringView` requires UTF-8 validation.

There is no implicit conversion from a string literal to `&String`. `&String` borrows an existing owned `String` object. A string literal is already a `StringView`; creating an owned `String` from it requires an explicit copy.

The `char` type is deferred. Initial string APIs should operate on `StringView` and bytes until Unicode scalar and grapheme behavior is specified.

## Standard Library and Low-Level Code

The compiler must not special-case names such as `print`, `exit`, or `File`.

Standard library functions provide these features.

```nct
import std.io.stdout

program(): i32 {
    var out = stdout()
    out.write("Hello\n").ignore()
    return 0
}
```

The standard library may use restricted ARM64 `asm` to connect Nocter code to macOS.

```nct
func write_stdout(text: StringView): void!IOError {
    asm {
        // ARM64 assembly
    }
}
```

Initial policy:

- `asm` is for the standard library first.
- General user code may be forbidden from using `asm`, or may require an explicit unsafe mode later.
- `asm` is ARM64-only for the current target.

## Reserved Keywords

Initial reserved keywords:

```text
import
program
func
pub
copy
struct
enum
trait
impl
method
let
var
return
if
else
for
while
loop
break
continue
match
is
try
ok
fail
some
none
move
region
using
asm
void
```

`program` is reserved because it is a top-level entry construct, not a normal identifier.

## Open Design Questions

The following areas remain intentionally open:

- exact grammar for generics
- exact generic parameter grammar beyond simple `T: Trait`
- full trait method-resolution order and ambiguity diagnostics
- `for` syntax and iteration protocol
- package layout and multi-file module resolution
- detailed lifetime inference and borrow-checker diagnostics beyond the adopted core rules
- whether attributes are needed later
- whether `asm` can ever be used outside the standard library
