# Source Style and Formatting

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Direction

Nocter syntax is tolerant where whitespace is not semantically meaningful, while the formatter defines one official source style.

Rules:

- Formatting must not change program semantics.
- Style violations are not compile errors.
- The parser accepts valid whitespace variations where tokenization remains unambiguous.
- The formatter emits the official source style.
- Specification snippets, root documentation examples, `spec/guides/ai.md`, and packages under
  `examples/` should use formatter output as the canonical presentation.
- The formatter belongs in the compiler toolchain, not in editor extensions.

This keeps the language pleasant to write by hand while avoiding multiple competing styles in documentation, generated examples, diagnostics, AI-generated code, and future editor tooling.

## Contract-First Module Roots

`index.nct` is both the module root and the preferred human-readable API document. Keep it focused
on the declarations that another source or module may use.

Keep these forms in `index.nct`:

- module documentation, imports needed by public signatures, and public re-exports;
- public and restricted-public type contracts;
- fields or variants intentionally exposed as stable data representation;
- bodyless callable, construction, instance-operation, conformance, and interface-default
  contracts;
- short inline behavior whose implementation is itself the clearest contract.

Move private representation, allocation or pointer work, platform operations, helper algorithms,
and ordinary nontrivial bodies to responsibility-named sources included by `index.nct`. A useful
inline body has no branch, loop, mutation, allocation, I/O, target-specific operation, or private
representation access. Constant results and direct representation-independent forwarding are
typical inline cases. Line count alone does not make a body trivial.

A public record should expose all of its fields as stable representation. If any field is an
implementation detail, prefer an opaque public nominal with construction and accessor contracts
instead of mixing a public field API with private layout.

## CLI

Formatting is exposed through `nocter fmt`.

```sh
nocter fmt app.nct
nocter fmt --check app.nct
```

Rules:

- `fmt` takes one `.nct` source file.
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

Project-wide formatting is not supported. Package identity alone does not define comment-preserving
traversal, atomic multi-file writes, or partial-failure behavior.

## Package Directives

Package directive records use the same four-space indentation as declarations. Multi-line records
place one field per line and retain a trailing comma.

```nct
#name: "json-tool"
#version: "0.1.0"
#executable: {
    name: "json-tool",
    module: "./src/app",
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
- An explicit discard initializer is formatted as `let _ = expression`.

Examples:

```nct
if path.len() == 0 {
    return error.new("app.missing_path", "missing path")
} else {
    use(path)
}
```

```nct
instance File {
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
- Outcome elimination of an existing move-only place is formatted without redundant grouping:
  `move value?`, `move value!`, `move value catch error { ... }`, and
  `move value otherwise { ... }`. The formatter rewrites `(move value)?` to `move value?`.
- Parentheses that separate two outcome elimination layers are retained: `(move result?)?` is not
  rewritten as the invalid `move result??`.

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
- A single-line comma-delimited list never retains a trailing comma, even though the parser accepts
  one.
- A multi-line comma-delimited list puts one item per line and retains a trailing comma.
- The same canonical trailing-comma rule applies to arguments, parameters, generic parameters and
  arguments, literals, import selections, directive data, enum payloads, and closure segments.
- Struct declaration fields and enum declaration variants are newline-separated declarations, not
  comma-delimited list items, and never end in commas. Named-field construction initializers remain
  comma-delimited list items.
- The closing `)` of a multi-line parameter list is aligned with the start of the declaration.
- A generic parameter list contains names only: `<T, U>`.
- Complete explicit generic owner arguments are attached to the owner name, as in
  `Vec<i32>.with_capacity(16)` and `Vec<i32> []`.
- A call never prints explicit arguments for the callable's own generic parameters; those parameters
  are inferred.
- A capability predicate uses no space before `:` and one space after it:
  `where T: Interface<U>`.
- Multiple bounds use one space around `+`:
  `where T: Iterator + ExactSizeIterator`.
- An intrinsic copy predicate is `where copy T`. `copy` is not a capability after `:`.
- A callable requirement clause remains on the signature line after result provenance:
  `: Self! from allocator where copy T`. Requirements use comma-space separation.
- A result provenance clause follows the return type on the same line with one space before
  `from`. Union origins use one space around `|`.
- An associated type declaration is `pub type Name` or `pub type Name: Bound + Bound` on its own
  interface-member line.
- An associated type binding is `type Name = Type` and precedes method implementations in a
  conformance body.
- A projected type uses no spaces around the dot: `Self.Item` or `S.Item`.
- A type-equality predicate uses one space around `=`: `R.Item = L.Item`.
- An operator requirement encloses its expression and writes an explicit result type:
  `where (&T == &T): bool`, `where (&T < &T): bool`, or `where (&C[K]): &V`.
- An equality declaration is `operator (&self == other: &Self): bool`; spaces surround `==`, the
  named right binding follows it, and the complete operand expression stays in parentheses.
- A strict-order declaration is `operator (&self < other: &Self): bool`; spaces surround `<`, the
  named right binding follows it, and `>`, `<=`, and `>=` have no declaration forms.
- A drop declaration is `drop TypePattern(&+self) { ... }`. It has no visibility, generic prefix,
  `where` clause, or result annotation, and follows ordinary top-level brace formatting.
- A readonly index declaration is `operator (&self[index: K]): &V`; a readwrite declaration is
  `operator (&+self[index: K]): &+V`. There is no space before `[`, and the index binding follows
  ordinary parameter spacing. An explicit result provenance clause, when needed, follows the
  result type.
- An opaque result is `some Interface` or `some Interface<Item = Type>`. Associated binding `=`
  uses one space on each side, comma spacing follows ordinary generic arguments, and `?` or `!`
  attaches to the complete opaque type without extra parentheses.
- Closure captures precede one semicolon and parameters follow it: `(&limit, move prefix; value)`.
  Capture and parameter segments independently use the common single-line or multi-line comma
  layout. The semicolon terminates the capture segment and replaces its trailing comma. A
  multi-line capture therefore attaches `;` directly to its final capture. A multi-line parameter
  segment retains its ordinary trailing comma before the closing parenthesis. There is one space
  after a single-line semicolon and no space before it. A result annotation follows the closing
  parenthesis as `): Type`.

```nct
let callback = (
    &source,
    move prefix;
    value,
    index,
): bool {
    ...
}
```

Examples:

```nct
let count: u64 = 0

let maybe_home: &str? = lookup("HOME")
let home = maybe_home?

func read_all(
    allocator: &+Allocator,
    path: &str,
): String! from allocator {
    ...
}

pub method &self.get(index: usize): &T?
func choose<T>(left: &T, right: &T): &T from left | right
construct String {
    pub default literal ""(text: &str): Self { ... }
}
```

## Expressions

Rules:

- Assignment and compound assignment use spaces around the operator.
- Binary operators use spaces around the operator.
- Type conversion uses one space around `as`, as in `&value as &View`.
- `otherwise` fallback blocks are formatted as `value otherwise { body }`.
- `if` and `match` expressions use the same formatting as their statement forms.
- A result-only body may be formatted as `{ expr }` when it is short.
- Unary operators are attached to their operand.
- Redundant grouping between unary `-` and an integer literal is removed, so `-(128)` formats as
  `-128`. This does not create a negative-literal token.
- Prefix borrowing binds before `as`; write `&value as &View` without redundant parentheses, and
  write `&(value as WiderInteger)` when borrowing the converted result.
- Function calls have no space between callee and `(`.
- Method calls have no space around `.`.
- Indexing has no space between value and `[`.
- Field access has no space around `.`.
- A wrapped binary expression normally puts a binary-only operator at the beginning of the
  continuation line. Binary `-` stays at the end of the preceding line because `-` can also begin a
  unary expression.
- A wrapped field or method chain puts `.` at the beginning of each continuation line.
- A call opener `(` and index opener `[` remain on the same line as the expression they extend.

Examples:

```nct
count += 1
let home = maybe_home otherwise { "/tmp" }
let label = if is_ready { "ready" } else { "waiting" }
let byte = bytes[i]
file.write_text("hello")

let total = left
    + right
    * scale

let difference = left -
    right

let result = values
    .map(transform)
    .filter(predicate)
```

## Control Flow

Rules:

- Control-flow keywords are followed by one space before their condition or pattern.
- `catch` after a fallible expression is separated from the expression by one space.
- Single-pattern enum checks use `if expr is Pattern { ... }`.
- `match` arms use `Pattern { ... }`.
- `match` fallback arms use `_ { ... }` and must be written last.
- A formatter preserves a final `_` arm even when the explicit arms cover all current variants.
- A formatter does not merge, delete, or reorder duplicate variant arms. Duplicate variants are
  semantic errors reported by checking.
- Enum pattern payload slots use the common comma-list formatting. Keep one `_` for every ignored
  payload position; the formatter never collapses multiple slots into one wildcard.
- A pattern target keeps its ownership prefix adjacent to the target expression: `match &value`,
  `match &+value`, and `match move value`.
- Range `for` syntax is formatted as `for i in start..<end { ... }`.

Examples:

```nct
let file = File.open(path) catch failure {
    return error.new("std.io.open_failed", failure.message)
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

Collection iteration keeps an explicit ownership prefix adjacent to its source:

```nct
for item in &values { ... }
for item in &+values { ... }
for item in move values { ... }
for item in make_iterator() { ... }
```

An existing move-only iterator is formatted with `move`; formatting never removes that required
ownership transfer. A newly produced collection that requires owned expansion is first bound, then
moved:

```nct
let values = make_values()
for item in move values { ... }
```

## Comments

Rules:

- Formatter output must preserve normal comments and doc comments once comment-preserving formatting is implemented.
- The current formatter rejects files that contain comments with a diagnostic instead of rewriting them.
- Formatter output may adjust surrounding whitespace but must not rewrite comment text.
- Line comments keep at least one space between code and `//` when they share a line.
- Block comments keep their internal text unchanged once comment-preserving formatting is implemented.
- Doc comments keep their doc marker spelling: `///`, `/**`, `//!`, or `/*!`.

The formatter does not reflow comment paragraphs.

## Formatter Non-goals

The formatter does not support:

- project-wide formatting
- import sorting
- semantic rewrites
- line-length based wrapping guarantees
- configurable style profiles
- editor-only formatting rules
- comment paragraph reflow
