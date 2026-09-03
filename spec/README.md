# Nocter Language Specification

This directory is the sole normative source for Nocter language behavior, public standard-library
APIs, command-line behavior, and editor-facing contracts. It is written for Nocter users,
standard-library authors, editor/tool authors, and AI assistants that generate or analyze Nocter
source.

The detailed specification is split by topic so each design area can evolve
without turning this file into a monolith.

Nocter is a statically typed, value-centered, module-oriented, low-dependency systems language.
Its compiler is intended to produce native executables directly, without requiring an external assembler, linker, runtime library, or platform SDK for normal users.
Nocter values simplicity, encapsulation, and foolproof design.
It also treats AI readability and writability as a tooling goal: the language keeps one canonical style, machine-readable diagnostics, and an example corpus instead of adding multiple alternate spellings.

## Chapters

- [Design Principles](00-design-principles.md)
- [Overview](00-overview.md)
- [Modules, Use Declarations, and Source Visibility](01-modules-use.md)
- [Values and Types](02-values-types.md)
- [Control Flow](03-control-flow.md)
- [Errors and Optionals](04-errors-optionals.md)
- [Ownership, Borrowing, and Drop](05-ownership-borrowing-drop.md)
- [Memory, Regions, and Allocators](06-memory-region-allocator.md)
- [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md)
- [Generics, Interfaces, and Methods](08-generics-interfaces-embedding-methods.md)
- [ABI and Layout](09-abi-layout.md)
- [Targets and Distribution](10-targets-distribution.md)
- [Standard Library, Primitives, and OS](11-stdlib-primitives-os.md)
- [Diagnostics](12-diagnostics.md)
- [Lexical Grammar](13-lexical-grammar.md)
- [Syntactic Grammar](25-syntactic-grammar.md)
- [Tooling and Editor Integration](14-tooling-editor-integration.md)
- [Command Line Interface](15-command-line-interface.md)
- [Source Style and Formatting](16-source-style-formatting.md)
- [Argument Packs, Literal Definitions, and Sequence Spread](17-argument-packs-literals-sequence-spread.md)
- [Callable Values and Interface Default Methods](18-callables-default-methods.md)
- [Construction Surfaces](19-construction-surfaces.md)
- [Native Testing](20-native-testing.md)
- [Practical Standard Library](21-practical-standard-library.md)
- [Borrow Coercions](22-borrow-coercions.md)
- [Expansion Operators](23-expansion-operators.md)
- [Strict Ordering Operators](24-ordering-operators.md)
- [Compile-Time Constants](26-constants.md)
- [Associative Collections](27-associative-collections.md)
- [JSON Values and Text](28-json.md) — v0.22.0 standard-library contract
- [Monotonic Time](29-monotonic-time.md)
- [Synchronous Subprocesses](30-subprocesses.md) — v0.30.0 standard-library contract
- [Captured Subprocess Output](31-subprocess-output.md) — v0.31.0 standard-library contract
- [Configured Synchronous Subprocesses](32-configured-subprocesses.md) — v0.32.0 candidate contract

## Supporting Material

- [AI Guide](guides/ai.md)
- [Runnable Examples](../examples/README.md)

## Contract Status

This working tree specifies the published v0.31.0 language and standard-library contract,
including the v0.20.0
interface-prerequisite foundation, v0.21.0 associative collections, v0.22.0 JSON standard module,
the complete v0.23.0 type-owned integer text surface, and explicit module subjects for external
values. v0.25.0 adds the source-visible `noalloc` callable guarantee. v0.26.0 adds normalized
durations, monotonic elapsed-time measurement, and blocking sleep. v0.27.0 completes lexical
UTF-8 path inspection and explicit directory lifecycle operations. v0.28.0 adopts practical
ASCII text transformations, a private scalar-formatting authority, and symmetric text output.
v0.29.0 adds borrowed standard input and an exact child-argument channel for `nocter run`. v0.30.0
adds the owning synchronous-subprocess contract. v0.31.0 adds simultaneous stdout and stderr
capture to that closed synchronous lifecycle. Publication status belongs to the release index.
Implementation phases and qualification evidence belong only to contributor records and are
intentionally not restated in the public specification. The
[release index](../releases/README.md) owns current publication and download status, and repository
tags preserve the exact specification for every published release.

The implemented [v0.32.0 candidate contract](32-configured-subprocesses.md) completes configured
synchronous commands with child environment, working-directory, and finite-input ownership. It is
qualified in the current tree but remains unavailable in published v0.31.0 artifacts.

A chapter states current behavior unless a section is explicitly labeled **Future Direction** or
**Non-goal**. Development phases and compiler work order do not change the meaning of an otherwise
unqualified rule.

Compiler architecture, backend boundaries, qualification evidence, and repository workflows are
non-normative and live under `development/`.

## Editing Policy

- Keep this file as the table of contents and high-level specification entry point.
- Put normative language and public standard-library rules in the relevant
  `spec/*.md` chapter.
- Keep examples close to the rule they explain.
- Keep `guides/ai.md` compact and example-oriented; do not let it replace normative specification chapters.
- Mark proposed behavior under an explicit **Future Direction** heading. Do not mix implementation
  phases or work plans into current rules.
- Keep compiler implementation status, backend work plans, and handoff notes in
  `../development/`.
