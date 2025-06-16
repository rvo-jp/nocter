<p align="center">
  <img src="https://rvo.jp/nocter/icon.svg" alt="Nocter Logo" width="120" height="120">
</p>

<h1 align="center">Nocter</h1>

<p align="center"><strong>Nocter</strong> is a simple and lightweight <strong>object-oriented interpreted language</strong> with JavaScript-inspired syntax and modern error handling.</p>

## Features

| Feature                           | Description |
|-----------------------------------|-------------|
| **Familiar, JavaScript-like Syntax** | Intuitive and expressive—ideal for developers familiar with modern scripting languages. |
| **Safe by Design**                 | Accessing undefined variables or properties produces clear runtime errors. Out-of-bounds array or string access is also treated as an error. |
| **Minimal and Sensible Type Coercion** | While Nocter enforces strict type checking by default, it allows limited type coercion where it makes sense. The `+` operator, for example, supports both numeric addition and string concatenation without explicit casting. |

### Flexible and Expressive Language Constructs

Nocter includes a number of original, practical features to enhance expressiveness:

- Function literals (anonymous functions)
- Inline evaluated blocks with local scope
- Local import and class definitions, usable outside the global scope
- Clear and concise try expressions for error handling

## Build & Run

In the project root, build the interpreter with:

```sh
./build.sh
```

To run a script:

```sh
nocter example.nct
```

## Roadmap

Planned improvements include:

- A more comprehensive standard library
- A fully interactive REPL
- Expanded support for language constructs, including new statements and operators
- More robust tooling and language server integration in the future
