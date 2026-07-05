# Errors and Optionals

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Fallible Types

Adopted: failure is represented with fallible types, not exceptions.

```nct
func open(path: StringView): File ! IOError {
    if failed {
        fail IOError.not_found(path)
    }

    return file
}
```

`T ! E` is a fallible type. It means the expression or function succeeds with `T` or fails with `E`.

```text
T ! E = fallible T with error E
```

Official source style writes spaces around `!`: `T ! E`.
The parser accepts compact spelling such as `T!E`, but formatter output must use the spaced form.

Inside a function returning `T ! E`, `return value` returns the success value and `fail error` returns the failure value.

```nct
func write(file: &+File, text: StringView): void ! IOError {
    if failed {
        fail IOError.broken_pipe
    }

    return
}
```

Adopted: the `try` operator unwraps fallible values for propagation.

```nct
let file = try File.open(path)
```

For `T ! E`, `try expr` evaluates to the success value when `expr` succeeds. On failure, the current function fails with the same error unless a `catch` clause is present.

`try` does not apply to optional values `T?` in the initial design.

Example:

```nct
let file = try File.open(path)
```

This binds `file` to the successful `File` value. If `File.open(path)` fails, the current function fails with that error as if `fail error` had been executed.

Rules:

- `try` is not an exception mechanism.
- `try` does not perform stack unwinding.
- `try` on `T ! E` without `catch` can be used inside a fallible function with the same error type `E`.
- `try` does not apply to `T?`.
- `fail` can be used only inside a function returning a fallible type.
- Scope-end cleanup and `drop` behavior still run as they would for an explicit `return` or `fail`.
- Error conversion is not implicit in the initial design.
- `throw` is not part of the language.

## Recoverable Failure and Non-Recoverable Termination

Adopted: `fail`, `trap`, and `abort` are distinct mechanisms.

```text
fail  = recoverable failure through T ! E
trap  = non-recoverable program defect or violated runtime check
abort = immediate process termination
```

Rules:

- `fail error` is valid only inside a function returning `T ! E`.
- `fail` follows normal `return`-like cleanup for scopes it leaves.
- `trap` has type `never`.
- `trap` is used for program defects, compiler-inserted safety checks, and impossible paths.
- Out-of-bounds indexing, integer overflow in normal arithmetic, division by zero, invalid live `bool` values, invalid enum tags, and explicit unreachable execution all trap.
- `trap` does not unwind the stack.
- `abort` has type `never`.
- `abort` terminates the process immediately and does not run Nocter cleanup.
- `panic` is not a language feature in v0.
- Stack unwinding is not part of v0.
- Build modes must not disable these trap checks; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes).

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

- `catch` applies only to fallible values of type `T ! E`.
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
): String ! AppError {
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

- `match` does not apply to `T ! E`.
- `if expr is Pattern` does not apply to `T ! E`.
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
func lookup(name: StringView): StringView? {
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
- `match` does not apply to `T?` in the initial design.
- `if expr is Pattern` does not apply to `T?` in the initial design.
- `some(value)` is not part of the initial language.
- `some` is not a reserved keyword.

## Composing Optionals and Fallible Types

Adopted: optional and fallible type constructors may be composed explicitly.

Preferred source spelling:

```text
T? ! E = (T?) ! E
```

`T? ! E` means the computation can fail with `E`. If it succeeds, the success value is optional: present `T` or `none`.

Rules:

- `?` binds to the success type before `! E`.
- `StringView? ! ProcessError` means `(StringView?) ! ProcessError`.
- `try expr` on `T? ! E` unwraps only the fallible layer and produces `T?`.
- `try` still does not apply to the optional layer.
- In a function returning `T? ! E`, `return value` returns success with a present `T`.
- In a function returning `T? ! E`, `return none` returns success with absence.
- In a function returning `T? ! E`, `fail error` returns failure with `E`.
- Other mixed forms must use parentheses in v0.
- `(T ! E)?` means an optional fallible value.
- `T ! (E?)` means a fallible value whose error payload is optional.

Example:

```nct
func env(name: StringView): StringView? ! ProcessError {
    if missing {
        return none
    }

    if invalid_utf8 {
        fail ProcessError.invalid_encoding
    }

    return value
}
```

Using a fallible optional:

```nct
let maybe_home = try process.env("HOME")

if let home = maybe_home {
    use(home)
}
```

Equivalent compact use:

```nct
if let home = try process.env("HOME") {
    use(home)
}
```

### Optional Propagation

Adopted: optional propagation syntax is not part of v0.

Nocter does not provide a postfix `expr?` operator or a `try`-like construct for optional values in v0. Returning absence remains explicit.

```nct
func require_home(): StringView? {
    if let home = lookup("HOME") {
        return home
    }

    return none
}
```

Rules:

- `try` remains exclusive to fallible `T ! E` values.
- `try optional_value` is invalid.
- Postfix optional propagation such as `optional_value?` is not part of v0.
- An optional function must use `return none` to return absence.
- Present / absent branching uses `if let`, `if var`, `while let`, and `while var`.
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
    fail AppError.missing_config(path)
}

load(config)
```

Rules:

- `let name = expr else { ... }` applies when `expr` has type `T?`.
- `var name = expr else { ... }` applies when `expr` has type `T?`.
- If `expr` is present, the contained `T` value is bound to `name` and execution continues after the declaration.
- If `expr` is `none`, the `else` block runs.
- The `else` block must have type `never`.
- The `else` block must leave the current control path with `return`, `return none`, `fail`, `break`, `continue`, a call returning `never`, a non-breaking infinite `loop`, or an equivalent terminating construct.
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

`let ... else` is for early exit. When both present and absent cases should continue locally, use `if let` or `if var` instead.

```nct
if let home = lookup("HOME") {
    use(home)
} else {
    use_default_home()
}
```

When absence should become a value, use `??` instead.

```nct
let home = lookup("HOME") ?? "/tmp"
```

### Optional Local Branching

Adopted: optional local branching uses `if let` and `if var`.

```nct
if let home = env("HOME") {
    consume(home)
} else {
    use_default_home()
}
```

```nct
if var text = maybe_text {
    text.push("!")
    consume(move text)
}
```

Rules:

- `if let name = expr { ... }` applies when `expr` has type `T?`.
- `if var name = expr { ... }` applies when `expr` has type `T?`.
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
- Evaluating `expr` follows normal ownership rules. If `expr` moves a move-only optional binding, that source binding becomes uninitialized on all continuing paths.
- For move-only `T`, `if let` / `if var` consumes the optional value and moves the contained value into the binding.
- For copy `T`, the contained value may be copied according to normal copy rules.

### Borrowed Optional Projections

Adopted: `if let` and `if var` can inspect an optional place by borrow without consuming the optional.

```nct
var maybe_name = get_name()

if let name = &maybe_name {
    inspect(name) // name: &String
}
```

```nct
var maybe_name = get_name()

if var name = &+maybe_name {
    name.push("!") // name: &+String
}
```

Rules:

- `if let name = &place { ... }` applies when `place` has type `T?`.
- The then-body binding has type `&T`.
- The optional value is not moved or copied.
- If `place` is `none`, no contained borrow is created and the else body runs if present.
- `if var name = &+place { ... }` applies when `place` has type `T?` and `place` is writable.
- The then-body binding has type `&+T`.
- The readwrite projection follows the normal exclusivity rules of `&+T`.
- `if let name = &+place` is not part of v0. Use `if var name = &+place` for a readwrite projection, or `if let name = &place` for a readonly projection.
- `if var name = &place` is not part of v0 because a readonly projection cannot create a mutable binding.
- The projected borrow exists only inside the then body.
- The binding is not available in `else` or later `else if` branches.
- While the projected borrow is live, the source optional place cannot be moved, assigned, reinitialized, or explicitly dropped.
- The projected borrow carries the provenance of the source optional place.
- Returning or storing the projected borrow is allowed only when the normal borrow-like provenance and lifetime rules allow it.
- `else if let name = &place { ... }` and `else if var name = &+place { ... }` are allowed by the same rules.

Borrowed optional projections are different from ordinary optional borrow values. If `expr` has type `(&T)?`, then `if let name = expr` is the ordinary optional rule and binds `name: &T`.

Adopted: optional loops use `while let` and `while var`.

```nct
var iter = bytes.iter()

while let byte = iter.next() {
    consume(byte)
}
```

Rules:

- `while let name = expr { ... }` applies only when `expr` has type `T?`.
- `while var name = expr { ... }` applies only when `expr` has type `T?`.
- If `expr` is present, the contained `T` value is bound to `name` and the loop body runs.
- If `expr` is `none`, the loop exits normally.
- The binding exists only inside the loop body.
- `while let` creates an immutable binding.
- `while var` creates a mutable binding.
- `while let` and `while var` do not use `some` / `none` patterns.
- For move-only `T`, each successful iteration consumes the optional value and moves the contained value into the binding.
- For copy `T`, the contained value may be copied according to normal copy rules.
- Borrowed optional projections such as `while let name = &place` and `while var name = &+place` are not part of v0 because the projection does not advance or consume the optional.
- Optional borrow values such as `(&T)?` are allowed. For example, `while let item = iter.next()` is valid when `next()` returns `(&T)?`.

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
- The operator does not apply to fallible `T ! E` values.

Example:

```nct
let port = env_int("PORT") ?? config.default_port ?? 8080
```

This is parsed as:

```nct
let port = env_int("PORT") ?? (config.default_port ?? 8080)
```
