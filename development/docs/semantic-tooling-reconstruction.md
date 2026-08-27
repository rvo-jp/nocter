# Semantic Tooling Reconstruction

This document defines the adopted v0.18.0 semantic-tooling reconstruction. It covers compiler
recovery, session composition, protocol-independent semantic queries, and editor features. It does
not change the Nocter language or runtime contract.

## Problem

The previous editor model represented one analysis attempt by its deepest reached phase and then
exposed optional facts from that phase. `SourceIndex` independently retained every reached source
binding. Feature implementations joined those two values themselves. A missing body therefore
meant any of the following without a type-level distinction:

- the body was rejected by an authored rule;
- the analysis attempt had not reached typed bodies;
- recovery deliberately retained only an earlier fact;
- compiler state was internally inconsistent.

Hover, semantic highlighting, inlay hints, navigation, rename, signature help, completion, and code
actions consequently assigned different meanings to the same absence. Recovery could also retain a
missing body after the session discarded the diagnostic that caused the absence. A feature-local
check cannot repair that model.

## Required Model

Compiler recovery publishes explicit evidence, not a deepest-phase value plus `Option` fields.
Every declared body has exactly one evidence state in a body-analysis result:

```text
BodyEvidence
|-- Typed(CheckedBody)
`-- Rejected(BodyRejection)
        |-- authored diagnostic or incomplete-syntax reason
        `-- exact typed interruption evidence when one exists
```

An internal failure cannot produce partial semantic evidence. It terminates recovery because no
source-level reason can justify a missing fact.

Name analysis follows the same rule. Every body is resolved or rejected with a source-backed
reason. Later session composition must retain the complete phase report even when command-line
presentation selects one canonical primary diagnostic.

Set-valued queries additionally publish coverage:

```text
Coverage
|-- Complete
|-- Partial(rejected semantic domains)
`-- Unavailable(no semantic evidence)
```

References, workspace symbols, and other set queries cannot represent a partial set as a complete
ordinary result. Mutation queries such as rename require complete coverage by type.

## Responsibility Boundaries

- Checking owns the reason why one semantic fact was accepted or rejected.
- Session composes complete phase reports. It does not discard a later analysis diagnostic merely
  because an earlier production error remains the command's canonical failure.
- `SourceIndex` remains only a bidirectional source-coordinate and semantic-identity projection. It
  does not acquire phase flags, recovery policy, or feature behavior.
- Analysis owns the only join between source occurrences and semantic evidence. Before exposing a
  query context, it validates every identity referenced by `SourceIndex`: authored bindings,
  documentation owners, and editor-visible names. The result is sealed once per immutable
  generation and cached; feature code has no access to an unvalidated context. A dangling entity
  is therefore an integrity failure for the generation, never a feature-specific empty result.
- Editor features consume typed query results. They cannot inspect checking recovery or join raw
  `SourceIndex` bindings to bodies.
- The language server maps protocol-independent outcomes to LSP. Only an integrity failure becomes
  JSON-RPC `-32603`; expected unavailability and partial coverage are ordinary semantic outcomes.

Each responsibility knows only the contract exported by the previous boundary. Protocol code does
not know checking representation, checking does not know editor features, and `SourceIndex` does
not know either.

## Migration Boundary

The JSON-RPC codec, LSP schema, server lifecycle, document overlay, generation ownership, and
UTF-8/UTF-16 coordinate authority remain in place. The following semantic stack is replaced:

1. sparse optional name and body recovery;
2. session outcomes that pair one error with an unrelated deepest semantic stage;
3. analysis feature modules that interpret missing facts independently;
4. language-server semantic handlers backed by those feature-local joins.

No compatibility adapter may translate the new evidence model back into the old deepest-stage
contract. Features remain unavailable until migrated to the shared query boundary.

## Completion Gate

The reconstruction is complete only when:

- every recovered body and name domain has an explicit accepted or rejected state;
- every rejected authored domain retains its source diagnostic in the same immutable result;
- internal failures cannot construct source-semantic recovery;
- session composition retains every diagnostic that explains retained evidence;
- no analysis feature directly joins `SourceIndex` with checking bodies or scopes;
- an unsealed semantic context is private to the query kernel, and whole-index validation is
  performed at most once per generation;
- no language-server feature depends directly on checking or source-index representation;
- set queries distinguish complete and partial coverage, and mutation requires complete coverage;
- all expected unavailable states are ordinary query outcomes rather than internal errors;
- architecture gates enforce those dependency and type boundaries;
- a shared state-by-feature matrix covers complete, declaration-rejected, name-rejected,
  body-rejected, syntax-incomplete, and integrity-failure generations;
- the old semantic-stage query model and all compatibility wrappers are absent;
- workspace tests, warnings-denied Clippy, formatting, generated documentation, and repository
  integrity checks pass.
