# Nocter AI Guide

This file is a compact guide for AI tools that read, write, review, or repair Nocter code.
The normative language specification is [SPEC.md](SPEC.md). When this guide conflicts with the specification, the specification wins.

## Goal

Nocter should be readable and writable by humans first, and predictable for AI tools second.
The language should not add alternate syntax only to satisfy AI tools. Instead, AI support comes from one canonical style, clear examples, machine-readable diagnostics, and compiler-owned structure dumps.

## Canonical Style

Use the formatter's output as the only canonical source style.

Important spellings:

```nct
use std/prelude

from std/io import print

program(): i32 {
    try print("Hello") catch error {
        return 1
    }

    return 0
}
```

Rules for generated code:

- Use 4 spaces for indentation.
- Do not write semicolons.
- Write fallible types as `T ! E`, not `T!E`.
- Write optional fallible types as `T? ! E`.
- Use `let` for immutable bindings and `var` for mutable bindings.
- Use `&T` for readonly borrow and `&+T` for readwrite borrow.
- Use `try expr` and `try expr catch error { ... }` only for fallible values such as `T ! E` and `T? ! E`.
- Use `if let value = optional { ... } else { ... }` for `T?`.
- Use `??` for optional default values.
- Use `match` for enum values.
- Do not use `match` to unwrap `T ! E`.
- Do not use `try` to unwrap `T?`.

## Import Rules

Nocter does not use a `module` declaration. A file's module identity comes from its path.

```nct
use std/prelude

from std/io import print
from ./config import Config
```

Rules:

- `use std/prelude` imports the explicit prelude.
- `from path import Name` imports selected public names.
- Paths starting with `./` or `../` are resolved relative to the current file.
- Paths such as `std/io` are resolved from the active Nocter home.
- Do not invent aliases or wildcard imports unless the specification adds them.

## Error And Optional Patterns

Fallible values use `T ! E`.

```nct
from std/io import IOError
from std/io import print

func announce(text: StringView): void ! IOError {
    try print(text)
    return
}
```

Handle an error locally with `catch`.

```nct
try print("Hello") catch error {
    return 1
}
```

Optional values use `T?`.

```nct
from std/process import ProcessError
from std/process import env

func use_home(): void ! ProcessError {
    if let home = try env("HOME") {
        use(home)
    } else {
        use("fallback")
    }

    return
}
```

Default optional values use `??`.

```nct
from std/process import ProcessError
from std/process import env

func user_name(): StringView ! ProcessError {
    return (try env("USER")) ?? "unknown"
}
```

## Common Mistakes

Avoid these patterns:

```nct
// invalid: `try` is for T ! E, not T?
let value = try maybe_value

// invalid: compact fallible spelling is parsed but not canonical style
func read(): String!IOError

// invalid: Nocter does not use a module declaration
module app/main

// invalid: main is not the Nocter entry point
func main(): i32 {
    return 0
}

// invalid: do not treat print as a compiler builtin
print("Hello")
```

Prefer:

```nct
use std/prelude

from std/io import print

program(): i32 {
    try print("Hello") catch error {
        return 1
    }

    return 0
}
```

## Machine-Readable Tooling

AI tools should interact with the compiler instead of reimplementing Nocter semantics.

Reserved and planned commands:

```sh
nocter check app.nct --format json
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter fmt app.nct
```

Expected AI loop:

```text
write Nocter code
run nocter fmt
run nocter check --format json
use diagnostics spans and fix hints
repeat
```

`tokens` and `ast` are tooling commands. They must not become a separate language definition. The compiler's parser and semantic checks remain the source of truth.

## Example Corpus

Use examples as training and repair references:

```text
spec/examples/valid/
spec/examples/invalid/
```

Valid examples should be formatter-ready Nocter code.
Invalid examples should contain one intended error pattern and a short comment explaining it.
