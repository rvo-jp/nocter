# Errors and Optionals

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Fallible Types

Adopted: failure is represented with fallible types, not exceptions.

```nct
func open(path: &str): File! {
    if failed {
        return Error.new("std.io.not_found", "file not found")
    }

    return file
}
```

`T!` is a fallible type. It means the expression or function succeeds with `T` or fails with the built-in `error` payload.

```text
T! = fallible T with built-in error
```

The failure type is not written at each call site. All fallible values use the same failure payload type, `error`.

Initial conceptual payload fields:

```text
code: &str
message: &str
```

Rules:

- `error` is compiler built-in type-position syntax, like `str`, `i32`, and `never`.
- `error` is not looked up through imports and cannot be redefined as a type declaration.
- The spelling `error` may still be used as an ordinary value binding name. For example, `catch error` binds a local value named `error`.
- `T!` always means success `T` or failure `error`.
- `Error` may be provided by `std/prelude` as a normal alias or wrapper for `error`.
- `ErrorCode` is a standard-library `&str` alias, not a compiler-reserved name.
- `ErrorCode` is intentionally open. Standard-library, user, and package code may introduce dotted string codes such as `"std.io.not_found"`, `"app.config.missing_key"`, or `"package.module.reason"`.
- Standard-library constructors such as `Error.new("std.io.not_found", "...")` translate the `ErrorCode` string into the built-in payload's primitive code representation.
- The compiler must not special-case ordinary names such as `Error`, `ErrorCode`, `IOError`, or `Result`.
- Domain detail is represented in the `error` payload and standard-library helper APIs, especially through classification code and `message`, not by writing a different failure type in the signature.
- `error.code` and `error.message` are the initial direct user-facing fields for reporting.
- `error.code` is an open dotted string code such as `"std.io.not_found"` or `"app.config.missing_key"`.
- `error!` is not a valid function return type. In a fallible function, `return error_value` means failure, so `error` cannot be used as the success type without ambiguity.

Inside a function returning `T!`, `return value` returns the success value unless the value has type `error`. `return error_value` returns the failure value.

```nct
func write(file: &+File, text: &str): void! {
    if failed {
        return Error.new("std.io.broken_pipe", "broken pipe")
    }

    return
}
```

Adopted: postfix `?` unwraps fallible and optional values for propagation.

```nct
let file = File.open(path)?
```

For `T!`, `expr?` evaluates to the success value when `expr` succeeds. On failure, the current function fails with the same `error` payload.

For `T?`, `expr?` evaluates to the present value when `expr` is present. On `none`, the current function returns `none` through its optional return layer.

Example:

```nct
let file = File.open(path)?
```

This binds `file` to the successful `File` value. If `File.open(path)` fails, the current function fails with that `error` as if `return error_value` had been executed.

Rules:

- Postfix `?` is not an exception mechanism.
- Postfix `?` does not perform stack unwinding.
- Postfix `?` on `T!` can be used only inside a fallible function.
- Postfix `?` on `T?` can be used only when the current function's return layer can carry `none`.
- Postfix `?` does not convert `none` into `error`.
- Postfix `?` does not convert `error` into `none`.
- Scope-end cleanup and `drop` behavior still run as they would for an explicit `return`.
- Error conversion is not needed for propagation because every fallible value fails with `error`.
- `throw` is not part of the language.

Adopted: postfix `!` forcefully unwraps fallible and optional values.

```nct
let file = File.open(path)!
let user = maybe_user!
```

Rules:

- For `T!`, `expr!` evaluates to the success value when `expr` succeeds.
- For `T?`, `expr!` evaluates to the present value when `expr` is present.
- If `expr!` sees failure or `none`, execution terminates immediately through a trap-like non-recoverable path.
- `expr!` does not return `error` or `none` to the caller.
- `expr!` has result type `T`.
- `expr!` is intended for tests, prototypes, and truly unrecoverable assumptions.
- Normal code should prefer `?`, `catch`, `let ... else`, or `??`.
- `expr!` is not stack unwinding.

## Recoverable Failure and Non-Recoverable Termination

Adopted: fallible `return`, `trap`, and `abort` are distinct mechanisms.

```text
return error_value = recoverable failure through T!
trap               = non-recoverable program defect or violated runtime check
abort              = immediate process termination
```

Rules:

- In a function returning `T!`, `return expr` is a failure return when `expr` has type `error`.
- In a function returning `T!`, `return expr` is a success return when `expr` is assignable to `T`.
- `T` must not be `error`.
- Fallible failure return follows normal `return` cleanup for scopes it leaves.
- `trap` has type `never`.
- `trap` is used for program defects, compiler-inserted safety checks, and impossible paths.
- Out-of-bounds indexing, integer overflow in normal arithmetic, division by zero, invalid live `bool` values, invalid enum tags, and explicit unreachable execution all trap.
- `trap` does not unwind the stack.
- `abort` has type `never`.
- `abort` terminates the process immediately and does not run Nocter cleanup.
- `panic` is not a language feature in v0.
- Stack unwinding is not part of v0.
- Build modes must not disable these trap checks; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes).

Adopted: `catch` handles the failure side of a fallible expression.

```nct
let file = File.open(path) catch error {
    return Error.new("std.io.open_failed", error.message)
}
```

`expr catch error { ... }` means:

- Evaluate `expr`.
- If `expr` succeeds, the whole `catch` expression evaluates to the success value.
- If `expr` fails, bind the failure value to the catch binding and execute the `catch` block.

Rules:

- `catch` applies only to fallible values of type `T!`.
- `catch` does not apply to optional values `T?`.
- The catch binding has type `error`.
- The binding name after `catch` is an ordinary local name. `catch error` is conventional, but `catch err` is also valid.
- The catch block is evaluated only on failure.
- The catch block must not fall through in the initial design.
- The catch block must leave the current control path with `return`, `break`, `continue`, a call returning `never`, or another terminating construct.
- The catch block has no trailing expression result.
- `catch` is not exception handling.
- `catch` does not perform stack unwinding.
- `catch` runs the same scope-end cleanup that the explicit terminating control flow would run.
- If a `catch` block terminates by calling a `never` function, cleanup behavior is determined by that `never` function. The compiler does not add implicit unwinding.
- The `catch` clause belongs to the immediately preceding fallible expression. It is not a general handler after arbitrary expressions.

Postfix `?` propagates the original failure.

`catch` is used for explicit local handling or error replacement.

```nct
func read_all(
    allocator: &+Allocator,
    path: &str,
): String! {
    var file = File.open(path) catch error {
        return Error.new("std.io.open_failed", error.message)
    }

    var text = file.read_to_string(allocator) catch error {
        return Error.new("std.io.read_failed", error.message)
    }

    return move text
}
```

`map_error` is not part of the initial language design. It may be considered later as an ordinary standard-library API, but the compiler does not special-case that name.

Fallible values are not pattern matched in the initial design.

Rules:

- `match` does not apply to `T!`.
- `if expr is Pattern` does not apply to `T!`.
- `is ok(...)` and failure patterns are not part of the language.
- `ok` is not a reserved keyword.
- Fallible values are handled with postfix `?` and `catch`.

## Optional Types

Adopted: optional values use the type syntax `T?`.

```text
T? = optional T
```

An optional value is either present with a `T` value or absent.

Inside a function returning `T?`, `return value` returns a present value and `return none` returns absence.

```nct
func lookup(name: &str): &str? {
    if missing {
        return none
    }

    return value
}
```

Rules:

- `T?` is not spelled as a special `Option<T>` type.
- `none` is the optional absent literal.
- `return value` in a `T?` function returns the present value.
- `return none` in a `T?` function returns absence.
- Postfix `?` on `T?` propagates `none` through the current optional return layer.
- `match` does not apply to `T?` in the initial design.
- `if expr is Pattern` does not apply to `T?` in the initial design.
- `some(value)` is not part of the initial language.
- `some` is not a reserved keyword.

## Composing Optionals and Fallible Types

Adopted: optional and fallible type constructors may be composed explicitly.

Preferred source spelling:

```text
T?! = fallible optional success
```

`T?!` means the computation can fail with `error`. If it succeeds, the success value is optional: present `T` or `none`.

Rules:

- `T!` means a fallible success value.
- `T?` means an optional value.
- Prefer `T?!` in official style.
- `expr?` on `T?!` unwraps only the fallible layer and produces `T?`.
- Applying `?` again to that `T?` propagates `none` through the current optional return layer.
- In a function returning `T?!`, `return value` returns success with a present `T`.
- In a function returning `T?!`, `return none` returns success with absence.
- In a function returning `T?!`, `return error_value` returns failure with `error`.
- Other mixed forms must use parentheses in v0.
- `(T!)?` means an optional fallible value.

Example:

```nct
func env(name: &str): &str?! {
    if missing {
        return none
    }

    if invalid_utf8 {
        return Error.new("std.process.invalid_encoding", "environment value is not UTF-8")
    }

    return value
}
```

Using a fallible optional:

```nct
let maybe_config = load_config()?

let config = maybe_config else {
    return none
}

use(config)
```

### Optional Propagation

Adopted: postfix `?` propagates optional absence.

When `expr` has type `T?`, `expr?` unwraps the present `T`. If `expr` is `none`, the current function returns `none` through its optional return layer.

```nct
func require_home(): &str? {
    let home = lookup("HOME") else {
        return none
    }

    return home
}
```

Rules:

- Postfix `?` on `T?` is valid when the current function's return type can carry `none`, such as `U?` or `(U?)!`.
- In a function returning `(U?)!`, `none` is returned as successful absence, not as failure.
- Postfix `?` on `T?` is invalid in a function whose current return layer cannot carry `none`.
- Early-exit extraction uses `let ... else` and `var ... else`.
- Defaulting uses `??`.
- `??` does not propagate absence out of the current function; it selects a fallback value or fallback optional expression.

### Optional Let Else Declarations

Adopted: optional early-exit extraction uses `let ... else` and `var ... else`.

```nct
let home = lookup("HOME") else {
    return none
}

use(home)
```

```nct
let config = find_config(path) else {
    return Error.new("app.config.missing", path)
}

load(config)
```

Rules:

- `let name = expr else { ... }` applies when `expr` has type `T?`.
- `var name = expr else { ... }` applies when `expr` has type `T?`.
- If `expr` is present, the contained `T` value is bound to `name` and execution continues after the declaration.
- If `expr` is `none`, the `else` block runs.
- The `else` block must have type `never`.
- The `else` block must leave the current control path with `return`, `return none`, `break`, `continue`, a call returning `never`, a non-breaking infinite `loop`, or an equivalent terminating construct.
- The `else` block must not fall through.
- The binding exists after the declaration and is not available inside the `else` block.
- `let ... else` and `var ... else` are declaration statements, not expressions.
- `let ... else` and `var ... else` do not use `some` / `none` patterns.
- `else` cannot provide a fallback value. Use `??` when absence should select a default value.
- Evaluating `expr` follows normal ownership rules. If `expr` moves a move-only optional binding, that source binding becomes uninitialized on the continuing present path.
- For move-only `T`, `let ... else` / `var ... else` consumes the optional value and moves the contained value into the binding.
- For copy `T`, the contained value may be copied according to normal copy rules.

Borrowed optional projections are allowed in optional let-else declarations:

```nct
let name = &maybe_name else {
    return none
}

inspect(name) // name: &String
```

```nct
var name = &+maybe_name else {
    return none
}

name.push("!") // name: &+String
```

Rules:

- `let name = &place else { ... }` applies when `place` has type `T?`.
- The continuing binding has type `&T`.
- The optional value is not moved or copied.
- If `place` is `none`, no contained borrow is created and the `else` block runs.
- `var name = &+place else { ... }` applies when `place` has type `T?` and `place` is writable.
- The continuing binding has type `&+T`.
- The readwrite projection follows the normal exclusivity rules of `&+T`.
- `let name = &+place else { ... }` is not part of v0. Use `var name = &+place else { ... }` for a readwrite projection, or `let name = &place else { ... }` for a readonly projection.
- `var name = &place else { ... }` is not part of v0 because a readonly projection cannot create a mutable binding.
- While the projected borrow is live, the source optional place cannot be moved, assigned, reinitialized, or explicitly dropped.
- The projected borrow carries the provenance of the source optional place.
- Returning or storing the projected borrow is allowed only when the normal borrow-like provenance and lifetime rules allow it.

`let ... else` is for early exit. A local optional branch where both present and absent paths continue is not part of v0. Use `??` when absence should select a value.

When absence should become a value, use `??` instead.

```nct
let home = lookup_home() ?? "/tmp"
```

### Optional and Fallible Pattern Branching

Adopted: `is` is reserved for enum variants only.

Rules:

- `if expr is Pattern { ... }` applies only to enum values, and the pattern must be written as `Enum.variant`.
- `T?` values do not support `is none`, `is Type`, or `is Type(name)`.
- `T!` values do not support `is Error(name)`, `is Type`, or `is Type(name)`.
- `T?` has no `Some` / `None` enum variants. The absence value is the keyword `none`, usable in expressions such as `return none`.
- `T!` has no success/failure enum variants. Failure is the fallible return channel carrying an `error` value.

### Optional Loops

Adopted: optional loops use ordinary `loop` plus `let ... else`.

```nct
var iter = bytes.iter()

loop {
    let byte = iter.next() else {
        break
    }

    consume(byte)
}
```

Rules:

- `while let`, `while var`, `if let`, and `if var` are not Nocter syntax.
- Use `loop` with `let ... else { break }` when `none` should end iteration.
- The extracted binding follows the normal `let ... else` rules.
- Optional borrow values such as `(&T)?` are allowed. For example, `let item = iter.next() else { break }` is valid inside a loop when `next()` returns `(&T)?`.

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
- The operator does not apply to fallible `T!` values.

Example:

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

This is parsed as:

```nct
let port = env_int("PORT") ?? (config.default_port ?? 8080)
```
