# Control Flow

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Functions

Functions are declared with `func`.

```nct
func scan_words(text: &str): WordStats {
    ...
}
```

Names do not define intrinsic language behavior. A function named `init`, `new`, or `drop` is ordinary. A root-file function named `main` is selected as the executable entry point only because the v0 compiler entry setting defaults to `main`; `--entry <name>` can select another root-file function. `drop` is not reserved; inherent destructor declarations and explicit drop statements are contextual source forms.

Parameters are written as `name: Type`. `var name: Type` parameters are not part of v0. Parameter binding and ownership rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#function-parameters).

Return checking:

- A `void` function may use bare `return` or reach the end of the function body.
- A non-fallible, non-optional function returning a non-`void` type must return a value on every reachable normal path, unless the path terminates with `never`.
- A fallible function `T!` must return a success value, return an `error` failure value, or terminate with `never` on every reachable path.
- An optional function `T?` must return a present value, `return none`, or terminate with `never` on every reachable path.
- A fallible optional function `T?!` must return a present success value, `return none` as success absence, return an `error` failure value, or terminate with `never` on every reachable path.
- `func main(): i32!` follows the same source-level return checking rules as a function returning `i32!`; success returns `i32`, and failure returns an `error` value.
- `func main(): void` and `func main(): i32` follow the same return checking rules as functions with those return types.

Return value ownership, move, borrow, and view rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#return-values).

## Function Calls and Arguments

Adopted: v0 uses positional arguments only.

```nct
func copy(allocator: &+Allocator, source: &str): String! {
    ...
}

let text = String.copy(&+allocator, "hello")?
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
    source: &str,
): String! {
    ...
}
```

```nct
let text = String.copy(
    &+allocator,
    "hello",
)?
```

Invalid in v0:

```nct
String.copy(allocator: &+allocator, source: "hello") // named arguments

func open(path: &str = "input.txt"): File! {
    ...
}

func print_all(parts: &str...): void {
    ...
}

func open(path: &str): File! {
    ...
}

func open(path: &str, mode: OpenMode): File! {
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

let file = File.open_with(path, OpenOptions{
    read: true,
    write: false,
    create: false,
})?
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

Adopted: enum pattern value selection uses the pattern conditional expression.

```nct
return error ?{
    AppError.missing_path : missing_code()
    AppError.open_failed(path) : code_for(path)
    : unknown_code()
}
```

Rules:

- `enum_expr ?{ ... }` is an expression.
- Arms use `Pattern : expression`.
- The fallback arm is written as `: expression` and is required in v0.
- Only the selected arm expression is evaluated.
- Payload bindings are visible only in their arm expression.
- `?{}` is for enum pattern selection; it is not the optional propagation postfix `?` and not the optional default operator `??`.

## Statements and Expressions

Adopted: the initial language is statement-centered.

`if`, `match`, and block bodies do not produce values in the initial design. Functions return values with explicit `return`. Fallible functions fail by returning an `error` value. Optional functions return absence with explicit `return none`. Enum pattern value selection uses `?{}` instead of expression-valued `match`.

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
- `let name = optional_expr else { ... }` is a declaration statement.
- `var name = optional_expr else { ... }` is a declaration statement.
- `if enum_expr is Pattern { ... }` is a statement.
- `match enum_expr { ... }` and `match enum_expr { ... else { ... } }` are statements.
- `for name in start..<end { ... }` is a statement.
- Blocks `{ ... }` do not return trailing expression values.
- `return value` is required to return a value from a function.
- `return error_value` is required to return a failure from a fallible function.
- `return none` is required to return absence from an optional function, or success absence from a fallible optional function.
- Optional `let ... else` and `var ... else` declarations must use an `else` block that terminates the current control path.
- Expression-valued `if`, `match`, and block forms are deferred.
- Use `enum_expr ?{ ... }` when an enum pattern dispatch must produce a value.

Invalid in the initial design:

```nct
let value = if condition {
    a
} else {
    b
}

return match error {
    AppError.open_failed(path) { 1 }
    else { 0 }
}
```

Use the ternary conditional operator for boolean value selection.

```nct
let value = condition ? a : b
```

Use `?{}` for enum pattern value selection.

```nct
return error ?{
    AppError.open_failed(path) : 1
    : 0
}
```

Statement separation:

- Semicolons are not part of the initial grammar.
- One statement per line is the normal style.
- A newline separates statements where the grammar can end a statement.
- A closing brace `}` ends the current block or arm.
- Multi-line expressions are allowed only where the expression syntax clearly continues, such as inside calls, literals, or parenthesized expressions.
- The lexical source text and comment rules are specified in [Lexical Grammar](13-lexical-grammar.md).

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
- Postfix `?`, `return`, `break`, and `continue` first drop temporaries already created by the current statement, then run the required normal or conditional drops for scopes they leave.
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
let view = (String.copy(allocator, "abc")?).view()
```

`String.copy(...)` produces a temporary owned `String`. `.view()` borrows from that temporary. The temporary would be dropped at the end of the statement, so the `&str` cannot be stored in `view`.

Write this instead:

```nct
var text = String.copy(allocator, "abc")?
let view = text.view()
```

A method receiver borrow lasts only for the call unless the method returns a value whose type carries a borrow-like lifetime tracked by the compiler.

```nct
file.write_text("hello")?
```

The call above creates a temporary readwrite borrow of `file` for the duration of the call and ends that borrow after the call.

Fallible temporary receivers must make each fallible step explicit:

```nct
(File.open(path)?).write_text("hello")?
```

If `File.open(path)` fails, no `File` temporary exists. If `write_text` fails, the temporary `File` produced by `File.open(path)` is dropped before the failure propagates. If `write_text` succeeds, the temporary `File` is dropped at the end of the statement.

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

Optional loop conditions may bind a present optional value:

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
}
```

Rules:

- `while condition { ... }` requires `condition` to have type `bool`.
- `while let name = expr { ... }` applies only when `expr` has type `T?`.
- `while var name = expr { ... }` applies only when `expr` has type `T?`.
- If `expr` is present, the contained `T` value is bound to `name` and the loop body runs.
- If `expr` is `none`, the loop exits normally.
- `while let` creates an immutable binding scoped to the loop body.
- `while var` creates a mutable binding scoped to the loop body.
- `while let` and `while var` do not use `some` / `none` patterns.
- For move-only `T`, each successful iteration consumes the optional value and moves the contained value into the binding.
- For copy `T`, the contained value may be copied according to normal copy rules.
- Borrowed optional projections such as `while let name = &place` and `while var name = &+place` are not part of v0.
- Optional borrow values such as `(&T)?` are allowed. For example, `while let item = iter.next()` is valid when `next()` returns `(&T)?`.
- `loop { ... }` is an infinite loop unless exited by `break`, `return`, or another terminating control flow.
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
- mutable element iteration over `&+[T]`
- compiler-lowered iteration syntax that treats ordinary names such as `iter` or `next` specially
- reverse iteration and custom step syntax

Collection iteration is not part of the initial `for` syntax. The compiler must not lower `for item in items` into calls to methods named `iter` or `next`. Collection iteration is expressed through ordinary standard-library iterator types and optional loop conditions.

Use range `for` with indexing:

```nct
for i in 0..<bytes.len() {
    let byte = bytes[i]
    consume(byte)
}
```

Or use explicit ordinary methods:

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
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
use std/process as process

func require_path(path: &str?): &str {
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
- Code after `return`, `break`, `continue`, or a `never` call in the same block is unreachable.
- Unreachable code is a compile-time error in the initial design.
- A `never`-typed expression can appear where another expression type is required because it produces no value.
- `never` cannot be constructed, stored in a variable, used as a field type, or used as an array element type in the initial design.
- Calling a `never` function does not imply stack unwinding, statement-end temporary drops, or caller-scope `drop` execution.
- If cleanup is required before a terminating API such as `exit` or `abort`, the program must perform that cleanup before the `never` call or use a normal `return`, `break`, or `continue` path.
- Fallible failure is recoverable failure and is valid only through fallible type `T!`.
- `trap` is non-recoverable failure caused by a program defect, violated compiler check, or impossible execution path.
- `abort` is immediate process termination and does not run Nocter cleanup.
- `panic` and stack unwinding are not part of v0.
- `panic` is not reserved. A user-defined function named `panic` is ordinary and has no language-defined behavior.

Example:

```nct
func require_path_short(path: &str?): &str {
    return path ?? process.abort()
}
```

The `??` expression above has type `&str`. The right side does not produce a fallback `&str`; it terminates the current path.

`never` also satisfies `catch` block termination:

```nct
let file = File.open(path) catch error {
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
