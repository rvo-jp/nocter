# Errors and Optionals

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Fallible Types

Failure is represented with fallible types, not exceptions.

```nct
func open(path: &str): File! {
    if failed {
        return error.new("std.io.not_found", "file not found")
    }

    return file
}
```

`T!` is a fallible type. It means the expression or function succeeds with `T` or fails with the built-in `error` payload.

```text
T! = fallible T with built-in error
```

The failure type is not written at each call site. All fallible values use the same failure payload type, `error`.

Payload fields:

```text
code: &str
message: &str
```

Rules:

- `error` is compiler built-in type-position syntax, like `str`, `i32`, and `never`.
- `error` is not looked up through imports and cannot be redefined as a type declaration.
- The spelling `error` may still be used as an ordinary value binding name. For example, `catch error` binds a local value named `error`.
- `T!` always means success `T` or failure `error`.
- `std/error` owns the source-backed `error.new(code: &str, message: &str)` construction member.
  The built-in type identity selects that validated surface; the compiler does not recognize the
  member name `new` or rewrite an alias.
- Error codes are intentionally open. Standard-library, user, and package code may introduce
  dotted strings such as `"std.io.not_found"`, `"app.config.missing_key"`, or
  `"package.module.reason"`.
- The current standard library does not define `Error` or `ErrorCode` aliases. `error` is the sole
  public spelling of the failure payload type.
- Domain detail is represented in the `error` payload and standard-library helper APIs, especially through classification code and `message`, not by writing a different failure type in the signature.
- `error.code` and `error.message` are the direct user-facing fields for reporting.
- `error.code` is an open dotted string code such as `"std.io.not_found"` or `"app.config.missing_key"`.
- `error.message` is a human-readable diagnostic message string.
- The built-in `error` payload is copyable, non-owning, and carries borrow-like
  provenance from its `&str` fields. Returning or storing an `error` follows the
  same escape rules as other aggregates containing borrow-like values.
- The ABI layout of `error` is specified in [ABI and Layout](09-abi-layout.md#built-in-error-layout).
- `error!` is not a valid function return type. In a fallible function, `return error_value` means failure, so `error` cannot be used as the success type without ambiguity. This rule is checked after type aliases and through optional success layers such as `error?!`.

The constructor is declared in ordinary Nocter source:

```nct
construct error {
    pub default func new(code: &str, message: &str): Self from code | message {
        return new_error(code, message)
    }
}
```

Inside a function returning `T!`, a compatible function body result or `return value` returns the success value unless the value has type `error`. `return error_value` returns the failure value.

```nct
func write(file: &+File, text: &str): void! {
    if failed {
        return error.new("std.io.broken_pipe", "broken pipe")
    }

    return
}
```

Postfix `?` unwraps fallible and optional values for propagation.

```nct
let file = File.open(path)?
```

For `T!`, `expr?` evaluates to the success value when `expr` succeeds. On failure, the current
function, method, or closure fails with the same `error` payload.

For `T?`, `expr?` evaluates to the present value when `expr` is present. On `none`, the current
function, method, or closure returns `none` through its optional return layer.

Example:

```nct
let file = File.open(path)?
```

This binds `file` to the successful `File` value. If `File.open(path)` fails, the current function fails with that `error` as if `return error_value` had been executed.

Rules:

- Postfix `?` is not an exception mechanism.
- Applying `?` to an existing move-only outcome place requires `move place?`. This canonical form
  moves the complete outcome first and then unwraps it. The source place is uninitialized on every
  continuation, including success, propagated failure, and propagated absence paths.
- A newly produced outcome temporary needs no `move`. A copyable outcome place may be used without
  `move`; it is copied and the original remains initialized.
- Postfix `?` does not perform stack unwinding.
- Postfix `?` on `T!` can be used only inside a fallible function.
- Postfix `?` on `T?` can be used only when the current callable body's result layer can carry
  `none`.
- Postfix `?` does not convert `none` into `error`.
- Postfix `?` does not convert `error` into `none`.
- Scope-end cleanup and `drop` behavior still run as they would for an explicit `return`.
- Error conversion is not needed for propagation because every fallible value fails with `error`.
- `throw` is not part of the language.

An existing move-only outcome is invalidated before its tag is selected:

```nct
func require_name(): String? {
    let maybe: String? = find_name()
    let text = move maybe?

    use(maybe) // error: use after move
    return move text
}
```

Postfix `!` forcefully unwraps fallible and optional values.

```nct
let file = File.open(path)!
let user = move maybe_user!
```

Rules:

- For `T!`, `expr!` evaluates to the success value when `expr` succeeds.
- For `T?`, `expr!` evaluates to the present value when `expr` is present.
- Applying `!` to an existing move-only outcome place requires `move place!`. The complete source
  outcome is moved before its tag is checked. A newly produced temporary or copyable outcome does
  not require `move`.
- If `expr!` sees failure or `none`, execution terminates immediately through the ordinary
  non-recoverable Nocter safety trap.
- That path is exactly the ordinary Nocter safety trap used for bounds, arithmetic, and other
  checked contract violations. Nocter does not print the `error` payload, emit a fixed absence
  message, or translate the trap into entry-wrapper failure status `1`.
- A forced-unwrap trap performs no stack unwinding or source-level cleanup. Live locals and
  statement temporaries are not dropped on that path.
- OS-provided signal names, process status, crash reports, and incidental output after a trap are
  outside the language contract.
- `expr!` does not return `error` or `none` to the caller.
- `expr!` has result type `T`.
- One expression layer accepts only one postfix `?` or `!`. To eliminate a second composed outcome
  layer, use an intermediate binding or explicit grouping such as `(load()?)!`.
- `expr!` is intended for tests, prototypes, and truly unrecoverable assumptions.
- Normal code should prefer `?`, `catch`, or `otherwise`.
- `expr!` is not stack unwinding.
- Programs that need a stable message, exit code, or cleanup before termination must handle the
  outcome with `catch` or `otherwise` and call an explicit process API after the required work.

## Recoverable Failure and Non-Recoverable Termination

Fallible `return`, `trap`, and `abort` are distinct mechanisms.

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
- `panic` is not a language feature.
- Nocter does not perform stack unwinding.
- Build modes must not disable these trap checks; see [Safety Checks and Build Modes](03-control-flow.md#safety-checks-and-build-modes).

`catch` handles the failure side of a fallible expression.

```nct
let file = File.open(path) catch failure {
    return error.new("std.io.open_failed", failure.message)
}
```

`expr catch error { ... }` means:

- Evaluate `expr`.
- If `expr` succeeds, the whole `catch` expression evaluates to the success value.
- If `expr` fails, bind the failure value to the catch binding and execute the `catch` block.
- If the block reaches its end, its result becomes the whole `catch` expression's value.

Local recovery can therefore compute a replacement and continue:

```nct
let port = configured_port() catch failure {
    report(failure)
    8080
}
```

Use `_` when the failure payload is intentionally discarded:

```nct
operation() catch _ {
    return fallback()
}
```

Rules:

- `catch` applies only to fallible values of type `T!`.
- `catch` does not apply to optional values `T?`.
- The catch binding has type `error`.
- The binding name after `catch` is an ordinary local name. `catch error` is conventional, but `catch err` is also valid.
- `catch _` creates no binding. `_` cannot be referenced, hovered, renamed, or used as a
  provenance origin.
- Bare `catch { ... }` is invalid; discarding the failure must be explicit.
- The catch block is evaluated only on failure.
- Applying `catch` to an existing move-only fallible place requires
  `move place catch name { ... }`. The complete fallible value is moved before selecting success
  or failure. A new temporary or copyable fallible value does not require `move`.
- A reachable catch block end must produce a value assignable to the fallible success type `T`.
- A catch block may instead leave the current control path with `return`, `break`, `continue`, a
  call returning `never`, or another terminating construct.
- For `void!`, an empty catch block recovers with `void`.
- A trailing `T!` is not flattened and a trailing `error` does not implicitly fail again. Use `?`
  to propagate or an explicit `return error_value` to replace the enclosing failure.
- `catch` is not exception handling.
- `catch` does not perform stack unwinding.
- A recovering catch moves its block result into the surrounding destination, then drops the
  remaining catch-local values before continuing.
- A terminating catch runs the same scope-end cleanup that its explicit control flow would run.
- If a `catch` block terminates by calling a `never` function, cleanup behavior is determined by that `never` function. The compiler does not add implicit unwinding.
- The `catch` clause belongs to the immediately preceding fallible expression. It is not a general handler after arbitrary expressions.

Postfix `?` propagates the original failure.

`catch` is used for explicit local handling or error replacement.

```nct
func read_all(
    path: &str,
): String! {
    var file = File.open(path) catch _ {
        return error.new("app.open_failed", "failed to open input")
    }

    let text = file.read_to_string() catch _ {
        return error.new("app.read_failed", "failed to read UTF-8 input")
    }

    return move text
}
```

`map_error` is not a language operation. It may be provided as an ordinary standard-library API in the future, but the compiler does not special-case that name.

Fallible values are not pattern matched.

Rules:

- `match` does not apply to `T!`.
- `if expr is Pattern` does not apply to `T!`.
- `is ok(...)` and failure patterns are not part of the language.
- `ok` is not a reserved keyword.
- Fallible values are handled with postfix `?` and `catch`.

## Optional Types

Optional values use the type syntax `T?`.

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
- `T` must not be `void`, including after alias expansion or generic substitution. Optional
  `void` has no source value for its present branch; use an enum when that state distinction is
  required.
- A compatible function body result or `return value` in a `T?` function returns the present value.
- `return none` in a `T?` function returns absence.
- Postfix `?` on `T?` propagates `none` through the current optional return layer.
- `match` and `if expr is Pattern` do not apply to `T?`.
- `some(value)` is not language syntax.
- `some` is not a reserved keyword. It is contextual only at the start of a static opaque result
  type such as `some Iterator<Item = T>`; in value position it remains an ordinary identifier.

## Composing Optionals and Fallible Types

Optional and fallible type constructors may be composed explicitly.

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
- `expr catch error { ... }` on `T?!` handles only failure and leaves the optional success layer.
- A reachable catch fallback for `T?!` therefore produces `T?`; a `T` result constructs presence,
  while `none` preserves absence.
- `otherwise` applied after that `catch` handles only successful absence; it does not enter the
  catch block.
- Applying `?` again to that `T?` propagates `none` through the current optional return layer.
- The second application is written through an intermediate binding or grouping. Adjacent
  `expr??` is invalid; `(expr?)?` exposes the two elimination boundaries explicitly.
- In a function returning `T?!`, a compatible function body result or `return value` returns success with a present `T`.
- In a function returning `T?!`, `return none` returns success with absence.
- In a function returning `T?!`, `return error_value` returns failure with `error`.
- `T` must not be `error`. Use a wrapper type if an `error` payload must be carried as successful optional data.
- An optional layer must not have `void` as its eventual payload. Consequently `void?!` and
  `(void!)?` are invalid even though `void!` is valid.
- Other mixed forms must use parentheses.
- `(T!)?` means an optional fallible value.

### Recursive Outcome Injection

A value-producing function body result or `return expression` is checked against the complete
declared result type by one outer-to-inner rule. This rule is the only implicit construction of
optional and fallible return layers.

Given an expression and an expected result type:

1. If the expression already has exactly the expected type, return that value unchanged. Do not
   add another outcome layer.
2. If the expected type is `U?`, `none` constructs absence. Every other expression is recursively
   injected into `U`, then wrapped as presence.
3. If the expected type is `U!`, an expression of type `error` constructs failure. Every other
   expression is recursively injected into `U`, then wrapped as success.
4. At a non-outcome expected type, the expression must be assignable to that type under the
   ordinary contextual typing rules.

The exact-type check occurs before opening an outcome layer. Returning an existing complete
outcome therefore preserves its tags rather than nesting or reinterpreting it. One optional layer
and one fallible layer are the maximum supported depth, and `error` cannot be a success base type,
so the injection path is unique.

The order of the declared type determines the meaning of contextual `none` and `error`:

| Declared result | Returned expression | Constructed result |
| --- | --- | --- |
| `T?!` | `value: T` | success with present `T` |
| `T?!` | `none` | success with absence |
| `T?!` | `failure: error` | outer failure |
| `(T!)?` | `value: T` | presence containing success `T` |
| `(T!)?` | `failure: error` | presence containing inner failure |
| `(T!)?` | `none` | outer absence |

For example:

```nct
func cached_name(): (String!)? {
    if cache_disabled {
        return none
    }

    if load_failed {
        return error.new("app.cache.load_failed", "failed to load cached name")
    }

    return String.copy("Nocter")
}
```

A result expression whose type is already `String!` is injected only into the outer optional
layer of `(String!)?`. A result whose type is already `(String!)?` is returned unchanged. The same
recursive rule applies to the final expression of a callable body.

Outcome injection does not weaken ownership rules or manufacture a copy. An existing move-only
binding still requires explicit `move`, whether it supplies the complete declared result or a
payload that the injection wraps. A newly produced temporary is transferred into the constructed
outcome normally.

Example:

```nct
func env(name: &str): &str?! {
    if missing {
        return none
    }

    if invalid_utf8 {
        return error.new("std.process.invalid_encoding", "environment value is not UTF-8")
    }

    return value
}
```

Using a fallible optional:

```nct
let maybe_config = load_config()?
let config = move maybe_config?

use(config)
```

Handling failure and absence independently:

```nct
let home = env("HOME") catch error {
    return report(error)
} otherwise {
    "unknown"
}
```

### Optional Propagation

Postfix `?` propagates optional absence.

When `expr` has type `T?`, `expr?` unwraps the present `T`. If `expr` is `none`, the current
function, method, or closure returns `none` through its optional return layer.

```nct
func require_home(): &str? {
    let home = lookup("HOME")?

    return home
}
```

Rules:

- Postfix `?` on `T?` is valid when the current callable body's result type can carry `none`, such
  as `U?` or `(U?)!`.
- In a function returning `(U?)!`, `none` is returned as successful absence, not as failure.
- Postfix `?` on `T?` is invalid in a function whose current return layer cannot carry `none`.
- Exact absence propagation uses `?`. Absence defaulting and control flow other than returning the
  same `none` use `otherwise`.
- `otherwise` does not propagate absence by itself; it selects a fallback block when the optional value is `none`.

### Optional Otherwise Expressions

Optional fallback uses `otherwise`.

```nct
let home = lookup("HOME") otherwise { "/tmp" }
```

```nct
let config = find_config(path) otherwise {
    return error.new("app.config.missing", path)
}

load(config)
```

Rules:

- `expr otherwise { body }` applies only when `expr` has type `T?`.
- Applying `otherwise` to an existing move-only optional place requires
  `move place otherwise { body }`. The complete optional value is moved before selecting presence
  or absence. A new temporary or copyable optional value does not require `move`.
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

`is` is reserved for enum variants only.

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
- Collection iteration helpers may return `T?`, but there is no dedicated optional-loop syntax.
