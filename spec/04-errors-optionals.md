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

Inside a function returning `T!`, a compatible function body result or `return value` returns the success value unless the value has type `error`. `return error_value` returns the failure value.

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
- Normal code should prefer `?`, `catch`, or `otherwise`.
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

Inside a function returning `T?`, a compatible function body result or `return value` returns a present value and `return none` returns absence.

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
- A compatible function body result or `return value` in a `T?` function returns the present value.
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
- In a function returning `T?!`, a compatible function body result or `return value` returns success with a present `T`.
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
let config = maybe_config otherwise { return none }

use(config)
```

### Optional Propagation

Adopted: postfix `?` propagates optional absence.

When `expr` has type `T?`, `expr?` unwraps the present `T`. If `expr` is `none`, the current function returns `none` through its optional return layer.

```nct
func require_home(): &str? {
    let home = lookup("HOME") otherwise { return none }

    return home
}
```

Rules:

- Postfix `?` on `T?` is valid when the current function's return type can carry `none`, such as `U?` or `(U?)!`.
- In a function returning `(U?)!`, `none` is returned as successful absence, not as failure.
- Postfix `?` on `T?` is invalid in a function whose current return layer cannot carry `none`.
- Absence defaulting and absence-side early exit use `otherwise`.
- `otherwise` does not propagate absence by itself; it selects a fallback block when the optional value is `none`.

### Optional Otherwise Expressions

Adopted: optional fallback uses `otherwise`.

```nct
let home = lookup("HOME") otherwise { "/tmp" }
```

```nct
let config = find_config(path) otherwise {
    return Error.new("app.config.missing", path)
}

load(config)
```

Rules:

- `expr otherwise { body }` applies only when `expr` has type `T?`.
- If `expr` is present, the result is the contained `T`.
- If `expr` is `none`, the fallback body is evaluated.
- The fallback body must produce `T`, or it may terminate the current control path with `return`, loop-local `break` / `continue`, or `never`.
- The fallback body follows the common body rule: statements first, then an optional result expression.
- The fallback body is evaluated only when needed.
- `otherwise` is an expression, not a declaration form.
- `otherwise` does not use `some` / `none` patterns.
- Evaluating `expr` and the fallback body follows normal ownership rules.
- `??`, `let ... else`, and `var ... else` are not Nocter syntax.

Chained fallback is written by nesting `otherwise` in the fallback body:

```nct
let port = env_int("PORT") otherwise {
    config.default_port otherwise { 8080 }
}
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

Rules:

- `while let`, `while var`, `if let`, and `if var` are not Nocter syntax.
- Optional values are not automatically iterable.
- Collection iteration helpers may return `T?`, but v0 does not introduce a dedicated optional loop syntax.
