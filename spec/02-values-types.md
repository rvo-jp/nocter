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

Use a discard initializer when an evaluated value is intentionally ignored:

```nct
let _ = String.copy("unused")
let _ = try_operation()
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
- `let _ = expression` is a discard initializer. It evaluates `expression` but creates no binding.
- A discard initializer accepts an expression of any type. It is the only source form that may
  intentionally discard a non-`void`, non-`never` body value.
- The discarded value is consumed and any owned content is dropped at the end of the discard
  statement. A borrow-like value requires no drop and its borrow ends according to normal
  statement-end liveness.
- Discarding `T?`, `T!`, `T?!`, or `(T!)?` does not unwrap, recover, or propagate it. The complete
  outcome value, including any active success, absence, or failure payload, is intentionally
  discarded and its owned content is dropped.
- Discarding an existing move-only binding still requires `move`, as in `let _ = move value`.
- `_` in a discard initializer cannot have a type annotation, cannot be referenced, and cannot be
  used with `var`.
- Local `let` and `var` bindings require an initializer.
- Uninitialized local declarations are not supported.
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
- Reinitializing only a field of an uninitialized binding is not supported.
- Statically named fields of a partially initialized struct independently carry initialized,
  uninitialized, or maybe initialized state. A field state becomes maybe initialized when control
  flow merges initialized and uninitialized incoming states.
- An initialized named field may be read or moved. An uninitialized or maybe initialized field may
  not be read, borrowed, moved, explicitly dropped, or used by compound assignment.
- Disjoint definitely initialized fields remain usable while another field is uninitialized or
  maybe initialized. The complete parent may be read, borrowed, moved, or passed only after every
  field is definitely initialized.
- Assignment is a statement, not an expression.
- Assignment target must be a writable place.
- Writable places are `var` bindings, fields reachable through writable
  places, fields reachable through `&+T` borrow bindings or parameters,
  elements of fixed-size arrays reached through writable places, elements of
  `&+[T]` readwrite slices, elements selected by a readwrite index declaration,
  and elements reached through one selected coercion to either kind of
  readwrite index operation.
- `let` bindings are not writable places.
- Fields reached through `&T` are not writable places.
- Elements reached through `&[T]` are not writable places.
- Built-in index assignment applies to fixed-size arrays and `&+[T]` slices. A
  nominal collection becomes a writable index place through an accessible
  `operator (&+self[index: K]): &+V` declaration or one accessible coercion to
  a readwrite index operation.
- Assignment to a place that conflicts with an active borrow is an error. The field-sensitive conflict rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#field-sensitive-borrows).
- Field assignment stores a complete field value. It either overwrites an initialized field or
  restores an uninitialized or maybe initialized field; it never creates a new partial state.
- For assignment, the complete right-hand side is evaluated first. After it succeeds, dynamic
  target-place components are evaluated exactly once. An initialized old value is dropped, a maybe
  initialized old value is conditionally dropped, and an uninitialized place performs no old-value
  drop. The new value is then stored and the target place becomes initialized.
- If right-hand-side evaluation propagates or terminates, the target expression is not evaluated
  and no assignment drop or store occurs. Side effects already performed by the right-hand side
  remain. Normal scope-end cleanup still applies to recoverable propagation.
- Whole-binding assignment to a maybe initialized `var` binding is allowed. If the right-hand side succeeds, the compiler conditionally drops the old value if it is initialized, then stores the new value.
- Named-field assignment to an uninitialized or maybe initialized field of a writable partial
  `var` parent is allowed when every proper-prefix field needed to reach it exists. On success that
  field becomes definitely initialized, so the complete parent becomes initialized once all fields
  are initialized.
- Whole-binding assignment over a partial `var` parent drops every remaining initialized field in
  reverse declaration order, conditionally drops each maybe initialized field, and then stores the
  complete replacement. Such a parent cannot own a drop declaration because its earlier partial
  move would have been rejected.
- Assigning an existing non-copy value requires explicit `move`.
- Assigning a copy value copies it.
- Field assignment follows the same ownership and borrow rules as local reassignment.
- Assignment itself produces no value.
- Chained assignment such as `a = b = c` is not supported.
- Compound assignment operators are `+=`, `-=`, `*=`, `/=`, and `%=`. They are allowed only for
  numeric writable places and require a right-hand side of the same numeric type.
- A compound assignment evaluates the complete right-hand side first. If that evaluation
  propagates or terminates, the target expression is not evaluated and no compound write occurs;
  side effects already performed by the right-hand side remain.
- After the right-hand side succeeds, dynamic target-place components such as an index expression
  or source-defined readwrite index operation are evaluated exactly once. The current target value
  is then read, the corresponding checked numeric operation is performed, and the result is stored.
- Compound assignment uses the same overflow, division, remainder, and writable-place rules as the
  corresponding ordinary numeric operation and assignment. It is not a textual desugaring to
  `target = target operator rhs`, because that would duplicate or reorder target evaluation.
- Compound assignment follows the same borrow-conflict rules as assignment at each evaluation
  point.
- Compound assignment requires a definitely initialized target because it reads the old value
  before writing the result. It cannot restore an uninitialized or maybe initialized field.

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

Whole `var` bindings may be reinitialized after a whole-value `move` or explicit `drop`. A writable
named field may be restored after an eligible field move without allowing field-by-field
construction of a never-initialized parent.

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
- Field reinitialization after moving a whole binding is not supported.
- Assignment may restore an uninitialized or maybe initialized field only when the parent began as
  a fully initialized value and became partial through named-field move. The right-hand side and
  conditional old-value drop follow the common assignment order above.
- Struct construction and whole-binding reinitialization do not accept field-by-field partial
  initialization. A fully initialized struct may enter the compiler-tracked partial state only
  after an eligible named-field move, under the restrictions in
  [Move Expressions](05-ownership-borrowing-drop.md#move-expressions).
- At scope end, uninitialized bindings are skipped, initialized bindings use ordinary drop,
  maybe initialized bindings use conditional drop, and partial structs apply those states to each
  named field in reverse declaration order.
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

The compiler tracks binding initialization state across control flow.

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

Primitive and built-in type names:

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

Built-in type syntax:

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

Supported optional and fallible compositions are ordinary sized
values. They may be stored in bindings and sized aggregates, moved, assigned, passed as arguments,
returned, and consumed later. An optional with no fallible layer is copyable exactly when its
recursively contained payload is copyable. Every fallible value and every mixed outcome containing
a fallible layer is move-only because its failure branch owns an `error`. Only the selected tag
branch is initialized. Absence never initializes a success payload, and failure initializes the
`error` payload instead of the success payload. The complete rules are defined in
[outcome copyability](05-ownership-borrowing-drop.md#copy-and-move).

Outcome construction at a callable return boundary is contextual, not a subtype conversion. The
checker records each presence or success injection required by the complete declared result type;
an expression that already has that complete type keeps its existing tags unchanged. The normative
algorithm is [Recursive Outcome Injection](04-errors-optionals.md#recursive-outcome-injection).

Nocter supports one optional layer, one fallible layer, or one of each in either order. Repeated
equal layers and deeper recursive outcome types are not supported.

`void!` is valid and represents a recoverable operation with no success value. An optional layer
must not ultimately wrap `void`; `void?`, `void?!`, `(void?)!`, and `(void!)?` are invalid. This
restriction is checked after alias expansion and generic substitution, so an otherwise valid
generic `T?` cannot be instantiated with `T = void`. Use an enum when absence and successful
completion are observably different states.

`never` is a control-flow termination type, not an outcome payload. `never?`, `never!`,
`never?!`, and `(never!)?` are invalid after alias expansion and generic substitution. Use `void!`
for a recoverable operation that has no success value, and use an enum for a value-level state.

After alias expansion, `never` may appear as the complete result of a function, method, closure,
or structural callable type. A type alias may name `never`, but using that alias remains subject to
the same position rule. `never` is invalid as a binding or parameter type, borrow or pointer
pointee, aggregate field or enum payload, array element, outcome payload, generic argument,
associated-type binding, or any other data-bearing type position. Use `*void` for an opaque raw
pointer.

`void` is a completion type, not a zero-sized value type. After alias expansion, it may appear as
the complete result of a function, method, closure, or structural callable type; as the direct
success completion of `void!`; and as the pointee spelling of opaque `*void`. It is invalid as a
binding or parameter type, borrow pointee, aggregate field or enum payload, array element, optional
payload, generic argument, associated-type binding, or any other data-bearing type position. A
type alias may name `void`, but does not bypass these use-site rules. Use an empty struct when a
storable zero-sized unit or marker value is required.

Prefix type operators bind more tightly than postfix outcome operators. Therefore `&T?` is an
optional readonly borrow, while `&(T?)` is a readonly borrow of an optional value. Parentheses in
type syntax group a type without creating a new type.

### Contextual Expected Types

An authoritative expected type flows from a destination into its expression at these boundaries:

- an explicitly typed binding initializer
- a simple assignment
- a callable argument
- a struct field initializer
- a fixed-array element initializer
- a typed-sequence literal capture
- an enum payload argument
- a `catch` or `otherwise` fallback result
- an explicit `return` or callable body result
- a contextually typed closure result

Grouping preserves the same expectation. `if`, `if is`, and `match` propagate an enclosing
expectation independently to every value-producing branch. The expected payload type of `catch`
and `otherwise` comes from the operated-on outcome; it does not need a further enclosing
destination.

An expected `void` result is a completion consumer rather than a value destination. An expression
of type `void` may be evaluated there and then complete normally, as in `return log_message()`.
When the expected type is `void!`, recursive outcome injection evaluates a `void` expression and
constructs payloadless success only after that expression completes. This does not make `void` a
storable value or a valid generic substitution.

Optional and fallible values use
[Recursive Outcome Injection](04-errors-optionals.md#recursive-outcome-injection) at these
boundaries. Outcome injection is directional: it consumes an expected type already supplied by
the program context. It does not infer an outcome wrapper from an unannotated initializer or from
a sibling control-flow branch.

```nct
let present: i32? = 42
let absent: i32? = none
let failed: i32! = error.new("app.failed", "operation failed")

let missing = none // error: no expected optional type
```

For a generic expected type with statically known outcome structure, inference may project through
those outcome layers and collect constraints for the payload. Injection occurs only after the
substitution is unique. `none` and a failure `error` select tags but contribute no payload-type
constraint, so they cannot determine an otherwise unknown generic parameter. A `never` expression
terminates before producing an argument or result and likewise contributes no type constraint; it
is checked only after another source determines the expected type. A `void` completion expression
also contributes no generic payload constraint.

```nct
func inspect<T>(value: T?): void {
    return
}

inspect(42)   // T = i32; inject presence after inference
inspect(none) // error: T cannot be inferred from absence
```

### Self Type Syntax

`Self` is type-position syntax, not an ordinary user-defined name.

`Self` is valid only in type positions owned by a type or interface declaration: an `instance` or
`conform` declaration, an interface member signature or default body, or a `construct` entry.

Meaning:

- In `instance File { ... }`, `Self` means `File`.
- In `interface Source { ... }`, `Self` means the eventual conforming type.
- In `conform Source for File { ... }`, `Self` means `File`.
- In `construct File { ... }`, `Self` means `File`.

Rules:

- `Self` cannot be used as a value expression.
- `Self` cannot be used as a binding name, parameter name, function name, method name, field name, enum variant name, module name, type declaration name, type parameter name, or import alias.
- `Self` is not resolved through normal name lookup.
- `Self` is not imported or exported.
- `Self` has no meaning outside a type- or interface-owned type position.
- Lowercase `self` is not special. It is an ordinary identifier if it is otherwise valid in that syntactic position.

This preserves Nocter's rule that ordinary names do not define special behavior. The special behavior belongs to type syntax, not to a value or declaration name.

### Associated Type Projections

Type selections are resolved from left to right. When the prefix names an imported module
namespace, `.Name` selects one exported type declaration, as in `parser.Parser<T>`. Once the prefix
denotes a type, `.Name` is an associated type projection selected by an interface conformance.

```nct
func next<S>(source: &+S): S.Item? where S: Source {
    return source.next()
}
```

`Self.Item` is valid when the current interface declares `Item`. `S.Item` requires exactly one
interface requirement on `S` to declare `Item`. A concrete projection such as `FileSource.Item`
requires exactly one applicable conformance that binds `Item`. Projection normalization also
applies beneath existing type constructors, so `Vec<S.Item>`, `S.Item?`, and `&S.Item` retain their
ordinary outer type rules.

An unknown or ambiguous selection is an error. Type arguments may follow a module-selected nominal
type, but not an associated projection because generic associated types are not supported. Nocter
does not select a declaration by import order, interface spelling, or the name `Item`.
Associated-type declarations, bindings, and constraints are specified in
[Generics, Interfaces, and Methods](08-generics-interfaces-embedding-methods.md#associated-types).

Built-in literal values:

```text
true
false
none
```

`true` and `false` have type `bool`. `none` is a contextual optional absence literal and requires an expected `T?` type.

User-defined typed literal construction, such as `Vec [1, 2, 3]` or `Path "README.md"`, is specified
in [Argument Packs, Literal Definitions, and Sequence Spread](17-argument-packs-literals-sequence-spread.md). It does
not change the meaning of built-in literals.

Built-in core type forms include `str`, `error`, `[T]`, `&str`, `&[T]`, `&+[T]`, and `[T; N]`. These forms are type-position syntax, not ordinary names imported from a module. In particular, `error` may still be used as a value binding name, such as the conventional binding in `catch error { ... }`.

Primitive scalar and view storage sizes are part of the target
[ABI and Layout](09-abi-layout.md#struct-layout) contract. Register transport does not widen their
stored aggregate fields.

`str` is unsized UTF-8 string data. `[T]` is unsized contiguous array data. These unsized data forms cannot be used by value as parameters, return values, fields, local annotations, optional payloads, fallible success payloads, or generic arguments unless they are behind an indirection. Use `&str` for a string slice, `&[T]` for a readonly array slice, `&+[T]` for a readwrite array slice, `String` for owned variable-length text, and `Vec<T>` for owned variable-length arrays.

Names such as `String`, `Vec`, `ViewIter`, `Allocator`, `File`, `OSError`, `print`, `args`, `env`, `cwd`, `exit`, and `abort` are not compiler built-ins.

The compiler does not treat `Int` specially, and the standard-library prelude does not export it. User code should write `i32` or define a project-local alias when a domain-specific name is useful.

### Type Aliases

`type` declares a pure type alias. A type alias introduces another name for the exact same type. It does not create a distinct nominal type.

```nct
pub type Count = i32
pub type Bytes = [u8]
pub type Map<K, V> = HashMap<K, V>
```

Rules:

- Type aliases are top-level declarations.
- Type aliases are private by default.
- A non-private `pub(...) type` makes the alias importable inside its declared visibility boundary.
- Bare `pub type` makes the alias importable and re-exportable across packages.
- Generic type aliases are allowed.
- A type alias has no separate identity from its target type.
- A direct or indirect alias-expansion cycle is invalid because it has no finite exact target type.
- A type alias does not change ownership, copyability, drop behavior, layout, or ABI.
- Implementations cannot target a type alias.
- A type alias cannot be used to create a type-safe wrapper around an existing type.
- There is no dedicated `newtype` syntax.
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
instance Count {
    ...
}
// error: instance target must be a nominal type, not a type alias
```

### Integer Literals

Integer literal rules:

- Accepted integer literal syntax is defined in [Lexical Grammar](13-lexical-grammar.md#integer-literals).
- An unsigned N-bit type has the inclusive range `0` through `2^N - 1`. A signed N-bit type has
  the inclusive range `-2^(N - 1)` through `2^(N - 1) - 1` and uses two's-complement
  representation. On the current target, `usize` is `u64`-width and `isize` is `i64`-width.
- Integer literals start as untyped integer literals.
- If an integer literal has an expected integer type, it takes that type when the value fits.
- When unary `-` applies directly to an integer literal, with any number of grouping parentheses
  between them, range checking uses the combined negative mathematical value. For an expected
  signed N-bit type, the positive magnitude may therefore be at most `2^(N - 1)`, allowing the
  exact minimum value. An expected unsigned type is invalid because unary `-` requires a signed
  operand.
- Without an expected type, a unary-negative integer literal is checked as `i32`, including the
  `-2^31` minimum case. A larger magnitude does not cause inference of a wider type.
- The signed-minimum case becomes one checked integer constant. It does not first construct an
  out-of-range positive value and does not execute a runtime negation overflow.
- If no context fixes the type, the literal becomes `i32`.
- Assigning an out-of-range literal is a type error.
- Non-literal integer values are not implicitly converted between integer types.
- Float literals are not supported.

Examples:

```nct
let a = 10        // i32
let b: u64 = 10   // u64
let c: u8 = 300   // error: literal out of range
let d = 0xFF_FF   // i32
let e: u8 = 0b1010
let minimum: i8 = -128
let too_small: i8 = -129 // error

let x: i32 = 10
let y: u64 = x    // error: no implicit integer conversion
```

## Numeric Operations and Conversions

Numeric operations do not perform implicit integer conversion.

Rules:

- Integer binary arithmetic uses operands of the same integer type.
- Shift operators use operands of the same integer type. Integer literals on the right side may be contextually typed by the left operand type when the value fits.
- Shift expressions return the left operand type.
- Left shift moves the fixed-width bit pattern toward the most-significant end, discards bits that
  leave the width, and fills low bits with zero. Discarded bits do not cause an arithmetic overflow
  trap.
- Unsigned right shift fills high bits with zero. Signed right shift uses two's-complement
  arithmetic shift and fills high bits with the original sign bit.
- A zero shift count leaves the value unchanged. A negative count or a count greater than or equal
  to the left operand's bit width traps before any machine shift is executed.
- Signed division truncates the mathematical quotient toward zero. Signed remainder satisfies
  `a = (a / b) * b + (a % b)`, has absolute value less than the absolute value of `b`, and is zero
  or has the same sign as dividend `a`. Unsigned division and remainder use the ordinary
  non-negative quotient and remainder.
- For a signed type, both `minimum / -1` and `minimum % -1` trap as division-family overflow even
  though the mathematical remainder would be zero.
- Integer literals may take an expected integer type when the value fits.
- Non-literal integer values are not implicitly converted.
- `bool` does not implicitly convert to or from integer types.
- Explicit integer conversion uses `expr as Type`.
- Integer `as` is allowed only for lossless conversions.
- Narrowing integer conversions are not allowed with `as`.
- Signedness-changing integer conversions are not allowed with `as` unless the target type can represent every value of the source type.
- On the ARM64 macOS target, `usize` has the same range as `u64`, and `isize` has the same range as `i64` for conversion checking.

The same `as` expression can explicitly select a one-step type-owned borrow coercion when the
source is already borrowed. That distinct contract is specified in
[Borrow Coercions](22-borrow-coercions.md); it does not relax the numeric rules above.

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

`checked` and `truncate` are explicit numeric conversion APIs declared as construction functions by
the active standard-library package. Their names have no special grammar meaning:

```nct
construct u8 {
    pub func checked(value: u64): Self?
    pub func truncate(value: u64): Self
}
```

Arithmetic trap rules:

- Overflow in normal integer arithmetic traps.
- Wrapping arithmetic must use an explicit wrapping API.
- Division by zero traps.
- Remainder by zero traps.
- Signed division-family overflow traps for both `/` and `%` when the operands are the minimum
  signed value and `-1`.
- Shift counts greater than or equal to the bit width of the shifted value trap.
- Shift counts must be non-negative.
- Left-shift bit loss is not integer overflow. It follows the fixed-width bit-shift rule and does
  not trap.

Trap semantics are specified in [Control Flow](03-control-flow.md#never-and-reachability). These arithmetic safety checks are always-on for every build mode; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes).

The exact names for wrapping arithmetic APIs belong to the primitive numeric API surface, not to the operator grammar.

## Operators, Comparison, and Precedence

Nocter has a closed operator grammar. An `instance` may define equality, strict ordering,
readonly/readwrite indexing, and readonly/readwrite/owned expansion with fixed declaration shapes.
Equality uses:

```nct
instance Text {
    pub operator (&self == other: &Self): bool {
        return self.bytes == other.bytes
    }
}
```

The declaration shape is fixed: both operands are readonly borrows of the same `Self` type and the
result is `bool`. It may be private or use an ordinary `pub` boundary. `!=` cannot be declared; it
negates the selected `==` result. Nocter does not derive structural equality.

Readonly and readwrite indexing use separate declarations:

```nct
instance Buffer<T> {
    pub operator (&self[index: usize]): &T {
        return &self.values[index]
    }

    pub operator (&+self[index: usize]): &+T {
        return &+self.values[index]
    }
}
```

An index declaration always returns an element borrow. It therefore defines a place, not a
value-producing or partial lookup operation. Use an ordinary method returning `&T?` for partial
lookup. The declaration body owns bounds and failure policy; the compiler does not add a second
bounds check around a source-defined operation.

Arrays, slices, and `str` are directly indexable. For a nominal receiver, selection first checks an
accessible declaration on the original type. If none applies, selection may use one accessible
borrow coercion whose target is directly indexable or owns an applicable index declaration.
Coercions do not chain, and multiple viable coercion targets are ambiguous. Readonly access uses a
readonly operation; assignment and `&+values[index]` require a readwrite operation and writable
receiver storage.

Comparison rules:

- Primitive equality is available for `bool`, matching integer types, and matching payloadless enum
  types.
- Nominal and view equality selects an accessible equality declaration from the left type.
- Equality may apply one readonly borrow coercion to each operand. An exact left declaration wins
  before coerced candidates; multiple remaining coercion candidates are ambiguous.
- Owned operands are implicitly borrowed for the selected readonly equality call and remain usable.
- `str` owns source-defined equality. The standard `String` coercion therefore supports all four
  `str`/`String` readonly combinations without duplicating the comparison algorithm.
- Struct equality is not automatically generated.
- Payload-carrying enum equality is not supported. Use `match` or `if expr is Pattern`.
- `<`, `<=`, `>`, and `>=` are ordering comparisons.
- Matching integer operands have primitive ordering.
- Other types may own strict ordering through
  `operator (&self < other: &Self): bool`; the complete declaration, generic-requirement,
  derivation, coercion, and evaluation rules are specified in
  [Strict Ordering Operators](24-ordering-operators.md).

Logical rules:

- `&&` requires `bool` operands and returns `bool`.
- `||` requires `bool` operands and returns `bool`.
- `&&` short-circuits: the right operand is evaluated only when the left operand is `true`.
- `||` short-circuits: the right operand is evaluated only when the left operand is `false`.
- `!expr` requires `expr: bool` and returns `bool`.

Unary numeric rules:

- `-expr` requires a signed numeric operand.
- Unary `+expr` is not part of the language.

The complete precedence, associativity, move-place, outcome-suffix, recovery, and primary-expression
grammar is centralized under [Expression Precedence](25-syntactic-grammar.md#expression-precedence).
In particular, unary borrowing binds before `as`, one ungrouped expression layer accepts one
outcome suffix, and `move place?` moves the complete place before applying that suffix.

Assignment remains a statement rather than an expression. `..<` remains confined to a range
`for` header. `if` and `match` are primary control expressions. `condition ? then : else` and
`enum_expr ?{ ... }` have no productions. `&&`, `||`, `otherwise`, `if`, and `match` evaluate only
the required operand, fallback, branch, or arm.

Example:

```nct
if count > 0 && state == ScanState.inside_word {
    ...
}
```

## Structs and Value Construction

Struct values may be constructed with explicit named-field struct literals when the structural
entry is accessible.

```nct
pub struct User {
    pub id: u64
    name: String
}
```

```nct
let user = User {
    id: 1,
    name: String.copy("alice"),
}
```

Rules:

- Struct literal syntax is `Type { field: value, ... }`.
- A struct may declare zero fields. Its structural literal is `Type {}` under the same visibility
  and construction-surface rules as any other struct.
- Struct literal fields are comma-delimited and may use one trailing comma on any layout.
- The type in a struct literal must name a struct type. A generic owner may use complete explicit
  type arguments or infer all of them under
  [Generic Owner Arguments](19-construction-surfaces.md#generic-owner-arguments).
- Every field must be initialized exactly once.
- Field order in the literal is free.
- Unknown fields are compile errors.
- Duplicate fields are compile errors.
- Field initializer expressions are evaluated left to right in the order written in the literal.
- Field initializer expressions follow normal ownership, move, copy, borrow, and postfix `?` rules.
- If a later field initializer fails through postfix `?`, already initialized owned field values are dropped in reverse initialization order before the failure propagates.
- Private fields may be initialized only in their authored source and sources that include it
  directly.
- Public fields may be initialized from other modules.
- Construction entries cannot be overloaded.
- Field default values, struct update syntax, positional structs, and tuple structs are not supported.
- A `construct` declaration groups type-owned construction functions and typed literals. Each
  member has its own visibility, and the declaration may make structural construction
  source-private. See [Construction Surfaces](19-construction-surfaces.md).
- Names such as `new`, `init`, and `create` are ordinary construction-function names when declared
  inside `construct`. The compiler does not special-case them.

When initialization logic or validation is needed, place a public construction function in the
type's `construct` declaration.

```nct
construct User {
    pub default func create(id: u64, name: String): Self {
        return User {
            id: id,
            name: move name,
        }
    }
}
```

Outside the private field's direct source-access boundary, a struct with private fields can be
created only through public APIs exposed by its module.

```nct
let user = User.create(1, String.copy("alice"))
```

## Enums and Variant Construction

Enums represent finite variants and may carry data.

```nct
enum AppError {
    missing_path
    open_failed(path: &str)
}
```

Rules:

- An enum must declare at least one variant. A zero-variant enum is invalid and does not define a
  nominal uninhabited value type.
- Enum variant names use snake_case.
- Variants may carry zero or more payload values.
- Variant payload declarations and constructor arguments are comma-delimited and may use one
  trailing comma on any layout.
- Payloadless variants are constructed as `EnumName.variant_name`.
- Payload variants are constructed as `EnumName.variant_name(args...)`.
- Every enum uses a `u8` ABI tag and must declare between 1 and 256 variants, inclusive, whether
  or not its variants carry payloads.
- Variant construction requires the payload arity and types to match the variant declaration.
- Variant payload arguments are evaluated left to right.
- Variant constructors are qualified with the enum name, such as `AppError.open_failed(path)`.
- Variant constructors must be qualified.
- A generic enum owner may use complete explicit type arguments or infer all of them under
  [Generic Owner Arguments](19-construction-surfaces.md#generic-owner-arguments).
- Variant constructors are not ordinary functions and are not magic identifier names; they are generated by the enum declaration.
- Enum variants and construction functions share the type member namespace. Defining a
  construction function with the same member name as a variant is a compile error.
- If an enum is public, its variants are public.
- Per-variant visibility is not supported.

Examples:

```nct
let state = ScanState.inside_word
let error = AppError.open_failed(path)
```

`match` is the control-flow form for enum pattern matching.

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

Enum patterns are shallow and positional. Their source form is centralized under
[Control Expressions and Enum Patterns](25-syntactic-grammar.md#control-expressions-and-enum-patterns).

The payload list uses the common comma-delimited-list grammar. A payloadless variant uses the first
form. A payload-bearing variant uses the second form and supplies exactly one slot for every
declared payload field.

```nct
enum Pair {
    values(left: String, right: String)
}

match &pair {
    Pair.values(_, right) {
        inspect(right) // right: &String
    }
}
```

The pattern target chooses how payload names are bound. Borrowed matching inspects an enum without
extracting its payload:

```nct
var message = next_message()

match &message {
    Message.text(text) {
        inspect(text) // text: &String
    }
    _ {
        ...
    }
}

match &+message {
    Message.text(text) {
        text.clear() // text: &+String
    }
    _ {
        ...
    }
}

match move message {
    Message.text(text) {
        consume(move text) // text: String
    }
    _ {
        ...
    }
}
```

Rules:

- `match` may be used as a statement or as an expression.
- Match arms use `Pattern { ... }`.
- A variant pattern must use the exact enum qualifier and variant name selected by the target enum
  type.
- Payload pattern slots are positional and their count must equal the variant payload arity.
- An identifier slot introduces one branch-local binding for the payload field at that position.
  It does not need to repeat the field's declaration name.
- `_` always occupies exactly one payload position and introduces no binding. Ignoring every field
  of a multi-payload variant requires one `_` for each field, such as `Pair.values(_, _)`.
- `Pair.values(_)` is therefore an arity error when `values` has two payload fields. `_` never
  abbreviates an entire payload list.
- Nested patterns, literal patterns, binding modifiers, field-name patterns, and rest patterns are
  not supported.
- `_ { ... }` is the fallback arm and matches any remaining value.
- Each enum variant may appear in at most one explicit arm of a `match`. Repeating the same
  qualified variant is a compile error even when its payload slots use different binding names or
  `_` positions.
- Enum variant patterns are tag patterns, not value refinements. Payload binding names and `_`
  control projection only; they cannot make two arms for the same variant disjoint.
- A `match` may have at most one `_` fallback arm.
- The `_` fallback arm must be the last arm.
- A `_` fallback arm remains valid when the preceding explicit arms already cover every current
  variant. This permits an intentional fallback for variants added by a future dependency version.
- Such a currently unreachable fallback is still resolved and checked as an ordinary arm. Its body
  must satisfy syntax, name, type, ownership, provenance, and selected-target rules, and its body
  result participates in `match` result-type compatibility.
- A currently exhaustive fallback has no runtime execution path for the current enum definition.
  The compiler may omit its machine code after checking, but it must not use that fact to accept an
  otherwise invalid body or a different result type.
- When `match` is used as an expression, each selected arm body result is the expression value.
- A `match` expression without `_` must cover all variants to avoid a `void` missing-branch type.
- Match expression arm result types must be compatible. A `never` arm is compatible with the other result type.
- `match` without `_` is treated as a terminating statement when every enum variant is covered by an explicit arm and every arm terminates.
- `match` with `_` is treated as a terminating statement when every explicit arm and the `_` arm terminate.
- Every enum payload field may use any sized type that is valid as a struct field. Construction,
  local storage, arguments, returns, assignment, optional/fallible wrapping, and pattern matching
  apply recursively to payload aggregates without a separate runtime type allowlist.
- A pattern target expression is evaluated exactly once before its tag is tested. An ordinary
  expression that produces a new owned enum temporary may be matched directly.
- A pattern target whose type is `Enum`, `&Enum`, or `&+Enum` selects one of the binding modes in
  the table below. Pattern matching dereferences a borrowed target only for tag inspection and
  payload projection; it does not introduce a general implicit dereference conversion.

| Pattern target | Payload name type | Effect on the target |
| --- | --- | --- |
| New owned enum temporary | declared payload type | Consumes the temporary |
| Existing enum place without `move` | declared payload type, only when that payload is copyable | Copies the named payload; retains the enum place |
| Readonly borrow expression of type `&Enum` | `&Payload` | Retains the enum and creates a readonly payload borrow |
| Readwrite borrow expression of type `&+Enum` | `&+Payload` | Retains the enum and creates an exclusive readwrite payload borrow |
| `move place` | declared payload type | Consumes the enum place |

- The borrowed modes apply both to an explicit target such as `match &value` or
  `match &+value` and to any target expression already typed as `&Enum` or `&+Enum`. Using an
  existing borrow as a pattern target is a use of that borrow, not an ownership transfer.
- Every payload name in one pattern uses the target's binding mode. A borrowed target therefore
  binds even a copyable payload as `&Payload` or `&+Payload`; it never performs a hidden payload
  copy. Code that needs owned copies matches an existing enum place without a borrow.
- Creating an `&+Enum` target follows the ordinary writable-place and exclusivity rules. Payload
  borrows derived from a borrowed target carry the target borrow's provenance and keep that borrow
  active through their last source-level use. A payload borrow may be returned or stored only when
  the ordinary borrow and provenance rules permit it.
- An existing enum place without `move` may bind copyable payloads even when another variant makes
  the enum type move-only. Naming a move-only payload from that target is an error. A payloadless
  pattern or `_` payload may still inspect the tag without consuming the place.
- Binding owned move-only payloads from an existing local, parameter, or named struct field
  requires an explicit `move` target, such as `match move result` or
  `match move holder.result`. The ordinary move-place restrictions still apply; indexes,
  dereferences, and computed projections are not move sources.
- A newly produced owned enum temporary, including a call result, variant constructor, control
  expression result, or value produced through postfix `?`, postfix `!`, `catch`, or `otherwise`,
  is already owned by the pattern operation and does not use `move`.
- An owned target is consumed as a whole into pattern-operation temporary storage before arm
  execution. Named payload bindings assume their fields' drop obligations. Unnamed fields and the
  complete active payload in a fallback arm remain in that storage. Its residual initialized
  fields are dropped by the ordinary statement-temporary rules after the selected arm is evaluated,
  or by early-exit cleanup if the arm exits the statement. No field of a consumed enum is dropped
  twice.
- If an owned enum has a type-owned drop declaration and an explicit arm transfers any named
  move-only payload, the drop body observes the complete enum exactly once after tag selection and
  before any payload leaves pattern-operation storage. The residual cleanup later drops only
  unnamed initialized payload fields and does not call the type-owned drop body again.
- If such an arm binds only copyable payloads, those bindings copy their values and the complete
  enum remains in pattern-operation storage. Its ordinary residual cleanup invokes the type-owned
  drop body and then drops the active payload. Fallback and implicit non-match paths likewise keep
  the complete enum until ordinary cleanup.
- Enum cleanup reads the active tag and drops only initialized fields of that variant. Fields drop
  in reverse payload declaration order and recursively use the same struct, enum, fixed-array, and
  outcome cleanup rules. Fixed-array elements drop in reverse index order.
- Scope-end cleanup, parameter cleanup, explicit discard initializers, call-result temporaries,
  assignment replacement, and partial control-flow cleanup all use the same active-variant rule.
- Payload names in a pattern are bound only inside that arm block.
- `_` inside a payload pattern, such as `AppError.open_failed(_)`, occupies exactly one declared
  payload position without introducing a binding. For a consumed target, that unnamed owned field
  remains under pattern-operation temporary cleanup. For a borrowed or retained target, the field
  is neither copied nor moved.
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

`if enum_expr is Pattern` checks one enum pattern.

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
- `if` pattern targets use the same owned, copied, readonly-borrowed, readwrite-borrowed, and moved
  binding modes as `match`.
- Payload names are bound only inside the then body.
- `if enum_expr is Enum.variant(_)` checks only the variant of a one-payload enum case and ignores
  that payload without introducing a binding. A multi-payload variant requires one slot per field.
- `else` may be used for the non-matching case.
- `else` is optional.
- `else if enum_expr is Pattern` is allowed.
- `else if enum_expr is Pattern` is equivalent to `else { if enum_expr is Pattern { ... } }`.
- Payload names are not available in `else` or later `else if` branches.
- `if expr is Pattern` may be used as a statement or as an expression with the same body-result rules as ordinary `if`.
- `if is` does not apply to fallible values `T!`.
- `if is` does not apply to optional values `T?`.
