# Nocter

**Nocter** is a simple and lightweight **object-oriented interpreted language**.
It features a JavaScript-inspired syntax that is flexible, approachable, and easy to learn.

---

## Features

### Familiar and Intuitive Syntax

Strongly influenced by JavaScript, Nocter offers a flexible and expressive syntax that feels natural to developers familiar with modern scripting languages.

### Safer by Design

- Accessing undefined variables or missing properties results in a clear error rather than a silent undefined.
- Out-of-bounds array access and null dereferencing also produce errors, helping catch bugs early.

### Minimal and Sensible Type Coercion

While Nocter enforces strict type checking by default, it allows limited type coercion where it makes sense:

- The + operator, for example, supports both numeric addition and string concatenation without explicit casting—only where ambiguity is unlikely.

### Flexible and Expressive Language Constructs

Nocter includes a number of original, practical features to enhance expressiveness:

- Function literals (anonymous functions)
- Inline evaluated blocks with local scope
- Local import and class definitions, usable outside the global scope
- Clear and concise try expressions for error handling

---

## Build & Run

In the project root, build the interpreter with:

```sh
./build.sh
```

To run a script:

```sh
nocter example.nct
```

---

## Roadmap

Planned improvements include:

- A more comprehensive standard library
- A fully interactive REPL
- Expanded support for language constructs, including new statements and operators
- More robust tooling and language server integration in the future
