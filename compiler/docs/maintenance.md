# Compiler Maintenance Policy

This document records long-lived maintenance rules for the Nocter compiler.
Short-term handoff notes belong in `../TODO.md`.
Normative language rules belong in `../../spec/`.

## Goal

Keep the compiler structure maintainable as it grows from a small bootstrap implementation to a multi-phase compiler.

Small diffs are not the primary goal.
A change is successful when the next feature can be added without duplicating semantics, hiding phase boundaries, or increasing accidental coupling between unrelated compiler layers.

## Module Boundary Rules

Split by responsibility and abstraction layer, not by line count.

Good module boundaries have these properties:

- callers can name what the module owns
- the module exposes a small API instead of forcing callers to inspect internal data
- tests can target the module without constructing unrelated compiler phases
- future features in the same phase naturally reuse the module

Avoid these shapes:

- a driver module that owns language semantics
- a type checker module that formats diagnostics directly
- a backend module that re-parses source-level syntax
- an LSP feature that resolves names without using compiler analysis
- a single file that mixes transport, document state, semantic analysis, and response rendering

## LSP Structure

`driver/lsp/` should move toward this structure:

```text
driver/lsp/
    mod.rs          # server loop, request routing, feature orchestration
    protocol.rs     # JSON-RPC framing, LSP position/range helpers
    documents.rs    # open document state, URI/path handling, sync params
    diagnostics.rs  # publishDiagnostics conversion and stale clearing
    semantic.rs     # semantic token classification
    hover.rs        # hover contents and hover symbol lookup
    definition.rs   # go-to-definition lookup and locations
    completion.rs   # completion items
    symbols.rs      # document symbols
    analysis.rs     # bridge from open documents to compiler analysis
```

Do not create all files preemptively.
Extract a module when there is a stable responsibility boundary and the extracted API is smaller than the copied context.

LSP must treat the compiler as the semantic source of truth.
TextMate grammar, snippets, and editor-side code may improve presentation, but name resolution, type information, diagnostics, and syntax interpretation must come from compiler modules.

## Refactoring Rules

Refactor before adding a feature when the current design would require:

- copying traversal logic
- adding protocol-specific fields to core compiler data
- exposing mutable internals across phases
- adding another large helper to an already mixed-responsibility file
- testing through a full CLI/LSP path when a phase-level test would be clearer

Prefer small APIs with owned result types over returning raw internal maps.
Prefer compiler-phase tests for compiler semantics and LSP tests for protocol behavior.

## Progress Recording

Every session that changes compiler behavior or structure should leave the repository easier to resume.

Use this rule:

- update `../TODO.md` for the next concrete action and current uncommitted state
- update `implementation-status.md` when user-visible behavior changes
- update `roadmap.md` when the recommended next task changes
- update `architecture.md` when module responsibilities change

Do not record every command or every passing test in long-lived documents.
Record only durable facts: implemented capability, remaining gap, design decision, or next action.

## Commit Hygiene

Keep unrelated user changes out of commits.

Prefer these commit shapes:

- one behavior change plus tests and docs
- one structural refactor with no behavior change
- one documentation update that changes development policy

Avoid commits that mix:

- generated assets and compiler logic
- root project files and compiler internals without a shared purpose
- formatting-only churn and semantic changes

When a rename plus extraction is needed, make the extraction behavior-preserving first, then continue feature work in a later change.
