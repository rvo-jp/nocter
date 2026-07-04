# Errors and Optionals

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Fallible Types

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

Adopted: the `try` operator unwraps fallible values for propagation.

```nct
let file = try File.open(path)
```

For `T!E`, `try expr` evaluates to the success value when `expr` succeeds. On failure, the current function fails with the same error unless a `catch` clause is present.

`try` does not apply to optional values `T?` in the initial design.

Example:

```nct
let file = try File.open(path)
```

This binds `file` to the successful `File` value. If `File.open(path)` fails, the current function fails with that error as if `fail error` had been executed.

Rules:

- `try` is not an exception mechanism.
- `try` does not perform stack unwinding.
- `try` on `T!E` without `catch` can be used inside a fallible function with the same error type `E`.
- `try` does not apply to `T?`.
- `fail` can be used only inside a function returning a fallible type.
- Scope-end cleanup and `drop` behavior still run as they would for an explicit `return` or `fail`.
- Error conversion is not implicit in the initial design.
- `throw` is not part of the language.

Adopted: `catch` handles the failure side of a fallible value at the `try` site.

```nct
let file = try File.open(path) catch error {
    fail AppError.open_failed(path)
}
```

`try expr catch error { ... }` means:

- Evaluate `expr`.
- If `expr` succeeds, the whole `try ... catch` expression evaluates to the success value.
- If `expr` fails, bind the failure value to `error` and execute the `catch` block.

Rules:

- `catch` applies only to fallible values of type `T!E`.
- `catch` does not apply to optional values `T?`.
- The catch binding has the failure type `E`.
- The catch block is evaluated only on failure.
- The catch block must not fall through in the initial design.
- The catch block must leave the current control path with `fail`, `return`, `break`, `continue`, a call returning `never`, or another terminating construct.
- The catch block has no trailing expression result.
- `catch` is not exception handling.
- `catch` does not perform stack unwinding.
- `catch` runs the same scope-end cleanup that the explicit terminating control flow would run.
- If a `catch` block terminates by calling a `never` function, cleanup behavior is determined by that `never` function. The compiler does not add implicit unwinding.
- The `catch` clause belongs to the preceding `try` expression. It is not a general handler after arbitrary expressions.

`try` without `catch` propagates the original failure. The propagated error type must match the current fallible function's error type exactly in the initial design.

`try` with `catch` is used for explicit error mapping.

```nct
func read_all(
    allocator: &+Allocator,
    path: StringView,
): String!AppError {
    var file = try File.open(path) catch error {
        fail AppError.open_failed(path)
    }

    var text = try file.read_to_string(allocator) catch error {
        fail AppError.read_failed(path)
    }

    return move text
}
```

`map_error` is not part of the initial language design. It may be considered later as an ordinary standard-library API, but the compiler does not special-case that name.

Fallible values are not pattern matched in the initial design.

Rules:

- `match` does not apply to `T!E`.
- `if expr is Pattern` does not apply to `T!E`.
- `is ok(...)` and `is fail(...)` patterns are not part of the language.
- `ok` is not a reserved keyword.
- Fallible values are handled with `try` and `try ... catch`.

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

Rules:

- `T?` is not spelled as a special `Option<T>` type.
- `none` is the optional absent literal.
- `return value` in a `T?` function returns the present value.
- `return none` in a `T?` function returns absence.
- `try` does not apply to `T?`.
- Optional propagation is not part of the initial design.
- `match` does not apply to `T?` in the initial design.
- `if expr is Pattern` does not apply to `T?` in the initial design.
- `some(value)` is not part of the initial language.
- `some` is not a reserved keyword.

Adopted: optional local branching uses `if let` and `if var`.

```nct
if let home = env("HOME") {
    use(home)
} else {
    use_default_home()
}
```

```nct
if var text = maybe_text {
    text.push("!")
    use(move text)
}
```

Rules:

- `if let name = expr { ... }` applies only when `expr` has type `T?`.
- `if var name = expr { ... }` applies only when `expr` has type `T?`.
- If `expr` is present, the contained `T` value is bound to `name` and the then body runs.
- `if let` creates an immutable binding.
- `if var` creates a mutable binding.
- If `expr` is `none`, the else body runs if present.
- `else` is optional.
- `else if let name = expr { ... }` is allowed.
- `else if var name = expr { ... }` is allowed.
- `else if let` and `else if var` are equivalent to nesting an `if` inside `else`.
- The binding exists only inside the then body.
- The binding is not available in `else` or later `else if` branches.
- `if let` and `if var` are statements and do not produce values.
- `if let` and `if var` do not use `some` / `none` patterns.
- `if var` does not write changes back into the original optional.
- For move-only `T`, `if let` / `if var` consumes the optional value and moves the contained value into the binding.
- For copy `T`, the contained value may be copied according to normal copy rules.
- Borrowing behavior for `if let` / `if var` on borrowed optionals is deferred until borrowed optional projections are specified.

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
