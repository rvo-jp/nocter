# Nocter Language Specification

This file is the public entry point for the Nocter language specification.
The detailed specification is split by topic in this directory so each design area can evolve without turning this file into a monolith.

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.
Its compiler is intended to produce native executables directly, without requiring an external assembler, linker, runtime library, or platform SDK for normal users.
Nocter also treats AI readability and writability as a tooling goal: the language keeps one canonical style, machine-readable diagnostics, and an example corpus instead of adding multiple alternate spellings.

## Chapters

- [Overview](00-overview.md)
- [Nocter v0 Contract](00-v0-contract.md)
- [Modules and Imports](01-modules-imports.md)
- [Values and Types](02-values-types.md)
- [Control Flow](03-control-flow.md)
- [Errors and Optionals](04-errors-optionals.md)
- [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md)
- [Memory, Regions, and Allocators](06-memory-region-allocator.md)
- [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md)
- [Generics and Methods](08-generics-traits-methods.md)
- [ABI and Layout](09-abi-layout.md)
- [Targets and Distribution](10-targets-distribution.md)
- [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md)
- [Diagnostics](12-diagnostics.md)
- [Lexical Grammar](13-lexical-grammar.md)
- [Tooling and Editor Integration](14-tooling-editor-integration.md)
- [Command Line Interface](15-command-line-interface.md)
- [Source Style and Formatting](16-source-style-formatting.md)

## Supporting Material

- [AI Guide](guides/ai.md)
- [Example Corpus](examples/)

## Editing Policy

- Keep this file as the table of contents and high-level specification entry point.
- Put normative language rules in the relevant `spec/*.md` chapter.
- Keep examples close to the rule they explain.
- Keep `guides/ai.md` compact and example-oriented; do not let it replace normative specification chapters.
- When a design is still provisional, mark it explicitly in that chapter instead of hiding uncertainty in broad wording.
