# Source Style and Formatting

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Direction

Adopted: Nocter syntax should be tolerant where whitespace is not semantically meaningful, while the formatter defines one official source style.

Rules:

- Formatting must not change program semantics.
- Style violations are not compile errors in v0.
- The parser accepts valid whitespace variations where tokenization remains unambiguous.
- The formatter emits the official source style.
- Specification examples, `README.md` examples, `AI.md`, `example.nct`, and `spec/examples/valid/` should use formatter output as the canonical presentation.
- The formatter belongs in the compiler toolchain, not in editor extensions.

This keeps the language pleasant to write by hand while avoiding multiple competing styles in documentation, generated examples, diagnostics, AI-generated code, and future editor tooling.

## CLI

Formatting is exposed through `nocter fmt`.

```sh
nocter fmt app.nct
nocter fmt --check app.nct
```

Rules:

- `fmt` takes one `.nct` source file in v0.
- `fmt` formats only the file named on the command line.
- `fmt` does not follow imports.
- `fmt` does not treat the input as a compile-unit root.
- `fmt` does not perform name resolution, type checking, ownership checking, target lowering, code generation, or execution.
- `fmt` must parse enough source structure to preserve comments and produce valid Nocter syntax.
- If parsing fails, `fmt` must not rewrite the file.
- `fmt` rewrites the file in place only after formatting succeeds.
- `fmt --check` compares the input against formatter output and does not rewrite the file.
- `fmt --check` exits successfully only when the file already matches formatter output.
- `fmt` does not need a target option.

Project-wide formatting is deferred until package roots or manifests exist.

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
    fail AppError.missing_path
} else {
    use(path)
}
```

```nct
impl File {
    pub method (file: &+Self).write_text(text: StringView): void ! IOError {
        ...
    }
}
```

## Type Syntax Spacing

Rules:

- Fallible type syntax is formatted as `T ! E`.
- Fallible optional success is formatted as `T? ! E`.
- Optional type syntax is attached to the success type: `T?`.
- Pointer and borrow syntax is attached to the type: `*T`, `&T`, `&+T`.
- Generic type arguments are attached to the type name: `View<T>`.
- Fixed-size arrays are formatted as `[T; N]`.
- Parentheses used for type grouping do not add internal padding: `(T ! E)?`.

Examples:

```nct
func open(path: StringView): File ! IOError
func env(name: StringView): StringView? ! ProcessError
func first(items: View<StringView>): StringView?
```

The parser may accept compact fallible spelling such as `T!E`, but formatter output must use `T ! E`.

## Declarations

Rules:

- Type annotations use no space before `:` and one space after `:`.
- Function return annotations use no space before `:` and one space after `:`.
- Commas have no preceding space and one following space on a single line.
- Multi-line parameter lists put one parameter per line and keep a trailing comma.
- The closing `)` of a multi-line parameter list is aligned with the start of the declaration.

Examples:

```nct
let count: u64 = 0

let home = maybe_home else {
    return none
}

func read_all(
    allocator: &+Allocator,
    path: StringView,
): String ! IOError {
    ...
}
```

## Expressions

Rules:

- Assignment and compound assignment use spaces around the operator.
- Binary operators use spaces around the operator.
- The optional default operator is formatted as `value ?? fallback`.
- The conditional operator is formatted as `condition ? then_value : else_value`.
- Unary operators are attached to their operand.
- Function calls have no space between callee and `(`.
- Method calls have no space around `.`.
- Indexing has no space between value and `[`.
- Field access has no space around `.`.

Examples:

```nct
count += 1
let home = maybe_home ?? "/tmp"
let label = is_ready ? "ready" : "waiting"
let byte = bytes[i]
file.write_text("hello")
```

## Control Flow

Rules:

- Control-flow keywords are followed by one space before their condition or pattern.
- `catch` in `try ... catch` is separated from the preceding expression by one space.
- `match` arms use `is Pattern { ... }`.
- Range `for` syntax is formatted as `for i in start..<end { ... }`.

Examples:

```nct
let file = try File.open(path) catch error {
    fail AppError.open_failed(path)
}

match error {
    is AppError.missing_path {
        ...
    }
    is AppError.open_failed(path) {
        ...
    }
}

for i in 0..<bytes.len() {
    ...
}
```

## Comments

Rules:

- Formatter output preserves line comments and block comments.
- Formatter output may adjust surrounding whitespace but must not rewrite comment text.
- Line comments keep at least one space between code and `//` when they share a line.
- Block comments keep their internal text unchanged in v0.

Comment paragraph reflow is not part of v0.

## Non-Goals in v0

The following are not part of formatter v0:

- project-wide formatting
- import sorting
- semantic rewrites
- line-length based wrapping guarantees
- configurable style profiles
- editor-only formatting rules
- comment paragraph reflow
