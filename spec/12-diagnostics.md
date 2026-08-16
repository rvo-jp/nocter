# Diagnostics

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Diagnostic Policy

Compiler diagnostics are part of the Nocter user experience and must explain source-level concepts clearly.

Diagnostics for type checking, ownership, borrowing, initialization state, visibility, imports, fallible values, optionals, primitive boundaries, and target selection should include:

- what is invalid
- which source construct is invalid
- why it is invalid
- the most useful correction direction when one is known

Diagnostic text must use Nocter source concepts such as `binding`, `borrow`, `move`, `drop`,
`pub(../)`, `pub(/)`, `T!`, `error`, `T?`, `T?!`, `entry function`, `primitive`, and `Nocter home`.
It should not expose backend implementation details such as temporary register allocation, Mach-O
offsets, internal AST node names, or recovery placeholders.

## Format

Errors use stable error codes and source spans.

Minimum source diagnostic shape:

```text
error[E0001]: cannot move `file` while it is borrowed
  --> app.nct:12:18
   |
12 |     close(move file)
   |                ^^^^ move occurs here
   |
note: readonly borrow created here
  --> app.nct:10:16
   |
10 |     inspect(&file)
   |              ^^^^
help: end the borrow before moving `file`
```

Rules:

- Every source-level compiler error should have an error code in the form `E0000`.
- A source-level diagnostic must have one primary span when the compiler can point to source text.
- Related spans should be emitted when they explain the cause, such as the creation of a borrow or the original move.
- `note` explains context or cause.
- `help` gives a correction direction when the compiler can suggest one without guessing intent.
- Command-line, target-selection, filesystem, and Nocter-home discovery errors may omit source spans, but must still name the failing argument, path, target, or environment variable.
- Error codes should not encode compiler phase. The same user-visible mistake should keep the same code even if implementation phases change.
- Error codes become compatibility-sensitive once published in user-facing documentation.

Spanless CLI diagnostic codes:

- `E0602`: formatter check found a source file that would change.
- `E0700`: command-line syntax or unsupported command.
- `E0701`: target selection failed.
- `E0702`: filesystem path or permission failure.
- `E0703`: Nocter home resolution or validation failed.
- `E0704`: temporary executable preparation or execution handoff failed before user code started.
- `E0800`: package manifest, package-root, executable declaration, or package target selection failed.

Source-backed callable contract diagnostics:

- `E0250`: a bodyless public callable contract has no implementation body.
- `E0251`: the selected private implementation body does not exactly match its public contract.
- `E0252`: more than one private implementation body matches the same public contract.
- `E0253`: body omission is used outside a public callable contract in `index.nct`, or on a callable
  form that requires an inline body.

## Source And Span Model

Compiler source identity and source positions are byte-based internally.

Internal model:

```text
SourceId
SourceMap
ByteSpan { source, start, end }
```

Rules:

- `SourceId` is an internal compiler integer ID.
- `SourceId` is never emitted in public JSON.
- `SourceMap` owns loaded source files and maps `SourceId` to display path, canonical absolute path when known, normalized text, and line-start offsets.
- `ByteSpan.source` is a `SourceId`.
- `ByteSpan.start` and `ByteSpan.end` are UTF-8 byte offsets in the normalized source text.
- `ByteSpan.end` is exclusive.
- The compiler normalizes CRLF to LF before computing spans.
- Bare carriage return is a source error.
- Internal compiler analysis should use `ByteSpan` instead of line/column pairs.
- Line and column pairs are derived only for human-readable diagnostics, JSON output, editor adapters, tests, and AI tooling.

Public JSON span shape:

```json
{
  "file": "app.nct",
  "absolute_path": "/Users/me/project/app.nct",
  "start_byte": 120,
  "end_byte": 124,
  "start_line": 12,
  "start_column_byte": 18,
  "end_line": 12,
  "end_column_byte": 22
}
```

JSON span rules:

- `file` is the diagnostic display path.
- `absolute_path` is the canonical absolute path used for editor integration and source-file identity, or `null` when unknown.
- `start_byte` and `end_byte` are UTF-8 byte offsets after line-ending normalization.
- `end_byte` is exclusive.
- `start_line` and `end_line` are 1-based line numbers after line-ending normalization.
- `start_column_byte` and `end_column_byte` are 1-based UTF-8 byte columns within the normalized line.
- These JSON line and column fields are not LSP positions.
- LSP adapters convert byte offsets or byte columns into the client position encoding.
- Display path rules and canonical source-file identity are specified in [Modules and Use Declarations](01-modules-use.md#source-file-identity).

## Machine-Readable JSON Diagnostics

`nocter check --format json` writes exactly one JSON object to stdout.

Top-level envelope:

```json
{
  "schema": "nocter.diagnostics",
  "version": 1,
  "ok": false,
  "command": "check",
  "target": "arm64-darwin",
  "root": "/Users/me/project/nocter.nct",
  "root_absolute_path": "/Users/me/project/nocter.nct",
  "diagnostics": []
}
```

Envelope rules:

- The top-level JSON value is always one object.
- The output is not newline-delimited JSON.
- The output is not a top-level array.
- `schema` is always `"nocter.diagnostics"` for this envelope.
- `version` is the integer schema version. The initial version is `1`.
- `ok` is `true` only when `diagnostics` is empty.
- `ok` is `false` when at least one diagnostic is present.
- `command` is the command that produced the diagnostics, initially `"check"`.
- `target` is the active target when known, or `null` if target selection did not complete.
- `root` is the selected package `nocter.nct` or explicit file display path when known, or `null` if
  input selection did not complete.
- `root_absolute_path` is the canonical absolute path of the root file when known, or `null` if root-file discovery did not complete.
- `diagnostics` is an array of diagnostic objects.
- Human-readable progress text, logs, or diagnostics must not be mixed into stdout.

Diagnostic object:

```json
{
  "code": "E0001",
  "severity": "error",
  "message": "cannot move `file` while it is borrowed",
  "primary_span": {
    "file": "app.nct",
    "absolute_path": "/Users/me/project/app.nct",
    "start_byte": 120,
    "end_byte": 124,
    "start_line": 12,
    "start_column_byte": 18,
    "end_line": 12,
    "end_column_byte": 22
  },
  "notes": [
    {
      "message": "readonly borrow created here",
      "span": {
        "file": "app.nct",
        "absolute_path": "/Users/me/project/app.nct",
        "start_byte": 84,
        "end_byte": 88,
        "start_line": 10,
        "start_column_byte": 16,
        "end_line": 10,
        "end_column_byte": 20
      }
    }
  ],
  "help": "end the borrow before moving `file`"
}
```

Diagnostic object rules:

- `code` uses the `E0000` form for source diagnostics, command-line errors, filesystem errors, Nocter-home errors, target-selection errors, and internal compiler errors that are reported through this JSON format.
- `severity` is currently `"error"`.
- Future schema versions may add `"warning"` and `"info"`.
- `message` is the primary human-readable diagnostic message.
- `primary_span` is a span object when a source location is known.
- `primary_span` is `null` for diagnostics without a useful source span, such as command-line, filesystem, Nocter-home, or target-selection errors.
- `notes` is an array. It is empty when there are no related notes.
- Each note has a `message` and a `span`.
- A note `span` may be `null` when the note is not tied to a source location.
- `help` is a string when the compiler has a concrete correction direction.
- `help` is `null` when no useful correction direction is known.

Span rules:

- Diagnostic JSON uses the public JSON span shape defined in [Source And Span Model](#source-and-span-model).
- `primary_span` and note `span` may be `null`.
- Non-null spans must include byte offsets and byte-column positions.

Example for a command-line or filesystem diagnostic:

```json
{
  "schema": "nocter.diagnostics",
  "version": 1,
  "ok": false,
  "command": "check",
  "target": null,
  "root": "missing.nct",
  "root_absolute_path": null,
  "diagnostics": [
    {
      "code": "E0002",
      "severity": "error",
      "message": "root file `missing.nct` was not found",
      "primary_span": null,
      "notes": [],
      "help": "check the path passed to `nocter check`"
    }
  ]
}
```

## Error Recovery

Parser recovery may be conservative; semantic diagnostics should avoid cascades.

Rules:

- The parser may stop after the first syntax error.
- After parsing succeeds, later compiler phases may report multiple independent errors.
- Cascaded errors should be suppressed when they are caused by an earlier error.
- The compiler may use internal error placeholders to continue analysis, but diagnostics must not mention those placeholders.
- If too many independent errors are found, the compiler may stop after a limit and report that additional errors were suppressed.
- A diagnostic should prefer the earliest source location that caused the invalid program, not the latest internal location where the compiler noticed it.

## Required Dedicated Diagnostics

Common Nocter-specific mistakes should have dedicated diagnostics instead of generic type errors.

Required diagnostic families:

- Root file path missing, not found, or not a `.nct` file.
- Nocter home missing, not a directory, or missing required entries such as `VERSION`, `MANIFEST.json`, or `std/`.
- Malformed `MANIFEST.json`, release mismatch between `VERSION` and manifest, host mismatch, or default target missing from implemented target list.
- Source file is not valid UTF-8.
- Unsupported source line ending, such as a bare carriage return.
- Unterminated block comment, string literal, or byte literal.
- Invalid escape sequence in a string literal or byte literal.
- Invalid integer literal syntax or digit separator placement.
- Unsupported float literal.
- Plain single-quoted character literal.
- Semicolon used as a statement terminator.
- Non-ASCII identifier or invalid module path segment.
- Import path not found.
- Import cycle detected, including the cycle path.
- Imported name not found.
- Type reference not declared in the current scope, such as `Int` when no alias
  or import defines it.
- Name collision in imports or declarations.
- Attribute syntax or reserved `@` used in source.
- A `pub(...)` scope containing a name, dependency alias, arbitrary path, or too many `../`
  components.
- A module importing a declaration outside its ancestor or package visibility boundary.
- Primitive declaration outside the exact implicit toolchain standard-library package.
- Primitive declaration with an invalid module path, name, or signature.
- Direct call to a package-visible standard-library primitive from a user package.
- Borrow exclusivity violation.
- Readwrite borrow from a non-writable place.
- Move while borrowed.
- Assignment or reinitialization while borrowed.
- Explicit `drop` while borrowed.
- Use after `move`.
- Use after explicit `drop`.
- Explicit `drop` of a copy value, borrow, uninitialized binding, maybe initialized binding, field, index, or non-binding expression.
- Use of a maybe initialized binding.
- Invalid reinitialization target after `move` or `drop`.
- Borrow escaping the storage, temporary, or region it refers to.
- Returning a borrow-like value whose provenance cannot outlive the function.
- Postfix `?` used on `T!` outside a fallible function.
- Postfix `?` used on `T?` outside a function whose current return layer can carry `none`.
- Postfix `?` or `!` used on a non-fallible and non-optional expression.
- Reachable `catch` fallback with no result for a non-`void` success type, or with a result not
  assignable to that success type.
- Reachable value-producing result expression in a `while`, `loop`, range `for`, or collection
  `for` body. Loop statements do not implicitly discard iteration results.
- Mixed optional/fallible type syntax where grouping changes meaning, such as `(T!)?`.
- `if is` used on a non-enum expression.
- `if is` pattern that does not use `Enum.variant`.
- `if is` enum pattern whose enum or variant does not match the target enum type.
- Enum pattern payload arity mismatch, including one `_` used for a variant with more than one
  payload field.
- Unsupported nested, literal, binding-modifier, field-name, or rest pattern syntax.
- Duplicate explicit variant arms in one `match`, regardless of payload binding names or `_`
  positions.
- Errors inside a currently unreachable exhaustive `_` arm are reported normally. Merely having
  that fallback arm is not a diagnostic.
- Owned move-only enum payload binding from an existing enum place without an explicit `move`
  pattern target, or a `move` pattern target whose operand is not an eligible move place.
- Readwrite-borrowed enum pattern target created from a non-writable place or while a conflicting
  borrow is active.
- `otherwise` used on a non-optional expression.
- `otherwise` fallback whose body result is not assignable to the optional payload type.
- Interpolation expression whose value type is not supported by the adopted formatting surface.
- Active Nocter home missing a trusted string, formatting, primitive, or runtime capability required
  by the selected target. `check`, `build`, and `run` report the same failure before successful
  buildability validation.
- Removed optional extraction syntax such as `let ... else`, `var ... else`, `if let`, `if var`, `while let`, `while var`, and `??`.
- Selected entry function missing from the root file.
- Selected entry function with an invalid return type.
- Selected entry function with type parameters, such as `func main<T>(): i32!`.
- Selected entry function with value parameters, such as `func main(args: Vec<&str>): i32!`.
- Duplicate selected entry function names, reported by the normal duplicate visible-name diagnostic.
- `return` without a value in a non-`void` function.
- `return` with a value in a `void` function.
- `return` value type mismatch when both expected and actual types are known.
- Opaque result in an unsupported type position or bodyless contract.
- Opaque result whose return paths select different witnesses, whose witness does not conform to
  the advertised interface, or whose associated binding disagrees with the conformance.
- Assignment between distinct declaration-scoped opaque result identities, even when their
  rendered interface contracts are identical.
- Non-`void` function reaching the end without an explicit return.
- A re-export whose visibility boundary is wider than the imported declaration's boundary.
- Reserved target requested before implementation.

These families do not require final numeric code assignment in the language design phase. The implementation should assign codes when the diagnostics are implemented.

## Examples

Visibility:

```text
error[E0412]: import `std/ptr` cannot access `pub(/)` name `from_addr`
  --> app.nct:3:22
   |
3 | use std/ptr.from_addr
   |                      ^^^^^^^^^
   |
note: `from_addr` is declared as `pub(/)` in the implicit `std` package
help: use a public safe API instead
```

Fallible propagation outside a fallible function:

```text
error[E0331]: postfix `?` would fail with `error`, but function `load` is not fallible
  --> app.nct:8:16
   |
8 |     let file = File.open(path)?
   |                              ^
   |
note: current function returns `String`
help: change the return type to `String!` or handle the failure with `catch`
```

Maybe initialized binding:

```text
error[E0220]: `file` may be uninitialized on this path
  --> app.nct:11:9
   |
11 |     file.read()?
   |     ^^^^
   |
note: `file` is moved on one branch above
help: reinitialize `file` on every path before using it
```

## Warnings

Warnings are not part of the current diagnostic surface.

Future warnings may be added for style, portability, or likely mistakes, but warnings must not change language semantics. A warning must not be required for memory safety or type soundness.
