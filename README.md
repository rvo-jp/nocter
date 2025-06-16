<p align="center"><img src="https://rvo.jp/nocter/icon.svg" alt="Nocter Logo" width="120" height="120"></p>
<h1 align="center">Nocter</h1>
<p align="center"><strong>Nocter</strong> is a simple and lightweight <strong>object-oriented interpreted language</strong> with JavaScript-inspired syntax and modern error handling.</p>

## Build & Run

In the project root, build the interpreter with:

```sh
./build.sh
```

To run a script:

```sh
nocter example.nct
```

## Features

### Familiar, JavaScript-like Syntax

Nocter’s syntax is intuitive and expressive—ideal for developers familiar with modern scripting languages.

### Safe by Design

- Undefined variables or properties produce clear runtime errors.
- Out-of-bounds access is also treated as errors.

```swift
let arr = [1, 2, 3]
arr[3] // error: index 3 out of bounds for 'arr' (length is 3)
```

### Minimal and Sensible Type Coercion

While Nocter enforces strict type checking by default, it allows limited type coercion where it makes sense. the + operator, for example, supports both numeric addition and string concatenation without explicit casting.

```swift
"abc" + 1 + 23 // "abc123"
```

### Flexible and Expressive Language Constructs

Nocter includes a number of original, practical features to enhance expressiveness:

- Function literals (anonymous functions)
- Inline evaluated blocks with local scope
- Local import and class definitions, usable outside the global scope
- Clear and concise try expressions for error handling

## Roadmap

Planned improvements include:

- A more comprehensive standard library
- A fully interactive REPL
- Expanded support for language constructs, including new statements and operators
- More robust tooling and language server integration in the future
