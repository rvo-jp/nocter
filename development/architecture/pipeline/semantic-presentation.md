# Semantic Presentation Boundary

This document owns the compiler-to-editor contract shared by session, source projection, analysis,
workspace analysis, and the language server. Internal mechanisms belong in those crates' colocated
READMEs.

## Pipeline

```text
session semantic evidence + validated SourceIndex
        |
        v
immutable AnalysisSnapshot and typed query capabilities
        |
        v
workspace generation
        |
        v
LSP result projection
```

The compiler selects semantic identities and canonical presentation facts. Analysis is the only
owner allowed to join those facts with source occurrences. The language server converts validated
query results to LSP coordinates and schemas; it cannot inspect compiler storage or reconstruct a
semantic description from source text.

## Generation Contract

One analysis snapshot contains one overlay, reached source/syntax generation, diagnostics, source
projection, and exclusive semantic-evidence outcome. Analysis validates their complete identity and
origin domains once before exposing query capabilities. A stale successful generation cannot answer
for a failed current generation.

Workspace analysis freezes one topology and compilation demand for each accepted document revision.
Every semantic response selects one immutable generation. Protocol requests never assemble their
own compiler session, source projection, or workspace scope.

## Availability and Coverage

Features depend on typed semantic capabilities, not a phase ordinal. An expected unavailable fact is
an ordinary empty/unavailable query result. An inconsistent generation is an integrity failure.

Set-valued queries state `Complete`, `Partial`, or `Unavailable` coverage. Rename and other semantic
mutations require complete coverage and one validated candidate snapshot. Protocol projection
cannot publish edits separately from the source grouping and document versions accepted by
analysis.

## Identity Presentation

Hover, completion, signature help, semantic tokens, navigation, references, and inlay hints render
the exact selected semantic identity. Compiler-owned presentation supplies normalized declarations,
types, owners, requirements, provenance, and documentation. Source binding ranges include only the
semantic token or authored path selected by the projection, never surrounding keywords, whitespace,
or declaration bodies.

Nominal type hover presents the nominal declaration only. Construction entries, fields, variants,
associated types, methods, interface implementations, locals, parameters, and catch bindings use
their own identities and ranges. Visible source spelling may differ from canonical identity without
changing semantics.

## Recovery Contract

Recovered name/body evidence explicitly classifies each domain as accepted or rejected with a
source-backed reason. Incomplete syntax may expose facts fixed before the hole, but it cannot invent
a checked node, dispatch, name target, or complete result set. Complete declarations such as `see`,
`use`, and target gates remain available when their own syntax nodes are complete.

## Required Invariants

- `SourceIndex` never decides semantic identity.
- Analysis is the only semantic/source join authority.
- Feature modules cannot implement independent recovery fallback order.
- Protocol code receives typed results, not compiler stores.
- Every interactive range is exact and half-open.
- Mutations are validated against the complete derived generation before publication.
