# Nocter Language Specification

This file is the public entry point for the Nocter language specification.
The detailed specification is split by topic under [`spec/`](spec/) so each design area can evolve without turning this file into a monolith.

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.
Its compiler is intended to produce native executables directly, without requiring an external assembler, linker, runtime library, or platform SDK for normal users.

## Chapters

- [Overview](spec/00-overview.md)
- [Modules and Imports](spec/01-modules-imports.md)
- [Values and Types](spec/02-values-types.md)
- [Control Flow](spec/03-control-flow.md)
- [Errors and Optionals](spec/04-errors-optionals.md)
- [Ownership, Borrowing, and Drop](spec/05-ownership-borrowing-drop.md)
- [Memory, Regions, and Allocators](spec/06-memory-region-allocator.md)
- [Strings, Arrays, Views, and Pointers](spec/07-strings-arrays-views-pointers.md)
- [Generics, Traits, and Methods](spec/08-generics-traits-methods.md)
- [ABI and Layout](spec/09-abi-layout.md)
- [Targets and Distribution](spec/10-targets-distribution.md)
- [Standard Library, Primitives, and OS](spec/11-stdlib-primitives-os.md)
- [Diagnostics](spec/12-diagnostics.md)
- [Lexical Grammar](spec/13-lexical-grammar.md)
- [Tooling and Editor Integration](spec/14-tooling-editor-integration.md)
- [Command Line Interface](spec/15-command-line-interface.md)

## Editing Policy

- Keep this file as the table of contents and high-level entry point.
- Put normative language rules in the relevant `spec/*.md` chapter.
- Keep examples close to the rule they explain.
- When a design is still provisional, mark it explicitly in that chapter instead of hiding uncertainty in broad wording.
