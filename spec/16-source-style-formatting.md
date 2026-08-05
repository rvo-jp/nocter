# Source Style and Formatting

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Direction

Adopted: Nocter syntax should be tolerant where whitespace is not semantically meaningful, while the formatter defines one official source style.

Rules:

- Formatting must not change program semantics.
- Style violations are not compile errors in v0.2.0.
- The parser accepts valid whitespace variations where tokenization remains unambiguous.
- The formatter emits the official source style.
- Specification examples, `README.md` examples, `spec/guides/ai.md`, `example.nct`, and `spec/examples/valid/` should use formatter output as the canonical presentation.
- The formatter belongs in the compiler toolchain, not in editor extensions.

This keeps the language pleasant to write by hand while avoiding multiple competing styles in documentation, generated examples, diagnostics, AI-generated code, and future editor tooling.

## CLI

Formatting is exposed through `nocter fmt`.

```sh
nocter fmt app.nct
nocter fmt --check app.nct
```

Rules:

- `fmt` takes one `.nct` source file in v0.2.0.
- `fmt` formats only the file named on the command line.
- `fmt` does not follow imports.
- `fmt` does not treat the input as a compile-unit root.
- `fmt` does not perform name resolution, type checking, ownership checking, target lowering, code generation, or execution.
- `fmt` must not delete or rewrite comments.
- Until comment-preserving formatting is implemented, `fmt` must reject files that contain comments instead of rewriting them.
- If parsing fails, `fmt` must not rewrite the file.
- `fmt` rewrites the file in place only after formatting succeeds.
- `fmt --check` compares the input against formatter output and does not rewrite the file.
- `fmt --check` exits successfully only when the file already matches formatter output.
- `fmt` does not need a target option.

Project-wide formatting remains deferred after v0.4.0 Phase 0; package identity alone does not
define comment-preserving traversal, atomic multi-file writes, or partial-failure behavior.

## Package Directives

Package directive records use the same four-space indentation as declarations. Multi-line records
place one field per line and retain a trailing comma.

```nct
#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    entry: "./src/app",
}
```

Directive values are formatted as declarative data. The formatter does not interpret them as
ordinary expressions or perform module resolution.

## Indentation and Blocks

Rules:

- Formatter output uses 4 spaces per indentation level.
- Formatter output does not use tabs for indentation.
- Opening braces stay on the same line as the construct that owns the block.
- `else` stays on the same line as the previous closing brace.
- Top-level declarations are separated by one blank line.
- Empty blocks may stay as `{}` only when the construct is intentionally empty and readability is not harmed.

Examples:

```nct
if path.len() == 0 {
    return Error.new("app.missing_path", "missing path")
} else {
    use(path)
}
```

```nct
impl File {
    pub method &+self.write_text(text: &str): void! {
        ...
    }
}
```

## Type Syntax Spacing

Rules:

- Fallible type syntax is formatted as `T!`.
- Fallible optional success is formatted as `T?!`.
- Optional type syntax is attached to the success type: `T?`.
- Pointer and borrow syntax is attached to the type: `*T`, `&T`, `&+T`.
- Optional borrows omit redundant grouping: `&T?` and `&+T?`.
- A prefix applied to an outcome type retains grouping to preserve meaning: `&(T?)`, `*(T!)`.
- Generic type arguments are attached to the type name: `Buffer<T>`.
- Unsized array data type syntax is formatted as `[T]`; array slices are formatted as `&[T]` and `&+[T]`.
- Fixed-size arrays are formatted as `[T; N]`.
- Parentheses used for type grouping do not add internal padding: `(T)!`.

Examples:

```nct
func open(path: &str): File!
func env(name: &str): &str?!
func first(items: &[&str]): &str?
func borrow_optional(value: T?): &(T?)
```

Formatter output uses `T?!` for fallible optional success values because the postfix operators can be read in order: optional success first, fallible wrapper second.

## Declarations

Rules:

- Type annotations use no space before `:` and one space after `:`.
- Function return annotations use no space before `:` and one space after `:`.
- Commas have no preceding space and one following space on a single line.
- Multi-line parameter lists put one parameter per line and keep a trailing comma.
- The closing `)` of a multi-line parameter list is aligned with the start of the declaration.
- A v0.3.0 Phase 4 generic bound uses no space before `:` and one space after it:
  `T: Interface<U>`.
- Multiple v0.3.0 Phase 9 bounds use one space around `+`:
  `T: Iterator<U> + ExactSizeIterator<U>`.
- A result provenance clause follows the return type on the same line with one space before
  `from`. Union origins use one space around `|`.

Examples:

```nct
let count: u64 = 0

let home = maybe_home?

func read_all(
    allocator: &+Allocator,
    path: &str,
): String! {
    ...
}

pub method &self.get(index: usize): &T? from self
func choose<T>(left: &T, right: &T): &T from left | right
construct String {
    pub default literal ""(text: &str): Self from current { ... }
}
```

## Expressions

Rules:

- Assignment and compound assignment use spaces around the operator.
- Binary operators use spaces around the operator.
- `otherwise` fallback blocks are formatted as `value otherwise { body }`.
- `if` and `match` expressions use the same formatting as their statement forms.
- A result-only body may be formatted as `{ expr }` when it is short.
- Unary operators are attached to their operand.
- Function calls have no space between callee and `(`.
- Method calls have no space around `.`.
- Indexing has no space between value and `[`.
- Field access has no space around `.`.

Examples:

```nct
count += 1
let home = maybe_home otherwise { "/tmp" }
let label = if is_ready { "ready" } else { "waiting" }
let byte = bytes[i]
file.write_text("hello")
```

## Control Flow

Rules:

- Control-flow keywords are followed by one space before their condition or pattern.
- `catch` after a fallible expression is separated from the expression by one space.
- Single-pattern enum checks use `if expr is Pattern { ... }`.
- `match` arms use `Pattern { ... }`.
- `match` fallback arms use `_ { ... }` and must be written last.
- Range `for` syntax is formatted as `for i in start..<end { ... }`.

Examples:

```nct
let file = File.open(path) catch error {
    return Error.new("std.io.open_failed", error.message)
}

match error {
    AppError.missing_path {
        ...
    }
    AppError.open_failed(path) {
        ...
    }
    _ {
        ...
    }
}

return match error {
    AppError.missing_path { missing_code() }
    AppError.open_failed(path) { code_for(path) }
    _ { unknown_code() }
}

for i in 0..<bytes.len() {
    ...
}
```

## Comments

Rules:

- Formatter output must preserve normal comments and doc comments once comment-preserving formatting is implemented.
- Current formatter v0.2.0 rejects files that contain comments with a diagnostic instead of rewriting them.
- Formatter output may adjust surrounding whitespace but must not rewrite comment text.
- Line comments keep at least one space between code and `//` when they share a line.
- Block comments keep their internal text unchanged in v0.2.0.
- Doc comments keep their doc marker spelling: `///`, `/**`, `//!`, or `/*!`.

Comment paragraph reflow is not part of v0.2.0.

## Non-Goals in v0.2.0

The following are not part of formatter v0.2.0:

- project-wide formatting
- import sorting
- semantic rewrites
- line-length based wrapping guarantees
- configurable style profiles
- editor-only formatting rules
- comment paragraph reflow
