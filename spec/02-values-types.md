# Values and Types

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

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

Local bindings must be initialized.

```nct
let path = "input.txt"
var count: i32 = 0

let missing: i32 // error
var later: File  // error
```

Assignment updates a writable place.

```nct
var file = try File.open(path)
file = try File.open(other_path)
```

Borrowing rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md).

Rules:

- `let` creates an immutable binding.
- `var` creates a mutable binding.
- Local `let` and `var` bindings require an initializer in the initial design.
- Uninitialized local variables are not part of the initial design.
- `let` bindings cannot be reassigned.
- `var` bindings may be reassigned.
- After `move name`, the binding enters an uninitialized state.
- After `drop name`, the binding enters an uninitialized state.
- A moved or explicitly dropped `let` binding cannot be reinitialized.
- A moved or explicitly dropped `var` binding may be reinitialized by assigning to the whole binding.
- Reinitializing a moved or explicitly dropped `var` binding does not drop an old value.
- If the right-hand side of a reinitialization fails through `try`, the binding remains uninitialized.
- An uninitialized binding cannot be read, borrowed, dropped, assigned through a field, or used for field access.
- Uninitialized bindings are not dropped at scope end.
- A maybe initialized binding cannot be read, borrowed, moved, explicitly dropped, assigned through a field, or used for field access.
- At scope end, maybe initialized bindings use conditional drop.
- To use a binding after a branch, every reachable path to that use must leave the binding initialized.
- Reinitializing only a field of an uninitialized binding is not part of v0.
- Assignment is a statement, not an expression.
- Assignment target must be a writable place.
- Writable places in v0 are `var` bindings, fields reachable through writable places, and fields reachable through `&+T` borrow bindings or parameters.
- `let` bindings are not writable places.
- Fields reached through `&T` are not writable places.
- Index assignment into `WriteView<T>`, arrays, or collections is deferred.
- Assignment to a borrowed value, or to a place whose parent is borrowed, is an error.
- Field assignment overwrites the field. It is not a partial move.
- For assignment, the right-hand side is evaluated first.
- If right-hand-side evaluation succeeds, the old value in the target place is dropped and the new value is stored.
- If right-hand-side evaluation fails through `try`, the target place is not changed. Normal scope-end cleanup still applies if control leaves the scope.
- Whole-binding assignment to a maybe initialized `var` binding is allowed. If the right-hand side succeeds, the compiler conditionally drops the old value if it is initialized, then stores the new value.
- Assigning an existing non-copy value requires explicit `move`.
- Assigning a copy value copies it.
- Field assignment follows the same ownership and borrow rules as local reassignment.
- Assignment itself produces no value.
- Chained assignment such as `a = b = c` is not part of v0.
- Compound assignment such as `+=` is allowed only for numeric writable places in v0.
- Compound assignment follows the same writable-place and borrow rules as assignment.

Examples:

```nct
let count = 0
count = 1 // error: let binding

var total = 0
total = 1 // OK
```

```nct
var a = try File.open(path_a)
var b = try File.open(path_b)

a = b      // error: File is not copy
a = move b // OK; b is no longer valid
```

```nct
var stats = WordStats.empty()
stats.bytes = 10
stats.lines += 1
```

If an owned field is overwritten, the old field value is dropped after the new value has been successfully evaluated.

```nct
var user = move old_user
user.name = move new_name
```

The field assignment above means:

1. Evaluate `move new_name`.
2. If evaluation succeeds, drop the old value in `user.name`.
3. Store the new value into `user.name`.
4. Mark `new_name` invalid.

If step 1 fails because the right-hand side contains `try`, `user.name` is not changed.

### Reinitialization After Move Or Drop

Adopted: v0 allows reinitialization only for whole `var` bindings after `move` or explicit `drop`.

```nct
var text = String.new()
consume(move text)

text = String.new() // OK: reinitializes the var binding
consume(move text)
```

Rules:

- Reinitialization is assignment to a whole `var` binding that is currently uninitialized because it was moved or explicitly dropped.
- Reinitialization is not reassignment over a live value, so no old value is dropped.
- If reinitialization succeeds, the binding becomes initialized again.
- If reinitialization fails through `try`, the binding remains uninitialized.
- `let` bindings cannot be reinitialized after move or explicit drop.
- Field reinitialization after moving a whole binding is not part of v0.
- Partial initialization states for structs are not part of v0.
- At scope end, only initialized bindings are dropped.
- Definite initialization is checked across control flow.

Examples:

```nct
var file = try File.open(path)
close(move file)

file.read() // error: file is uninitialized

file = try File.open(other_path)
try file.read()
```

```nct
var text = String.new()

if condition {
    consume(move text)
    text = String.new()
}

consume(move text) // OK: both paths leave text initialized
```

```nct
var text = String.new()

if condition {
    consume(move text)
}

consume(move text) // error: text may be uninitialized
```

### Initialization State Across Control Flow

Adopted: the compiler tracks binding initialization state across control flow.

Tracked states:

```text
initialized
uninitialized
maybe initialized
```

Rules:

- New `let` and `var` bindings start initialized because local bindings require initializers.
- `move name` changes that binding to uninitialized on paths that continue after the move.
- `drop name` changes that binding to uninitialized on paths that continue after the drop.
- Successful whole-binding assignment to a `var` binding changes that binding to initialized.
- Reads, borrows, moves, field access, field assignment, method calls through the binding, and explicit `drop name` require initialized state.
- A maybe initialized binding cannot be used directly.
- At a control-flow join, only reachable incoming paths are considered.
- If all incoming paths have the binding initialized, the joined state is initialized.
- If all incoming paths have the binding uninitialized, the joined state is uninitialized.
- If incoming paths disagree, the joined state is maybe initialized.
- Scope end drops initialized bindings.
- Scope end does not drop uninitialized bindings.
- Scope end conditionally drops maybe initialized bindings.
- Conditional drop is generated by the compiler. It is not user-visible state and does not change the source-level type.
- A whole-binding assignment to a maybe initialized `var` binding may be used to restore the state to initialized.
- `if`, `match`, loop exits, `break`, `continue`, `return`, `fail`, and `try` propagation participate in the same state analysis.
- For loops, the compiler treats the body as running zero or more times and computes a conservative fixed point. If a binding's state may differ after the loop, the result is maybe initialized.

Examples:

```nct
var text = String.new()

if condition {
    consume(move text)
    text = String.new()
}

consume(move text) // OK: initialized on all paths
```

```nct
var text = String.new()

if condition {
    consume(move text)
}

consume(move text) // error: maybe initialized
```

```nct
var file = try File.open(path)

if should_close {
    close(move file)
}

// file is maybe initialized here.
// It cannot be used directly, but scope end will conditionally drop it.
```

```nct
var file = try File.open(path)

if should_close {
    close(move file)
}

file = try File.open(other_path)
try file.read() // OK: whole-binding assignment restored initialized state
```

## Values and Types

Nocter is value-centered. Data is represented with explicit value types.

Initial primitive and built-in type names:

```text
bool
i8 i16 i32 i64
u8 u16 u32 u64
usize isize
void
never
```

Initial built-in type syntax:

```text
*T
&T
&+T
T?
T!E
[T; N]
```

Initial built-in literal values:

```text
true
false
none
```

`true` and `false` have type `bool`. `none` is a contextual optional absence literal and requires an expected `T?` type.

Names such as `String`, `StringView`, `View`, `WriteView`, `Allocator`, `File`, `IOError`, `OSError`, `print`, `exit`, and `abort` are not compiler built-ins.

`Int` is not a compiler built-in name.

The standard library prelude may define and export `Int` as a normal type alias:

```nct
pub type Int = i32
```

Using `Int` requires `use std/prelude` or an explicit import from `std/prelude`. The compiler must not treat the identifier `Int` specially.

```nct
use std/prelude
```

No implicit prelude is part of this rule. `use std/prelude` is file-local and explicit.

When imported, `Int` is an alias of `i32`, not a distinct type. Fixed-width integer types such as `i32` and `u64` remain available for ABI, binary format, pointer arithmetic, and low-level standard-library code.

### Type Aliases

Adopted: `type` declares a pure type alias. A type alias introduces another name for the exact same type. It does not create a distinct nominal type.

```nct
pub type Int = i32
pub type Bytes = View<u8>
pub type Map<K, V> = HashMap<K, V>
```

Rules:

- Type aliases are top-level declarations.
- Type aliases are private by default.
- `pub type` makes the alias importable and re-exportable.
- Generic type aliases are allowed.
- A type alias has no separate identity from its target type.
- A type alias does not change ownership, copyability, drop behavior, layout, or ABI.
- Implementations cannot target a type alias.
- A type alias cannot be used to create a type-safe wrapper around an existing type.
- No dedicated `newtype` syntax is part of v0.
- Use a `struct` when a distinct type is required.

Examples:

```nct
let x: Int = 10
let y: i32 = x  // OK: Int is i32
```

```nct
type UserId = u64
type OrderId = u64

let user: UserId = 10
let order: OrderId = user  // OK: both aliases are u64
```

```nct
pub copy struct UserId {
    pub value: u64
}
```

```nct
impl Int {
    ...
}
// error: impl target must be a nominal type, not a type alias
```

### Integer Literals

Integer literal rules:

- Integer literals start as untyped integer literals.
- If an integer literal has an expected integer type, it takes that type when the value fits.
- If no context fixes the type, the literal becomes `i32`.
- Assigning an out-of-range literal is a type error.
- Non-literal integer values are not implicitly converted between integer types.

Examples:

```nct
let a = 10        // i32
let b: u64 = 10   // u64
let c: u8 = 300   // error: literal out of range

let x: i32 = 10
let y: u64 = x    // error: no implicit integer conversion
```

## Numeric Operations and Conversions

Adopted: numeric operations do not perform implicit integer conversion.

Rules:

- Integer binary arithmetic uses operands of the same integer type.
- Integer literals may take an expected integer type when the value fits.
- Non-literal integer values are not implicitly converted.
- `bool` does not implicitly convert to or from integer types.
- Explicit conversion uses `expr as Type`.
- `as` is allowed only for lossless conversions in the initial design.
- Narrowing conversions are not allowed with `as`.
- Signedness-changing conversions are not allowed with `as` unless the target type can represent every value of the source type.
- On the initial ARM64 macOS target, `usize` has the same range as `u64`, and `isize` has the same range as `i64` for conversion checking.

Examples:

```nct
let a: u32 = 10
let b: u64 = 20

let c = a + b          // error
let d = (a as u64) + b // OK
```

```nct
let x: u32 = 10
let y: u64 = x as u64 // OK: lossless widening

let signed: i32 = 10
let unsigned = signed as u64 // error: not lossless for all i32 values

let big: u64 = 300
let small = big as u8       // error: narrowing
let checked = u8.checked(big)    // u8?
let truncated = u8.truncate(big) // u8
```

`checked` and `truncate` are explicit numeric conversion APIs. They are ordinary associated functions on primitive numeric types, not special names in the grammar.

Arithmetic trap rules:

- Overflow in normal integer arithmetic traps.
- Wrapping arithmetic must use an explicit wrapping API.
- Division by zero traps.
- Remainder by zero traps.
- Signed division overflow, such as minimum signed value divided by `-1`, traps.
- Shift counts greater than or equal to the bit width of the shifted value trap.
- Shift counts must be non-negative.

Trap semantics are specified in [Control Flow](03-control-flow.md#never-and-reachability). These arithmetic safety checks are always-on for every build mode; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes).

The exact names for wrapping arithmetic APIs belong to the primitive numeric API surface, not to the operator grammar.

## Operators, Comparison, and Precedence

Adopted: operator behavior is built in for a small initial set. User-defined operator overloads are not part of the initial design.

Comparison rules:

- `==` and `!=` require operands of the same type.
- Built-in equality is available for `bool`, integer types, and payloadless enum types.
- Struct equality is not automatically generated.
- Payload-carrying enum equality is not part of the initial design. Use `match` or `if expr is Pattern`.
- `<`, `<=`, `>`, and `>=` are ordering comparisons.
- Ordering comparisons require numeric operands of the same type.
- Ordering comparisons are not defined for `bool`, structs, strings, or enums in the initial design.

Logical rules:

- `&&` requires `bool` operands and returns `bool`.
- `||` requires `bool` operands and returns `bool`.
- `&&` short-circuits: the right operand is evaluated only when the left operand is `true`.
- `||` short-circuits: the right operand is evaluated only when the left operand is `false`.
- `!expr` requires `expr: bool` and returns `bool`.

Unary numeric rules:

- `-expr` requires a signed numeric operand in the initial design.
- Unary `+expr` is not part of the language.

Precedence, from highest to lowest:

```text
1. call / method / index / field
   f(x), x.method(), x[i], x.field

2. postfix / type conversion
   expr as Type

3. unary
   !x, -x, &x, &+x, move name, try x

4. multiplicative
   *, /, %

5. additive
   +, -

6. shift
   <<, >>

7. ordering comparison
   <, <=, >, >=

8. equality comparison
   ==, !=

9. logical and
   &&

10. logical or
    ||

11. optional default
    ??

12. ternary conditional
    condition ? then : else
```

Rules:

- Assignment is a statement, not an expression, and is not part of the precedence table.
- The half-open range token `..<` is part of the initial `for` header syntax, not a general binary operator, and is therefore not in the precedence table.
- `??` is right-associative.
- The ternary conditional operator is right-associative.
- `&&`, `||`, `??`, and the ternary conditional operator evaluate only the needed operand or branch.

Example:

```nct
if count > 0 && state == ScanState.inside_word {
    ...
}
```

## Structs and Value Construction

Adopted: struct values are constructed with explicit named-field struct literals.

```nct
pub struct User {
    pub id: u64
    name: String
}
```

```nct
let user = User{
    id: 1,
    name: try String.copy(allocator, "alice"),
}
```

Rules:

- Struct literal syntax is `Type{ field: value, ... }`.
- The type in a struct literal must name a struct type. For generic structs, the type may include type arguments.
- Every field must be initialized exactly once.
- Field order in the literal is free.
- Unknown fields are compile errors.
- Duplicate fields are compile errors.
- Field initializer expressions are evaluated left to right in the order written in the literal.
- Field initializer expressions follow normal ownership, move, copy, borrow, and `try` rules.
- If a later field initializer fails through `try`, already initialized owned field values are dropped in reverse initialization order before the failure propagates.
- Private fields may be initialized only inside the module that defines the struct.
- Public fields may be initialized from other modules.
- There is no constructor overloading in v0.
- Field default values are not part of v0.
- Struct update syntax is not part of v0.
- Positional structs and tuple structs are not part of v0.
- Dedicated constructor syntax is not part of v0.
- Names such as `new`, `init`, and `create` are ordinary associated function names. The compiler does not special-case them.

When initialization logic or validation is needed, use an ordinary associated function.

```nct
impl User {
    pub func create(id: u64, name: String): User {
        return User{
            id: id,
            name: move name,
        }
    }
}
```

Outside the defining module, a struct with private fields can be created only through public APIs exposed by that module.

```nct
let user = User.create(1, try String.copy(allocator, "alice"))
```

## Enums and Variant Construction

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
- Payloadless variants are constructed as `EnumName.variant_name`.
- Payload variants are constructed as `EnumName.variant_name(args...)`.
- Variant construction requires the payload arity and types to match the variant declaration.
- Variant payload arguments are evaluated left to right.
- Variant constructors are qualified with the enum name, such as `AppError.open_failed(path)`.
- Unqualified variant constructors are not part of v0.
- Variant constructors are not ordinary functions and are not magic identifier names; they are generated by the enum declaration.
- Enum variants and associated functions share the type member namespace in v0. Defining an associated function with the same member name as a variant is a compile error.
- If an enum is public, its variants are public in the initial design.
- Per-variant visibility is not part of the initial design.

Examples:

```nct
let state = ScanState.inside_word
let error = AppError.open_failed(path)
```

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

Adopted: `if enum_expr is Pattern` checks one enum pattern.

```nct
if error is AppError.open_failed(path) {
    report(path)
} else if error is AppError.read_failed(path) {
    report(path)
} else {
    report_other(error)
}
```

Rules:

- `if enum_expr is Pattern` uses the same enum pattern syntax as `match`.
- Payload names are bound only inside the then body.
- `else` may be used for the non-matching case.
- `else` is optional.
- `else if enum_expr is Pattern` is allowed.
- `else if enum_expr is Pattern` is equivalent to `else { if enum_expr is Pattern { ... } }`.
- Payload names are not available in `else` or later `else if` branches.
- `if is` is a statement and does not produce a value.
- `if is` does not apply to fallible values `T!E`.
- `if is` does not apply to optional values `T?`.
