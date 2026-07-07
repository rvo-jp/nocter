# Tooling and Editor Integration

This file is part of the Nocter language specification.
The specification entry point is [../SPEC.md](../SPEC.md).

## Direction

Adopted: editor tooling must be able to start with TextMate grammar support and later migrate to LSP without replacing the extension.

The compiler is the source of truth for language semantics. Editor extensions may present Nocter code, but they must not become a second implementation of import resolution, name lookup, type checking, ownership checking, or borrow checking.

AI-assisted coding tools follow the same rule. They may generate, format, inspect, and repair Nocter code, but they should use compiler-owned formatting, diagnostics, token dumps, and AST dumps instead of maintaining a separate interpretation of the language.

Initial VS Code extension layout:

```text
tools/
    vscode-nocter/
        package.json
        language-configuration.json
        syntaxes/
            nocter.tmLanguage.json
        snippets/
            nocter.code-snippets
        src/
            extension.ts
```

## TextMate Stage

The first editor integration layer is TextMate grammar plus language configuration.

Allowed responsibilities:

- syntax highlighting
- comment toggling for `//`, `/* ... */`, `///`, `/** ... */`, `//!`, and `/*! ... */`
- bracket matching for `()`, `{}`, and `[]`
- auto closing quotes and braces
- snippets for common declarations

Rules:

- TextMate grammar must follow [Lexical Grammar](13-lexical-grammar.md).
- TextMate grammar may use the reserved keyword list from [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md#reserved-keywords).
- TextMate grammar must not define semantic validity beyond lexical and shallow syntactic patterns.
- TextMate grammar must not attempt to resolve imports.
- TextMate grammar must not infer types.
- TextMate grammar must not implement ownership, borrow, move, drop, optional, fallible, or region checks.

## Compiler Diagnostics Stage

Before LSP, editor diagnostics should come from the compiler.

Initial command direction:

```sh
nocter check app.nct --format json
```

Rules:

- `nocter check` parses, resolves, type-checks, and ownership-checks the same compile unit model as `nocter build`.
- `nocter check` does not emit an executable.
- `--format json` emits machine-readable diagnostics suitable for editor integrations.
- The complete `check` command contract is specified in [Command Line Interface](15-command-line-interface.md#check).
- JSON diagnostics use the same error codes and source concepts as human-readable diagnostics.
- The JSON envelope and diagnostic object are specified in [Diagnostics](12-diagnostics.md#machine-readable-json-diagnostics).
- The VS Code extension may translate JSON diagnostics into VS Code Problems.
- The VS Code extension must not synthesize semantic diagnostics that the compiler did not report.

Editor adapters should treat the JSON envelope as a transport format. They may map `diagnostics` into editor Problems, but they must not infer extra semantic diagnostics from syntax highlighting scopes or editor-local parsing.

Editor path mapping rules:

- Use `span.absolute_path` for editor document identity when it is present.
- Use `span.file` for human-facing display.
- Do not use display paths to decide whether two source files are the same file.
- LSP document mapping must follow the canonical source-file identity rules in [Modules and Imports](01-modules-imports.md#source-file-identity).

## Hover And Documentation

Adopted: hover text and generated API documentation must come from compiler-owned semantic data, not from editor-side parsing.

Rules:

- `///` and `/** ... */` document the next declaration, field, enum variant, method, function, or local binding that the compiler marks as documentable.
- `//!` and `/*! ... */` document the source file/module itself.
- `//` and `/* ... */` are normal comments and must not appear in hover text.
- Empty lines break attachment between a doc comment and the following construct.
- Multiple adjacent doc comments are concatenated in source order.
- `nocter ast app.nct --format json` may expose attached doc text through AST node `documentation` fields for tooling and tests.
- The VS Code extension must request hover data from `nocter lsp` once LSP support exists.
- Before LSP exists, the TextMate extension may highlight doc comments but must not infer hover text independently.

## AI-Assisted Tooling Stage

Nocter should be easy for AI tools to read, write, review, and repair without adding alternate syntax to the language.

Reserved command direction:

```sh
nocter tokens app.nct --format json
nocter ast app.nct --format json
```

Rules:

- `tokens` exposes the compiler lexer output for one source file.
- `ast` exposes the compiler parser output for one source file.
- Both commands are tooling and debugging commands, not user program execution commands.
- Both commands are JSON-only in v0.
- Both commands use compiler-owned source spans and token / AST node names.
- Both commands must not perform name resolution, type checking, ownership checking, target lowering, code generation, or execution.
- The JSON shapes are versioned and may evolve while the parser and AST are still unstable.
- AI tools should prefer `nocter fmt`, `nocter check --format json`, `nocter tokens --format json`, and `nocter ast --format json` over reimplementing syntax and semantics.

The compact AI-facing guide is [../AI.md](../AI.md). The example corpus is [examples/](examples/).

## LSP Stage

Long-term editor support should be provided by a compiler-backed language server.

Command direction:

```sh
nocter lsp
```

Rules:

- `nocter lsp` reuses the compiler lexer, parser, resolver, type checker, ownership checker, and diagnostics.
- LSP must not maintain a separate semantic model that can diverge from `nocter build` or `nocter check`.
- LSP features are added incrementally after diagnostics are reliable.
- The VS Code extension should become an LSP client when `nocter lsp` exists.
- The extension may keep TextMate grammar for baseline highlighting while semantic tokens mature.
- The LSP server is responsible for converting compiler byte spans to the client position encoding.

Feature order:

1. publish diagnostics
2. document symbols
3. go to definition
4. hover type information
5. completion for imports and visible names
6. rename
7. semantic tokens
8. formatting through `nocter fmt`

Formatting behavior is specified in [Source Style and Formatting](16-source-style-formatting.md). Editor extensions may call `nocter fmt`, but they must not maintain a separate formatter whose output can diverge from the compiler toolchain.

## Non-goals

The following are not part of the intended editor architecture:

- implementing a Nocter parser inside the VS Code extension
- implementing name resolution inside the VS Code extension
- implementing type checking inside the VS Code extension
- implementing borrow checking inside the VS Code extension
- maintaining a separate editor-only module graph
- maintaining a separate AI-only parser or semantic model
- making TextMate scopes part of the language specification
- requiring VS Code for normal command-line use
