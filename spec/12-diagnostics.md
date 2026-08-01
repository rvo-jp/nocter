# Diagnostics

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Diagnostic Policy

Adopted: compiler diagnostics are part of the Nocter user experience and must explain source-level concepts clearly.

Diagnostics for type checking, ownership, borrowing, initialization state, visibility, imports, fallible values, optionals, primitive boundaries, and target selection should include:

- what is invalid
- which source construct is invalid
- why it is invalid
- the most useful correction direction when one is known

Diagnostic text must use Nocter source concepts such as `binding`, `borrow`, `move`, `drop`, `pub(nocter)`, `T!`, `error`, `T?`, `T?!`, `entry function`, `primitive`, and `Nocter home`. It should not expose backend implementation details such as temporary register allocation, Mach-O offsets, internal AST node names, or recovery placeholders.

## Format

Adopted: errors use stable error codes and source spans.

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

Initial spanless CLI diagnostic codes:

- `E0602`: formatter check found a source file that would change.
- `E0700`: command-line syntax or unsupported command.
- `E0701`: target selection failed.
- `E0702`: filesystem path or permission failure.
- `E0703`: Nocter home resolution or validation failed.
- `E0704`: temporary executable preparation or execution handoff failed before user code started.

## Source And Span Model

Adopted: compiler source identity and source positions are byte-based internally.

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
- Bare carriage return is a source error in v0.
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

Adopted: `nocter check --format json` writes exactly one JSON object to stdout.

Top-level envelope:

```json
{
  "schema": "nocter.diagnostics",
  "version": 1,
  "ok": false,
  "command": "check",
  "target": "arm64-darwin",
  "root": "app.nct",
  "root_absolute_path": "/Users/me/project/app.nct",
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
- `root` is the root file display path when known, or `null` if root-file discovery did not complete.
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
- `severity` is `"error"` in v0.
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

Adopted: parser recovery may be conservative; semantic diagnostics should avoid cascades.

Rules:

- The parser may stop after the first syntax error in v0.
- After parsing succeeds, later compiler phases may report multiple independent errors.
- Cascaded errors should be suppressed when they are caused by an earlier error.
- The compiler may use internal error placeholders to continue analysis, but diagnostics must not mention those placeholders.
- If too many independent errors are found, the compiler may stop after a limit and report that additional errors were suppressed.
- A diagnostic should prefer the earliest source location that caused the invalid program, not the latest internal location where the compiler noticed it.

## Required Dedicated Diagnostics

Adopted: common Nocter-specific mistakes should have dedicated diagnostics instead of generic type errors.

Required v0 diagnostic families:

- Root file path missing, not found, or not a `.nct` file.
- Nocter home missing, not a directory, or missing required entries such as `VERSION`, `MANIFEST.json`, or `std/`.
- Malformed `MANIFEST.json`, release mismatch between `VERSION` and manifest, host mismatch, or default target missing from implemented target list.
- Source file is not valid UTF-8.
- Unsupported source line ending, such as a bare carriage return.
- Unterminated block comment, string literal, or byte literal.
- Invalid escape sequence in a string literal or byte literal.
- Invalid integer literal syntax or digit separator placement.
- Float literal used in v0 source.
- Plain single-quoted character literal used in v0 source.
- Semicolon used in v0 source.
- Non-ASCII identifier or invalid module path segment.
- Import path not found.
- Import cycle detected, including the cycle path.
- Imported name not found.
- Type reference not declared in the current scope, such as `Int` when no alias
  or import defines it.
- Name collision in imports or declarations.
- Attribute syntax or reserved `@` used in v0 source.
- `pub(nocter)` item imported from a user project module.
- `pub(nocter)` used outside the active Nocter home.
- Primitive declaration outside the active Nocter home.
- Primitive declaration with an invalid module path, name, or signature.
- Direct call to a `pub(nocter)` primitive from a user project module.
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
- `catch` block that can fall through.
- Mixed optional/fallible type syntax where grouping changes meaning, such as `(T!)?`.
- `if is` used on a non-enum expression.
- `if is` pattern that does not use `Enum.variant`.
- `if is` enum pattern whose enum or variant does not match the target enum type.
- Move-only enum payload binding from an existing local without an explicit
  `move` pattern target, or from a member target before field moves are
  supported.
- `otherwise` used on a non-optional expression.
- `otherwise` fallback whose body result is not assignable to the optional payload type.
- Check-only construct used with `build` or `run`, such as string interpolation
  before interpolation lowering is implemented, `std/process.env(name)` before
  environment runtime support is promoted, a fixed-array element type containing
  a payload enum, or move-only payload binding with an unsupported recursive drop
  tree and broader payload enum pattern target expressions before those lowering
  paths are promoted.
- Standard-library runtime subset violation during buildability validation, such
  as an unsupported `Vec<T>` element storage path.
- Removed optional extraction syntax such as `let ... else`, `var ... else`, `if let`, `if var`, `while let`, `while var`, and `??`.
- Selected entry function missing from the root file.
- Selected entry function with an invalid return type.
- Selected entry function with type parameters, such as `func main<T>(): i32!`, used in v0.
- Selected entry function with value parameters, such as `func main(args: Vec<&str>): i32!`, used in v0.
- Duplicate selected entry function names, reported by the normal duplicate visible-name diagnostic.
- `return` without a value in a non-`void` function.
- `return` with a value in a `void` function.
- `return` value type mismatch when both expected and actual types are known.
- Non-`void` function reaching the end without an explicit return.
- `pub use path.Name` or `pub use path.{...}` attempting to re-export a private or `pub(nocter)` name.
- Reserved target requested before implementation.

These families do not require final numeric code assignment in the language design phase. The implementation should assign codes when the diagnostics are implemented.

## Examples

Visibility:

```text
error[E0140]: `from_addr` is visible only inside the active Nocter home
  --> app.nct:3:22
   |
3 | use std/ptr.from_addr
   |                      ^^^^^^^^^
   |
note: `from_addr` is declared as `pub(nocter)`
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

Warnings are not part of the initial required diagnostic surface.

Future warnings may be added for style, portability, or likely mistakes, but warnings must not change language semantics. A warning must not be required for memory safety or type soundness.
