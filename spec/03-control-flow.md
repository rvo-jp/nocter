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

Names usually do not define intrinsic language behavior. A function named `init`, `new`, or `drop` is ordinary. A selected executable module's function named `main` is the executable entry point. `drop` is not reserved; inherent destructor declarations and explicit drop statements are contextual source forms.

Parameters are written as `name: Type`. Mutable parameter bindings are not supported. Parameter binding and ownership rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#function-parameters).

Return checking:

- A `void` function may use bare `return` or reach the end of the function body.
- A non-fallible, non-optional function returning a non-`void` type must produce a value through the function body result or explicit `return` on every reachable normal path, unless the path terminates with `never`.
- A fallible function `T!` must produce a success value through the function body result or explicit `return`, return an `error` failure value, or terminate with `never` on every reachable path.
- A fallible `void!` function may use bare `return` or reach the end of the
  function body for success; `return error_value` returns failure.
- An optional function `T?` must produce a present value through the function body result or explicit `return`, `return none`, or terminate with `never` on every reachable path.
- A fallible optional function `T?!` must produce a present success value through the function body result or explicit `return`, `return none` as success absence, return an `error` failure value, or terminate with `never` on every reachable path.
- `func main(): i32!` and `func main(): usize!` follow the same source-level return checking rules as functions returning those fallible types; success returns `i32` or `usize`, and failure returns an `error` value.
- `func main(): void!` follows the same source-level return checking rules as a
  function returning `void!`; success returns no value, and failure returns an
  `error` value.
- `func main(): void`, `func main(): i32`, and `func main(): usize` follow the same return checking rules as functions with those return types.

Return value ownership, move, borrow, and view rules are specified in [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md#return-values).

## Function Calls and Arguments

Calls use positional arguments only.

```nct
func copy(allocator: &+Allocator, source: &str): String! {
    ...
}

let text = String.copy(&+allocator, "hello")?
```

Rules:

- Top-level function, construction function, method, and primitive calls use positional arguments.
- Argument expressions are matched to parameters by position.
- Argument count must match parameter count exactly.
- Each argument must type-check against the corresponding parameter type under the normal contextual typing, ownership, move, copy, and borrow rules.
- A callable's own generic type arguments are always inferred from its receiver, arguments,
  contextual closures, and expected result type. They are not written at the call site.
- Function call arguments are evaluated left to right in the order written.
- Method receiver expressions are evaluated before method arguments.
- Method arguments are then evaluated left to right in the order written.
- Parameter names are not part of call syntax.
- Named arguments, default parameters, and variadic functions are not supported.
- Top-level functions, construction functions, and methods cannot be overloaded by type, arity, or
  return type.
- A duplicate callable name in the same namespace is a compile error.
- A trailing comma is allowed in multi-line parameter lists and multi-line argument lists.
- A trailing comma is not allowed in single-line parameter lists or single-line argument lists.

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
    "hello",
)
```

Invalid:

```nct
String.copy(source: "hello") // named argument

func open(path: &str = "input.txt"): File! {
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
Variadic capture is not function-parameter syntax. Literal definitions use their own `...items` capture form.

```nct
pub struct OpenOptions {
    pub read: bool
    pub write: bool
    pub create: bool
}

let file = File.open_with(path, OpenOptions {
    read: true,
    write: false,
    create: false,
})?
```

## Body Results and Control Expressions

Braced bodies have a unified result form.

```nct
{
    stmt1
    stmt2
    result
}
```

The last expression in a body is the body result. A short body may be written on one line.

```nct
{ expr }
```

Rules:

- A body contains zero or more statements followed by an optional result expression.
- The result expression is written without `return`.
- An expression used as a non-final statement must have type `void` or `never`. A non-final
  expression of any value-producing type is an error even when its value would be immediately
  droppable.
- `let _ = expression` explicitly evaluates and discards a value-producing expression. It is a
  statement, not a binding and not a body result.
- Bindings introduced by earlier statements in the same body are in scope for the result expression.
- A body without a result expression has type `void` unless all reachable paths terminate with `return`, `break`, `continue`, or `never`.
- A body whose reachable paths all terminate has type `never`.
- Function, method, drop, `if`, `if is`, `match`, loop, and `catch` bodies use this same body form.
- A function or method body result is a return value for the declared return type.
- Explicit `return` remains valid when an early exit is clearer or required.
- A `void` function may have no body result and may reach the end of the body.
- A non-`void` function must either have a body result assignable to the declared return type or guarantee an explicit return or `never` on every reachable path.

`if`, `if is`, and `match` can be used as expressions.

```nct
func max(a: i32, b: i32): i32 {
    if a > b {
        a
    } else {
        b
    }
}
```

Rules:

- `if condition { ... }` may be used as a statement. Without `else`, its value type is `void`.
- `if condition { ... } else { ... }` may be used as an expression when the branch body result types are compatible.
- The `if` condition expression must have type `bool`.
- Only the selected `if` branch is evaluated.
- `if enum_expr is Enum.variant { ... }` follows the same statement/expression rules as ordinary `if`.
- Payload names introduced by `if expr is Enum.variant(payload)` are visible only inside the then body.
- `if expr is Enum.variant(_)` checks the variant and discards the payload
  without introducing a binding.
- `else if ...` is syntax for an `else` body whose result is another `if` expression.
- `match enum_expr { ... }` and `match enum_expr { ... _ { ... } }` may be used as statements or expressions.
- A `match` expression without `_` must cover all variants to avoid a `void` missing-branch type.
- `match` arm body result types must be compatible when the `match` value is used.
- A `match` `_` fallback arm matches every remaining variant and must be the
  last arm.
- A `never` branch is compatible with the other branch result type.
- Only the selected `match` arm body is evaluated.
- `for name in start..<end { ... }` is a statement.
- `return value` explicitly returns a value from a function.
- `return error_value` explicitly returns a failure from a fallible function.
- `return none` explicitly returns absence from an optional function, or success absence from a fallible optional function.

Examples:

```nct
let value = if condition {
    a
} else {
    b
}

return match error {
    AppError.open_failed(path) { 1 }
    _ { 0 }
}
```

Removed:

- The ternary conditional operator `condition ? then_value : else_value` is not Nocter syntax. Use `if`.
- The pattern conditional expression `enum_expr ?{ ... }` is not Nocter syntax. Use `match`.

Statement separation:

- Semicolons are not statement terminators.
- One statement per line is the normal style.
- A newline separates statements where the grammar can end a statement.
- A single newline does not separate statements when the next line starts with a continuation leader
  such as `.`, a binary-only operator, `as`, `catch`, or `otherwise`.
- A token that can begin an expression, including unary `-`, never acts as a continuation leader.
  When subtraction spans lines, keep `-` at the end of the first line rather than the beginning of
  the second line.
- A blank or comment-only intervening line ends leading-token continuation.
- A closing brace `}` ends the current block or arm.
- Multi-line expressions are allowed only where the expression syntax clearly continues, such as inside calls, literals, or parenthesized expressions.
- The complete continuation-leader set and lexical source text rules are specified in
  [Lexical Grammar](13-lexical-grammar.md#statement-separation).

## Evaluation Order and Temporaries

Expression evaluation is left-to-right.

Rules:

- Function call arguments are evaluated left-to-right.
- Method call receiver expressions are evaluated before method arguments.
- For evaluated method arguments, evaluation remains left-to-right.
- Struct literal field initializer expressions are evaluated left-to-right in the order written in the literal, regardless of declaration order.
- Assignment evaluates the right-hand side before replacing the target place. The detailed assignment rules are specified in [Values and Types](02-values-types.md#bindings-and-mutability).
- Operators and expressions with conditional evaluation, such as `&&`, `||`, `otherwise`, `if`, and `match`, evaluate only the needed operand, branch, or arm.
- When an operand or branch is evaluated, its subexpressions still follow the normal left-to-right rule.
- Temporaries are dropped at the end of the current statement in reverse creation order unless ownership is moved into a longer-lived owner.
- A value produced for `let _ = expression` is consumed by that discard statement. Its active owned
  content is dropped after expression evaluation and before earlier temporaries from the same
  statement are dropped.
- Longer-lived owners include local bindings, owned parameters, constructed aggregate values, assigned target places, and returned values.
- Blocks, `if` bodies, `match` arms, and loop bodies create scopes.
- Initialized local values are dropped at scope end in reverse declaration order.
- Maybe initialized local values use compiler-generated conditional drop at scope end.
- Postfix `?`, `return`, `break`, and `continue` first drop temporaries already created by the current statement, then run the required normal or conditional drops for scopes they leave.
- Borrows and borrow-like views derived from temporaries cannot escape the statement.
- Temporary lifetime extension is not supported.

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
let view = &String.copy("abc") as &str
```

`String.copy(...)` produces a temporary owned `String`. The explicit conversion borrows from that
temporary. The temporary would be dropped at the end of the statement, so the `&str` cannot be
stored in `view`.

Write this instead:

```nct
var text = String.copy("abc")
let view = &text as &str
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

Loop forms are `while`, `loop`, range `for`, collection `for`, `break`, and `continue`.

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

```nct
loop {
    let item = iter.next() otherwise { break }
    consume(item)
}
```

Rules:

- `while condition { ... }` requires `condition` to have type `bool`.
- `while let`, `while var`, `if let`, and `if var` are not Nocter syntax.
- Optional values do not have dedicated loop syntax; use `otherwise { break }` or `otherwise { continue }` inside an ordinary loop when absence controls iteration.
- `loop { ... }` is an infinite loop unless exited by `break`, `return`, or another terminating control flow.
- `for name in start..<end { ... }` loops over a half-open integer range.
- `in` is a reserved keyword used by the `for` header.
- `..<` is the half-open range token in range `for` header syntax.
- `start` and `end` are evaluated once, left-to-right, before the loop begins.
- `start` and `end` must have the same integer type after literal contextual typing.
- The loop variable has the same type as `start` and `end`.
- The loop variable is an immutable binding scoped to the loop body.
- If `start >= end`, the loop body runs zero times.
- The range step is always `+1`.
- `break` exits the innermost loop.
- `continue` skips to the next iteration of the innermost loop.
- `break value` is not supported.
- Loops are statements and do not produce values.
- Exiting a loop runs the normal scope-end `drop` behavior for values whose scopes end.
- `break` and `continue` run the same cleanup for scopes they leave.

Collection iteration is protocol-driven and has an explicit ownership mode:

```nct
for item in &values {
    inspect(item)
}

for item in move values {
    consume(move item)
}

for item in &+values {
    update(item)
}
```

- `&expression` selects the source type's readonly expansion operator.
- `&+expression` selects its readwrite expansion operator and holds an exclusive source loan.
- `move expression` selects its owned expansion operator and transfers the source.
- A bare expression is accepted only when its type already implements the trusted iterator
  contract. A collection value without `&`, `&+`, or `move` is rejected rather than guessed.
- The source expression is evaluated once. Its iterator is owned by the loop and advanced through
  a validated declaration identity, not a method-name search.
- Each successful step initializes one immutable loop binding. Absence ends the loop without
  initializing or dropping an element.
- `continue` drops an unconsumed current element before advancing. `break`, `return`, propagation,
  and normal completion drop live element state and then the iterator exactly once.
- A readonly yielded borrow retains its source loan through the borrow's last use. A consuming
  yielded value owns exactly one transferred drop obligation.
- A readwrite yielded borrow holds one exclusive element loan. It must end before the iterator is
  advanced again.

Deferred:

- implicit choice between readonly and consuming iteration for a bare collection value
- reverse iteration and custom step syntax
- asynchronous iteration and iterator adapters that require closures

The compiler must not lower collection iteration into calls selected by the spellings `iter`,
`into_iter`, or `next`. Expansion operators and the trusted `Iterator` declaration are selected by
declaration identity. See [Expansion Operators](23-expansion-operators.md).

Use range `for` with indexing:

```nct
for i in 0..<bytes.len() {
    let byte = bytes[i]
    consume(byte)
}
```

## Never and Reachability

`never` represents a computation that does not return normally.

`never` is not an ordinary value-carrying type. It is the type of control flow that terminates the current path instead of producing a value.

Typical uses:

- `trap(): never`
- `std/process.abort(): never`
- `std/process.exit(code): never`
- an infinite event loop that has no reachable `break`
- an explicit unreachable-code marker in the standard library

`trap` is the primitive boundary for non-recoverable program defects. The compiler may also generate traps for checked operations such as out-of-bounds indexing or invalid arithmetic.

`abort` and `exit` are standard-library process APIs. They are not compiler primitives.

`panic` is not a language feature. Nocter has no stack-unwinding mechanism.

Example:

```nct
use std/process as process

func require_path(path: &str?): &str {
    let value = path otherwise { process.abort() }

    return value
}
```

Rules:

- A function declared as returning `never` must not complete normally.
- A `never` function body must terminate all reachable paths with another `never` call, a non-breaking infinite `loop`, a low-level primitive such as `trap`, a standard-library terminating API such as `abort` or `exit`, or equivalent terminating control flow.
- `return` and `return value` are not valid in a `never` function.
- Falling off the end of a `never` function is a compile error.
- A call whose type is `never` terminates the current control path.
- Code after `return`, `break`, `continue`, or a `never` call in the same block is unreachable.
- Unreachable statements have no runtime semantics. They do not contribute to body result typing,
  definite-initialization joins, move/drop liveness on later reachable paths, or buildability.
- Unreachable statements after a proven terminal statement are accepted. A future lint may report
  them, but unreachable code is not a required compile-time error.
- A `never`-typed expression can appear where another expression type is required because it produces no value.
- `never` cannot be constructed, stored in a variable, used as a field type, or used as an array element type.
- Calling a `never` function does not imply stack unwinding, statement-end temporary drops, or caller-scope `drop` execution.
- If cleanup is required before a terminating API such as `exit` or `abort`, the program must perform that cleanup before the `never` call or use a normal `return`, `break`, or `continue` path.
- Fallible failure is recoverable failure and is valid only through fallible type `T!`.
- `trap` is non-recoverable failure caused by a program defect, violated compiler check, or impossible execution path.
- `abort` is immediate process termination and does not run Nocter cleanup.
- `panic` and stack unwinding are not language features.
- `panic` is not reserved. A user-defined function named `panic` is ordinary and has no language-defined behavior.

Example:

```nct
func require_path_short(path: &str?): &str {
    return path otherwise { process.abort() }
}
```

The `otherwise` expression above has type `&str`. The fallback body does not produce a fallback `&str`; it terminates the current path.

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

Safety checks are part of Nocter semantics and remain enabled in every build mode.

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
- General user code has no unchecked arithmetic, unchecked indexing, or unchecked enum-tag operation.
- Wrapping arithmetic is not unchecked arithmetic. It must be exposed through explicit numeric APIs.
- Target overlays and compiler primitive lowering may use target-specific machine instructions internally, but that must not expose undefined behavior to ordinary Nocter code.
