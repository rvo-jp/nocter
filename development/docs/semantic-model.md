# Semantic Identity and Typed Model

This document owns the v0.14.0 compiler identity and semantic-data boundaries. Public Nocter
syntax and behavior remain owned by the [language specification](../../spec/README.md). The active
work order and acceptance gates live in the
[v0.14.0 milestone](../milestones/v0.14.0.md).

## Problem

The v0.13.0 compiler has strong feature-specific plans, but no single semantic object graph. A
definition is variously identified by a `ByteSpan`, a canonical type string, a private synthetic
method name, a resolver-local `SymbolId`, or a stable editor span. Type-check facts are collected by
a later AST traversal, lowering reconstructs supported shapes from AST plus span maps, and editor
features retain their own syntax walkers. These representations can agree under tests while still
allowing new consumers to repeat or subtly change earlier decisions.

The migration makes identity and semantic ownership explicit. It does not hide the existing split
behind more helper functions.

## Identity Domains

Each ID is a newtype over a private integer allocated by one immutable compile-unit generation.
Different domains are not interchangeable.

| ID | Record | Never used for |
|---|---|---|
| `DefId` | declaration kind, owner, visibility, source anchor | source slicing or display text |
| `BodyId` | owning `DefId`, source body, parameters, root expression/block | callable naming |
| `ExprId` | owning body and typed expression node | cross-generation edit identity |
| `TyId` | interned normalized semantic type | source spelling |
| `RequirementId` | authored requirement and selected evidence | runtime witness lookup |
| `IntrinsicId` | validated target primitive role | standard-library public name lookup |
| `MonoItemId` | `DefId`, `TyId` substitutions, requirement evidence | emitted symbol presentation |

`SourceId` continues to identify loaded source within a generation. `ByteSpan` continues to locate
text. Neither is a declaration identity.

## Semantic Database

`SemanticDb` owns compact tables and typed lookup methods. Construction follows source loading and
syntax parsing, then resolution fills ownership and reference edges without changing IDs.

```text
SemanticDb
  definitions: DefId -> Definition
  bodies:       BodyId -> BodyRecord
  expressions:  ExprId -> TypedExpr        (Phase 1)
  types:        TyId -> Ty
  requirements: RequirementId -> Evidence
  intrinsics:   IntrinsicId -> Intrinsic
  locations:    SemanticId -> SourceLocation
```

The database is passed as one immutable semantic context after construction. Phase-specific mutable
builders may allocate records, but completed passes do not receive writable access to earlier
tables. Diagnostics ask the database for an ID's primary or focus span. LSP maps a package-stable
editor identity to the current generation's `DefId` and then uses the same records as compilation.

## Syntax and Semantics

AST nodes preserve authored structure and token ranges. They do not contain resolved types,
inferred origins, target names, or invented declarations. In particular:

- `InstanceDecl` stores a source-ordered `Vec<InstanceMember>`;
- operator declarations store operator syntax and callable syntax without a method name;
- coercion declarations store receiver, target, provenance clause, and body without a method name;
- source order is never reconstructed by sorting spans after parsing;
- adapters expose borrowed signature/body views only while a consumer migrates to semantic records.

The definition index assigns `DefId` independently of public name. Duplicate declarations therefore
receive distinct IDs before diagnostics report the conflict. Reexports and aliases reference the
original definition plus their own import occurrence; they do not clone definition identity.

## Typed Semantic Result

Phase 1 changes checking from `diagnostics + span maps` into a typed result:

```text
CheckedUnit {
  db,
  typed_bodies,
  diagnostics,
}
```

Every expression node records its `ExprId`, `TyId`, value category, ownership transition,
provenance, selected callable/operator/coercion/requirement IDs, and required adjustments. Invalid
or incomplete source records explicit error nodes and missing edges. Later phases may render or
lower this result; they may not invoke selectors again.

## Migration Enforcement

- New semantic maps use typed IDs as keys. A new `HashMap<ByteSpan, SemanticFact>` requires a
  documented diagnostic-only reason.
- New call targets use `DefId` or `IntrinsicId`, never a `String`.
- An adapter module names its replacement phase in a comment and has no public export outside the
  compiler crate.
- Tests compare source presentation only at formatter, diagnostic, or LSP boundaries. Semantic
  tests compare IDs and records.
- Each migration commit removes at least one old authority or prevents new users of it; table-only
  scaffolding is not considered progress.
