# Control Flow

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Functions

Functions are declared with `func`.

```nct
func scan_words(text: StringView): WordStats {
    ...
}
```

Names do not define special behavior. A function named `main`, `init`, or `new` is ordinary unless the language defines a syntactic rule around a declaration. `drop` is reserved for destructor declarations and explicit drop statements.

Parameters are written as `name: Type`. `var name: Type` parameters are not part of v0. Parameter binding and ownership rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#function-parameters).

Return checking:

- A `void` function may use bare `return` or reach the end of the function body.
- A non-fallible, non-optional function returning a non-`void` type must return a value on every reachable normal path, unless the path terminates with `never`.
- A fallible function `T!E` must return a success value, `fail` with an error, or terminate with `never` on every reachable path.
- An optional function `T?` must return a present value, `return none`, or terminate with `never` on every reachable path.
- `program(): void` and `program(): i32` follow the same return checking rules as functions with those return types.

Return value ownership, move, borrow, and view rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#return-values).

## Function Calls and Arguments

Adopted: v0 uses positional arguments only.

```nct
func copy(allocator: &+Allocator, source: StringView): String!AllocError {
    ...
}

let text = try String.copy(&+allocator, "hello")
```

Rules:

- Function, associated function, method, and primitive calls use positional arguments.
- Argument expressions are matched to parameters by position.
- Argument count must match parameter count exactly.
- Each argument must type-check against the corresponding parameter type under the normal contextual typing, ownership, move, copy, and borrow rules.
- Function call arguments are evaluated left to right in the order written.
- Method receiver expressions are evaluated before method arguments.
- Method arguments are then evaluated left to right in the order written.
- Parameter names are not part of call syntax.
- Named arguments are not part of v0.
- Default parameters are not part of v0.
- Variadic functions are not part of v0.
- Function, associated function, and method overload by type, arity, or return type is not part of v0.
- A duplicate callable name in the same namespace is a compile error.
- A trailing comma is allowed in multi-line parameter lists and multi-line argument lists.
- A trailing comma is not allowed in single-line parameter lists or single-line argument lists in v0.

Examples:

```nct
pub func copy(
    allocator: &+Allocator,
    source: StringView,
): String!AllocError {
    ...
}
```

```nct
let text = try String.copy(
    &+allocator,
    "hello",
)
```

Invalid in v0:

```nct
String.copy(allocator: &+allocator, source: "hello") // named arguments

func open(path: StringView = "input.txt"): File!IOError {
    ...
}

func print_all(parts: StringView...): void {
    ...
}

func open(path: StringView): File!IOError {
    ...
}

func open(path: StringView, mode: OpenMode): File!IOError {
    ...
}
```

Use a configuration struct when an API has many boolean or optional choices.

```nct
pub struct OpenOptions {
    pub read: bool
    pub write: bool
    pub create: bool
}

let file = try File.open_with(path, OpenOptions{
    read: true,
    write: false,
    create: false,
})
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
func max(a: i32, b: i32): i32 {
    if a > b {
        return a
    }

    return b
}
```

Rules:

- `if condition { ... }` is a statement.
- `if condition { ... } else { ... }` is a statement.
- `if let name = optional_expr { ... }` is a statement.
- `if var name = optional_expr { ... }` is a statement.
- `if enum_expr is Pattern { ... }` is a statement.
- `match enum_expr { ... }` is a statement.
- `for name in start..<end { ... }` is a statement.
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

return match error {
    is AppError.open_failed(path) { 1 }
    else { 0 }
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

## Evaluation Order and Temporaries

Adopted: expression evaluation is left-to-right.

Rules:

- Function call arguments are evaluated left-to-right.
- Method call receiver expressions are evaluated before method arguments.
- For evaluated method arguments, evaluation remains left-to-right.
- Struct literal field initializer expressions are evaluated left-to-right in the order written in the literal, regardless of declaration order.
- Assignment evaluates the right-hand side before replacing the target place. The detailed assignment rules are specified in [Values and Types](02-values-types.md#bindings-and-assignment).
- Operators with conditional evaluation, such as `&&`, `||`, `??`, and `condition ? then : else`, evaluate only the needed operand or branch.
- When an operand or branch is evaluated, its subexpressions still follow the normal left-to-right rule.
- Temporaries are dropped at the end of the current statement in reverse creation order unless ownership is moved into a longer-lived owner.
- Longer-lived owners include local bindings, owned parameters, constructed aggregate values, assigned target places, and returned values.
- Blocks, `if` bodies, `match` arms, and loop bodies create scopes.
- Initialized local values are dropped at scope end in reverse declaration order.
- Maybe initialized local values use compiler-generated conditional drop at scope end.
- `try`, `return`, `fail`, `break`, and `continue` first drop temporaries already created by the current statement, then run the required normal or conditional drops for scopes they leave.
- Borrows and borrow-like views derived from temporaries cannot escape the statement.
- Temporary lifetime extension is not part of the initial design.

Examples:

```nct
let result = make_a().combine(make_b())
```

Evaluation order:

1. `make_a()`
2. method receiver preparation for `.combine`
3. `make_b()`
4. method call
5. statement-end temporary drops

This is invalid:

```nct
let view = (try String.copy(allocator, "abc")).view()
```

`String.copy(...)` produces a temporary owned `String`. `.view()` borrows from that temporary. The temporary would be dropped at the end of the statement, so the `StringView` cannot be stored in `view`.

Write this instead:

```nct
var text = try String.copy(allocator, "abc")
let view = text.view()
```

A method receiver borrow lasts only for the call unless the method returns a value whose type carries a borrow-like lifetime tracked by the compiler.

```nct
try file.write("hello")
```

The call above creates a temporary readwrite borrow of `file` for the duration of the call and ends that borrow after the call.

Fallible temporary receivers must make each fallible step explicit:

```nct
try (try File.open(path)).write("hello")
```

If `File.open(path)` fails, no `File` temporary exists. If `write` fails, the temporary `File` produced by `File.open(path)` is dropped before the failure propagates. If `write` succeeds, the temporary `File` is dropped at the end of the statement.

## Loops

Adopted: the initial loop forms are `while`, `loop`, range `for`, `break`, and `continue`.

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
- `for name in start..<end { ... }` loops over a half-open integer range.
- `in` is a reserved keyword used by the `for` header.
- `..<` is the half-open range token in the initial `for` header syntax.
- `start` and `end` are evaluated once, left-to-right, before the loop begins.
- `start` and `end` must have the same integer type after literal contextual typing.
- The loop variable has the same type as `start` and `end`.
- The loop variable is an immutable binding scoped to the loop body.
- If `start >= end`, the loop body runs zero times.
- The step is always `+1` in the initial design.
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
- reverse iteration and custom step syntax

Collection iteration is not part of the initial `for` syntax. The compiler must not lower `for item in items` into calls to methods named `iter` or `next`.

Use range `for` with indexing:

```nct
for i in 0..<bytes.len() {
    let byte = bytes[i]
    consume(byte)
}
```

Or use explicit ordinary methods when a standard-library iterator type exists:

```nct
var iter = bytes.iter()

loop {
    if let byte = iter.next() {
        consume(byte)
    } else {
        break
    }
}
```

## Never and Reachability

Adopted: `never` represents a computation that does not return normally.

`never` is not an ordinary value-carrying type. It is the type of control flow that terminates the current path instead of producing a value.

Typical uses:

- `trap(): never`
- `std/process.abort(): never`
- `std/process.exit(code): never`
- an infinite event loop that has no reachable `break`
- an explicit unreachable-code marker in the standard library

`trap` is the primitive boundary for non-recoverable program defects. The compiler may also generate traps for checked operations such as out-of-bounds indexing or invalid arithmetic.

`abort` and `exit` are standard-library process APIs. They are not compiler primitives.

`panic` is not a language feature in v0. No stack unwinding mechanism is part of v0.

Example:

```nct
import std/process as process

func require_path(path: StringView?): StringView {
    if let value = path {
        return value
    }

    process.abort()
}
```

Rules:

- A function declared as returning `never` must not complete normally.
- A `never` function body must terminate all reachable paths with another `never` call, a non-breaking infinite `loop`, a low-level primitive such as `trap`, a standard-library terminating API such as `abort` or `exit`, or equivalent terminating control flow.
- `return` and `return value` are not valid in a `never` function.
- Falling off the end of a `never` function is a compile error.
- A call whose type is `never` terminates the current control path.
- Code after `return`, `fail`, `break`, `continue`, or a `never` call in the same block is unreachable.
- Unreachable code is a compile-time error in the initial design.
- A `never`-typed expression can appear where another expression type is required because it produces no value.
- `never` cannot be constructed, stored in a variable, used as a field type, or used as an array element type in the initial design.
- Calling a `never` function does not imply stack unwinding, statement-end temporary drops, or caller-scope `drop` execution.
- If cleanup is required before a terminating API such as `exit` or `abort`, the program must perform that cleanup before the `never` call or use a normal `return`, `fail`, `break`, or `continue` path.
- `fail` is recoverable failure and is valid only through fallible type `T!E`.
- `trap` is non-recoverable failure caused by a program defect, violated compiler check, or impossible execution path.
- `abort` is immediate process termination and does not run Nocter cleanup.
- `panic` and stack unwinding are not part of v0.
- `panic` is not reserved. A user-defined function named `panic` is ordinary and has no language-defined behavior.

Example:

```nct
func require_path_short(path: StringView?): StringView {
    return path ?? process.abort()
}
```

The `??` expression above has type `StringView`. The right side does not produce a fallback `StringView`; it terminates the current path.

`never` also satisfies `catch` block termination:

```nct
let file = try File.open(path) catch error {
    process.abort()
}
```

Invalid:

```nct
func invalid(): never {
    return
}

func also_invalid(): i32 {
    process.abort()
    return 0
}
```

The first function returns normally. The second contains unreachable code after a `never` call.

## Safety Checks and Build Modes

Adopted: safety checks are part of Nocter semantics and remain enabled in every build mode.

Build modes may change diagnostics, debug information, and optimization level. They must not change the safety meaning of a valid Nocter program.

Always-on checks:

- Bounds checks for indexing.
- Integer overflow checks for normal arithmetic.
- Division and remainder by zero checks.
- Signed division overflow checks.
- Shift count range checks.
- Invalid live `bool` bit-pattern checks where a value can enter from a primitive or ABI boundary.
- Invalid enum tag checks where a value can enter from a primitive or ABI boundary.
- Reaching `unreachable()` or an equivalent impossible-path marker.

Rules:

- Debug and release builds have the same trap conditions.
- A build mode must not turn a checked operation into undefined behavior.
- The optimizer may remove a safety check only when it proves that the trap condition cannot occur on that path.
- Removing a check is valid only when the source-level observable behavior is unchanged.
- If a check is statically known to fail, the compiler may emit an unconditional trap for that path.
- General user code has no unchecked arithmetic, unchecked indexing, or unchecked enum-tag operation in v0.
- Wrapping arithmetic is not unchecked arithmetic. It must be exposed through explicit numeric APIs.
- Target overlays and compiler primitive lowering may use target-specific machine instructions internally, but that must not expose undefined behavior to ordinary Nocter code.
