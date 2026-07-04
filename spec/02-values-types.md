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
var count: Int = 0

let missing: Int // error
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
- Assignment requires a writable place.
- Assignment to a borrowed value is an error.
- For reassignment, the right-hand side is evaluated first.
- If right-hand-side evaluation succeeds, the old value is dropped and the new value is stored.
- If right-hand-side evaluation fails through `try`, the old value is not replaced before propagation. Normal scope-end cleanup still applies if control leaves the scope.
- Assigning an existing non-copy value requires explicit `move`.
- Assigning a copy value copies it.
- Field assignment follows the same ownership and borrow rules as local reassignment.
- Compound assignment such as `+=` follows the same writable-place and borrow rules as assignment.

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

Initial built-in type constructors:

```text
*T
T?
T!E
Array<T, N>
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

let signed: Int = 10
let unsigned = signed as u64 // error: not lossless for all Int values

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
   !x, -x, &x, &+x, move x, try x

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
