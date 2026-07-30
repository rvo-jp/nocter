# Values and Types

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

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
var file = File.open(path)?
file = File.open(other_path)?
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
- If the right-hand side of a reinitialization fails through postfix `?`, the binding remains uninitialized.
- An uninitialized binding cannot be read, borrowed, dropped, assigned through a field, or used for field access.
- Uninitialized bindings are not dropped at scope end.
- A maybe initialized binding cannot be read, borrowed, moved, explicitly dropped, assigned through a field, or used for field access.
- At scope end, maybe initialized bindings use conditional drop.
- To use a binding after a branch, every reachable path to that use must leave the binding initialized.
- Reinitializing only a field of an uninitialized binding is not part of v0.
- Assignment is a statement, not an expression.
- Assignment target must be a writable place.
- Writable places in v0 are `var` bindings, fields reachable through writable
  places, fields reachable through `&+T` borrow bindings or parameters,
  elements of fixed-size arrays reached through writable places, and elements
  of `&+[T]` readwrite slices.
- `let` bindings are not writable places.
- Fields reached through `&T` are not writable places.
- Elements reached through `&[T]` are not writable places.
- Built-in index assignment applies to fixed-size arrays and `&+[T]` slices.
  Owned collection indexing is ordinary library API behavior and is not a
  compiler-provided writable place in v0.
- Assignment to a place that conflicts with an active borrow is an error. The field-sensitive conflict rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#field-sensitive-borrows).
- Field assignment overwrites the field. It is not a partial move.
- For assignment, the right-hand side is evaluated first.
- If right-hand-side evaluation succeeds, the old value in the target place is dropped and the new value is stored.
- If right-hand-side evaluation fails through postfix `?`, the target place is not changed. Normal scope-end cleanup still applies if control leaves the scope.
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
var a = File.open(path_a)?
var b = File.open(path_b)?

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

If step 1 fails because the right-hand side contains postfix `?`, `user.name` is not changed.

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
- If reinitialization fails through postfix `?`, the binding remains uninitialized.
- `let` bindings cannot be reinitialized after move or explicit drop.
- Field reinitialization after moving a whole binding is not part of v0.
- Partial initialization states for structs are not part of v0.
- At scope end, only initialized bindings are dropped.
- Definite initialization is checked across control flow.

Examples:

```nct
var file = File.open(path)?
close(move file)

file.read() // error: file is uninitialized

file = File.open(other_path)?
file.read()?
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
- `if`, `match`, loop exits, `break`, `continue`, `return`, and postfix `?` propagation participate in the same state analysis.
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
var file = File.open(path)?

if should_close {
    close(move file)
}

// file is maybe initialized here.
// It cannot be used directly, but scope end will conditionally drop it.
```

```nct
var file = File.open(path)?

if should_close {
    close(move file)
}

file = File.open(other_path)?
file.read()? // OK: whole-binding assignment restored initialized state
```

## Values and Types

Nocter is value-centered. Data is represented with explicit value types.

Initial primitive and built-in type names:

```text
bool
i8 i16 i32 i64
u8 u16 u32 u64
usize isize
str
error
void
never
```

Initial built-in type syntax:

```text
*T
&T
&+T
[T]
&[T]
&+[T]
T?
T!
T?!
[T; N]
(T)
Self
```

`T!` means a fallible value whose success payload is `T` and whose failure payload is the built-in `error` type. `T?!` means a fallible value whose success payload is optional.

Parentheses in type syntax group a type without creating a new type. For example, `(&T)?` means an optional readonly borrow, while `&(T?)` means a readonly borrow of an optional value.

### Self Type Syntax

Adopted: `Self` is type-position syntax, not an ordinary user-defined name.

`Self` is valid only in type positions inside an inherent `impl` block or a
qualified associated function declaration such as `func File.open`.

Meaning:

- In `impl File { ... }`, `Self` means `File`.
- In `func File.open(...)`, `Self` means `File`.

Rules:

- `Self` cannot be used as a value expression.
- `Self` cannot be used as a binding name, parameter name, function name, method name, field name, enum variant name, module name, type declaration name, type parameter name, or import alias.
- `Self` is not resolved through normal name lookup.
- `Self` is not imported or exported.
- `Self` has no meaning outside inherent member type positions.
- Lowercase `self` is not special. It is an ordinary identifier if it is otherwise valid in that syntactic position.

This preserves Nocter's rule that ordinary names do not define special behavior. The special behavior belongs to type syntax, not to a value or declaration name.

Initial built-in literal values:

```text
true
false
none
```

`true` and `false` have type `bool`. `none` is a contextual optional absence literal and requires an expected `T?` type.

User-defined typed literal construction, such as `Vec [1, 2, 3]` or
`Path "README.md"`, is a post-v0 future design. It is specified separately in
[Future Literal Definitions and Spread](17-future-literal-definitions-spread.md)
and does not change the meaning of built-in literals in v0.

Built-in core type forms include `str`, `error`, `[T]`, `&str`, `&[T]`, `&+[T]`, and `[T; N]`. These forms are type-position syntax, not ordinary names imported from a module. In particular, `error` may still be used as a value binding name, such as the conventional binding in `catch error { ... }`.

`str` is unsized UTF-8 string data. `[T]` is unsized contiguous array data. These unsized data forms cannot be used by value as parameters, return values, fields, local annotations, optional payloads, fallible success payloads, or generic arguments unless they are behind an indirection. Use `&str` for a string slice, `&[T]` for a readonly array slice, `&+[T]` for a readwrite array slice, `String` for owned variable-length text, and `Vec<T>` for owned variable-length arrays.

Names such as `String`, `Error`, `ErrorCode`, `Vec`, `ViewIter`, `Allocator`, `File`, `IOError`, `OSError`, `print`, `args`, `env`, `cwd`, `exit`, and `abort` are not compiler built-ins.

`Int` is not part of v0. The compiler must not treat the identifier `Int` specially, and the standard-library prelude does not export it. User code should write `i32` or define a project-local alias when a domain-specific name is useful.

### Type Aliases

Adopted: `type` declares a pure type alias. A type alias introduces another name for the exact same type. It does not create a distinct nominal type.

```nct
pub type Count = i32
pub type Bytes = [u8]
pub type Map<K, V> = HashMap<K, V>
```

Rules:

- Type aliases are top-level declarations.
- Type aliases are private by default.
- `pub type` makes the alias importable and re-exportable.
- `pub(nocter) type` makes the alias importable only inside the active Nocter home.
- Generic type aliases are allowed.
- A type alias has no separate identity from its target type.
- A type alias does not change ownership, copyability, drop behavior, layout, or ABI.
- Implementations cannot target a type alias.
- A type alias cannot be used to create a type-safe wrapper around an existing type.
- No dedicated `newtype` syntax is part of v0.
- Use a `struct` when a distinct type is required.

Examples:

```nct
let x: Count = 10
let y: i32 = x  // OK: Count is i32
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
impl Count {
    ...
}
// error: impl target must be a nominal type, not a type alias
```

### Integer Literals

Integer literal rules:

- Accepted integer literal syntax is defined in [Lexical Grammar](13-lexical-grammar.md#integer-literals).
- Integer literals start as untyped integer literals.
- If an integer literal has an expected integer type, it takes that type when the value fits.
- If no context fixes the type, the literal becomes `i32`.
- Assigning an out-of-range literal is a type error.
- Non-literal integer values are not implicitly converted between integer types.
- Float literals are not part of v0.

Examples:

```nct
let a = 10        // i32
let b: u64 = 10   // u64
let c: u8 = 300   // error: literal out of range
let d = 0xFF_FF   // i32
let e: u8 = 0b1010

let x: i32 = 10
let y: u64 = x    // error: no implicit integer conversion
```

## Numeric Operations and Conversions

Adopted: numeric operations do not perform implicit integer conversion.

Rules:

- Integer binary arithmetic uses operands of the same integer type.
- Shift operators use operands of the same integer type. Integer literals on the right side may be contextually typed by the left operand type when the value fits.
- Shift expressions return the left operand type.
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
- Built-in equality is available for `bool`, integer types, `&str`, and payloadless enum types.
- `String == String`, `String == &str`, and `&str == String` require operator definitions and are deferred in v0.
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
   expr?, expr!, expr as Type

3. unary
   !x, -x, &x, &+x, move name

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

11. fallible catch / optional otherwise
    expr catch name { body }
    expr otherwise { body }
```

Rules:

- Assignment is a statement, not an expression, and is not part of the precedence table.
- The half-open range token `..<` is part of the initial `for` header syntax, not a general binary operator, and is therefore not in the precedence table.
- `catch` binds to the immediately preceding expression after higher-precedence
  operators have been parsed.
- `otherwise` is right-associative.
- `if` and `match` are control expressions, not precedence-table operators.
- `condition ? then : else` is not Nocter syntax. Use `if condition { then } else { else_value }`.
- `enum_expr ?{ ... }` is not Nocter syntax. Use `match enum_expr { ... }`.
- `&&`, `||`, `otherwise`, `if`, and `match` evaluate only the needed operand, branch, or arm.

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
    name: String.copy(allocator, "alice")?,
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
- Field initializer expressions follow normal ownership, move, copy, borrow, and postfix `?` rules.
- If a later field initializer fails through postfix `?`, already initialized owned field values are dropped in reverse initialization order before the failure propagates.
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
pub func User.create(id: u64, name: String): User {
    return User{
        id: id,
        name: move name,
    }
}
```

Outside the defining module, a struct with private fields can be created only through public APIs exposed by that module.

```nct
let user = User.create(1, String.copy(allocator, "alice")?)
```

## Enums and Variant Construction

Adopted: enums represent finite variants and may carry data.

```nct
enum AppError {
    missing_path
    open_failed(path: &str)
}
```

Rules:

- Enum variant names use snake_case.
- Variants may carry zero or more payload values.
- Payloadless variants are constructed as `EnumName.variant_name`.
- Payload variants are constructed as `EnumName.variant_name(args...)`.
- In the v0 backend, payloadless enum tags use `u8` ABI representation, so a payloadless enum may have at most 256 variants.
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
    AppError.missing_path {
        ...
    }
    AppError.open_failed(path) {
        ...
    }
    _ {
        ...
    }
}
```

Rules:

- `match` may be used as a statement or as an expression.
- Match arms use `Pattern { ... }`.
- `_ { ... }` is the fallback arm and matches any remaining value.
- A `match` may have at most one `_` fallback arm.
- The `_` fallback arm must be the last arm.
- When `match` is used as an expression, each selected arm body result is the expression value.
- A `match` expression without `_` must cover all variants to avoid a `void` missing-branch type.
- Match expression arm result types must be compatible. A `never` arm is compatible with the other result type.
- `match` without `_` is treated as a terminating statement when every enum variant is covered by an explicit arm and every arm terminates.
- `match` with `_` is treated as a terminating statement when every explicit arm and the `_` arm terminate.
- In the v0 backend, payloadless enum `match` statements and supported
  payloadless enum `match` expressions lower through the enum tag ABI.
- Payload-carrying enum construction, local storage, returns, and value
  arguments lower in the current runtime-supported payload subset: copy/no-drop
  payloads, plus variants with one direct-drop aggregate payload when active
  payload cleanup is required.
- Tag-only payload-carrying enum `if is Enum.variant(_)` statements and
  tag-only payload-carrying enum `match` statements lower over existing enum
  bindings and parameters in the current runtime-supported payload subset,
  including wildcard-only, nonexhaustive no-wildcard, and exhaustive
  no-wildcard statement forms.
- Payload-carrying enum `if is Enum.variant(binding)` statements/value
  expressions and `match` statement/value-expression arms lower over existing
  enum bindings and parameters when the payload ABI is `i32`, `u8`, `usize`,
  `bool`, or `&str`. The binding is loaded only inside the matching branch.
- Payload-carrying enum active payload cleanup lowers for runtime-supported enum
  values whose droppable variants each have exactly one direct-drop aggregate
  payload. Scope-end cleanup, parameter cleanup, discarded call results,
  call-result bindings, and whole-local replacement drop only the active payload.
- Payload-carrying enum `if is` / `match` over temporary expressions,
  aggregate/slice/non-copy payload binding, and multi-field droppable payload
  cleanup are typechecked in v0 but still reject before build/run.
- Payload names in a pattern are bound only inside that arm block.
- `_` inside a payload pattern, such as `AppError.open_failed(_)`, requires a
  payload to exist and discards it without introducing a binding.
- `_` by itself is valid only as the `match` fallback arm. It is not a valid
  `if is` pattern.

Example value selection:

```nct
return match error {
    AppError.missing_path { missing_code() }
    AppError.open_failed(path) { code_for(path) }
    _ { unknown_code() }
}
```

Removed: `enum_expr ?{ ... }` is not Nocter syntax. Use `match` expressions for enum pattern value selection.

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
- `if enum_expr is Enum.variant(_)` checks only the variant and discards the
  payload without introducing a binding.
- `else` may be used for the non-matching case.
- `else` is optional.
- `else if enum_expr is Pattern` is allowed.
- `else if enum_expr is Pattern` is equivalent to `else { if enum_expr is Pattern { ... } }`.
- Payload names are not available in `else` or later `else if` branches.
- `if expr is Pattern` may be used as a statement or as an expression with the same body-result rules as ordinary `if`.
- `if is` does not apply to fallible values `T!`.
- `if is` does not apply to optional values `T?`.
