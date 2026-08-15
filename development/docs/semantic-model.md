# Semantic Identity and Typed Model

This document owns the v0.14.0 compiler identity and semantic-data boundaries. Public Nocter
syntax and behavior remain owned by the [language specification](../../spec/README.md). The active
work order and acceptance gates live in the
[v0.14.0 milestone](../milestones/v0.14.0.md).

## Migration Context

The published v0.13.0 compiler had strong feature-specific plans, but no single semantic object
graph. Definitions were variously identified by a `ByteSpan`, canonical type string, private
synthetic method name, resolver-local `SymbolId`, or stable editor span. Phase 0 replaced source
declaration and body equality with a compile-unit identity domain. Phase 1 made type checking own
an error-tolerant typed result. Phase 2 moved editor projection and the remaining type-occurrence
and binding identity to semantic indexes. Phase 3 replaced AST-shaped control-flow lowering with
checked MIR.

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
| `OpaqueTypeId` | one authored anonymous `some Interface` result | named-declaration lookup |
| `TyId` | interned normalized semantic type | source spelling |

`IntrinsicId` is the closed identity domain for compiler-recognized primitive operations; source
names are converted once and never drive backend dispatch. `RequirementId` and `MonoItemId` remain
planned domains. They are not introduced as empty wrappers before their phases can remove the
corresponding old authority.

`SourceId` continues to identify loaded source within a generation. `ByteSpan` continues to locate
text. Neither is a declaration identity.

## Semantic Database

`SemanticDb` owns compact syntax-identity tables and typed lookup methods. Construction follows
source loading and parsing; resolution refers to those records without changing IDs. The checked
file's `TypedHir` owns the type arena and partial expression semantics for the same immutable
generation.

```text
SemanticDb
  definitions: DefId -> Definition
  bodies:       BodyId -> BodyRecord
  expressions:  ExprId -> ExpressionRecord
  locations:    DefId / BodyId / ExprId -> ByteSpan

TypedHir
  typed expressions: ExprId -> PartialSemantic<TyId>
  types:             TyId -> normalized TypeExpr
  type occurrences: source occurrence -> DefId
  binding facts:    LocalSymbolId -> checked binding semantics
  syntax facts:     source occurrence -> checked expression plan
```

## Editor Projection

Editor services consume two retained projections built during compile-unit analysis:

- `SemanticOccurrenceIndex` maps source occurrences to `DefId` or `LocalSymbolId`, their role, and
  contextual checked type.
- `EditorSyntaxIndex` retains syntax-only cursor sites such as calls, literals, interpolation,
  module paths, import selectors, documentation attachment, and callable source snapshots.

Resolver surfaces have a separate `DefId`-keyed declaration index. A field, variant, method,
literal, or coercion is selected by ID first; its owner and source presentation are returned from
that index. Editor code does not scan every type and compare member spans. Call-specialization
closure is a lazy compile-unit fact shared by buildability and lowering rather than recomputed by
each consumer.

`EditorSyntaxIndex` includes completion sites and `LexicalScopeIndex` includes visible locals and
scoped imports. Successful-source completion uses those retained indexes. AST scope walking is
confined to explicitly named recovery paths used only after incomplete-source parsing; recovery
cannot replace a successful semantic result.

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

Phase 1 changed checking from an externally recollected fact bundle into one typed result:

```text
TypecheckOutput {
  diagnostics,
  typed_hir: {
    expressions: ExprId -> {
      body: BodyId,
      intrinsic_ty: Known(TyId) | Error,
      contextual_ty: Option<TyId>,
      diverges: bool,
    },
    types: TyId -> normalized TypeExpr,
    compatibility facts,
  },
}
```

Every authored expression has an `ExprId` and owning `BodyId`. Its intrinsic type is either a known
`TyId` or an explicit error. A separate contextual `TyId` records the checked effective type when
the surrounding return, argument, assignment, or other expectation changes representation. The
compiler does not overwrite an integer literal's intrinsic type merely because an argument expects
`usize`, and lowering does not repeat that conversion decision. Value-category, ownership,
provenance, call, operator, coercion, and adjustment facts remain in the same checker-owned
`TypedHir`. Expression semantics use `ExprId`, declaration targets use `DefId`, and binding facts
use `LocalSymbolId`.

Divergence is retained independently from contextual type. A call returning `never` may inhabit a
scalar return context, but its MIR continuation is still non-returning. MIR construction therefore
does not inspect a type alias or compare an authored type name with the string `never`.

Phase 3 constructs checked MIR once per specialized `BodyId` and retains it in compile-unit
analysis.
Buildability follows `DefId` call edges in that body, while machine-IR lowering consumes the same
validated object. Returning calls identify their result place and successor block; non-returning
calls have no fabricated destination or successor. A trapping fallible call identifies distinct
success and trap successors, establishing the same representation boundary that propagated and
handled failure paths will extend. Retained occurrence spans locate syntax-only
operations and diagnostics; path-sensitive execution facts belong in MIR rather than treating
those locations as identity. Invalid or incomplete source therefore remains partial rather than
becoming an alternate successful model. Later phases may render or lower this result; they may not
invoke selectors again.

Phase 4 makes storage projection equally explicit. A checked `TyId` is projected through the
shared ABI authority and cached for one machine body. The callable return contract, parameter
ordinal mapping, integer operator, and aggregate range are typed values; a backend consumer cannot
reconstruct any of them from source names, parameter names, or a repeated AST type classification.

## Migration Enforcement

- New semantic maps use typed IDs as keys. A new `HashMap<ByteSpan, SemanticFact>` requires a
  documented diagnostic-only reason.
- Semantic call selection uses `DefId`. Backend linkage strings are generated only after selection
  and never flow back into resolver or type-check equality.
- An adapter module names its replacement phase in a comment and has no public export outside the
  compiler crate.
- Tests compare source presentation only at formatter, diagnostic, or LSP boundaries. Semantic
  tests compare IDs and records.
- Each migration commit removes at least one old authority or prevents new users of it; table-only
  scaffolding is not considered progress.
