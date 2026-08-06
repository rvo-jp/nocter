# Nocter Language Specification

This directory is the authoritative source for Nocter language behavior.
It is written for Nocter users, standard-library authors, editor/tool authors,
and AI assistants that need to generate or analyze Nocter source.

The detailed specification is split by topic so each design area can evolve
without turning this file into a monolith.

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.
Its compiler is intended to produce native executables directly, without requiring an external assembler, linker, runtime library, or platform SDK for normal users.
Nocter values simplicity, encapsulation, and foolproof design.
It also treats AI readability and writability as a tooling goal: the language keeps one canonical style, machine-readable diagnostics, and an example corpus instead of adding multiple alternate spellings.

## Chapters

- [Design Principles](00-design-principles.md)
- [Overview](00-overview.md)
- [Nocter v0.2.0 Language Contract](00-v0.2.0-contract.md)
- [Modules and Use Declarations](01-modules-use.md)
- [Values and Types](02-values-types.md)
- [Control Flow](03-control-flow.md)
- [Errors and Optionals](04-errors-optionals.md)
- [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md)
- [Memory, Regions, and Allocators](06-memory-region-allocator.md)
- [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md)
- [Generics, Interfaces, Embedding, and Methods](08-generics-interfaces-embedding-methods.md)
- [ABI and Layout](09-abi-layout.md)
- [Targets and Distribution](10-targets-distribution.md)
- [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md)
- [Diagnostics](12-diagnostics.md)
- [Lexical Grammar](13-lexical-grammar.md)
- [Tooling and Editor Integration](14-tooling-editor-integration.md)
- [Command Line Interface](15-command-line-interface.md)
- [Source Style and Formatting](16-source-style-formatting.md)
- [Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md)
- [Callable Values and Interface Default Methods](18-callables-default-methods.md)
- [Construction Surfaces](19-construction-surfaces.md)
- [Native Testing](20-native-testing.md)

## Supporting Material

- [AI Guide](guides/ai.md)
- [Example Corpus](examples/)

## Version Status

The released language baseline is v0.4.0. Its package-root, deterministic dependency-graph,
immutable editor-snapshot, and release-qualification contracts are recorded in the
[v0.4.0 Release Record](../development/docs/v0.4.0.md). The
[v0.3.0 Release Record](../development/docs/v0.3.0.md) and
[v0.2.0 Language Contract](00-v0.2.0-contract.md) remain historical boundaries.

The adopted v0.5.0 Phase 2 contract adds compiler-owned native test declarations,
per-declaration process isolation, and `std/testing`. The completed v0.5.0 Phase 3 contract adds an
immutable exact-graph editor index, package-wide references and rename, compiler-planned source
edits, and semantic inlay hints. These contracts are specified here before the v0.5.0 release so
packages and tooling share one source of truth.

## Editing Policy

- Keep this file as the table of contents and high-level specification entry point.
- Put normative language and public standard-library rules in the relevant
  `spec/*.md` chapter.
- Keep examples close to the rule they explain.
- Keep `guides/ai.md` compact and example-oriented; do not let it replace normative specification chapters.
- When a design is still provisional, mark it explicitly in that chapter instead of hiding uncertainty in broad wording.
- Keep compiler implementation status, backend work plans, and handoff notes in
  `../development/`.
