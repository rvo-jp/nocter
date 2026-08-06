# Tooling and Editor Integration

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Compiler Authority

Editor and AI tooling must consume compiler-owned lexical, syntax, resolution, type, ownership,
diagnostic, formatting, and source-edit results. It must not maintain a second semantic model that
can diverge from `nocter check` or `nocter build`.

The supported compiler entry points are:

```sh
nocter check --format json
nocter tokens app.nct --format json
nocter ast app.nct --format json
nocter lsp
```

`tokens` and `ast` inspect one file without resolution, typechecking, lowering, or execution.
Diagnostics use the envelope specified in [Diagnostics](12-diagnostics.md). The language server
reuses the full compiler pipeline and package model.

## Documentation

Hover and generated API documentation use compiler-attached Markdown documentation:

- `///` and `/** ... */` document the following declaration or documentable member.
- `//!` and `/*! ... */` document the source module.
- Ordinary `//` and `/* ... */` comments are not documentation.
- An empty line ends attachment.
- Adjacent documentation comments are concatenated in source order.

Hover on an imported module path presents the resolved module documentation when available.
Tooling must not infer documentation from nearby raw text after the compiler reports no attachment.

## Language Server Snapshot

One accepted document version belongs to one immutable, generation-numbered package snapshot.
Diagnostics, hover, completion, signature help, definition, references, rename, code actions,
inlay hints, and semantic tokens for that request observe the same generation.

An open document overrides disk content throughout its generation. Package graphs for open
`nocter.nct` overlays are locked, offline, and read-only: analysis never fetches dependencies,
generates locks, or rewrites source. Changed imports invalidate reverse importers; unrelated modules
and nested packages retain independent state.

A failed source or package load retains every reached source and unresolved candidate needed for
future invalidation. A failed graph is not replaced by a stale successful graph.

## Semantic Ranges

Every semantic result uses the exact token or identifier that carries the fact. Keywords,
visibility modifiers, owners, whitespace, delimiters, braces, and bodies are excluded unless the
request explicitly targets that syntax.

Examples:

- a declaration hover focuses its declared name
- a method receiver binding focuses `self`, while the semantic owner type remains display data
- a field or variant focuses its member name and may display the qualified `Type.member`
- an import module path is one namespace range
- executable and test entry strings use their content without quotation marks
- a native test declaration focuses its test name, not `test` or its body

Internal canonical identities may contain package/module qualification. User presentation chooses
the shortest unambiguous visible spelling and must not leak storage paths such as
`std/iter/core.Type` into ordinary signatures.

## Semantic Tokens

Semantic tokens are emitted only for compiler-resolved facts. Unresolved identifiers and module
paths do not receive guessed semantic classifications.

The `readonly` modifier means assignment with `=` is invalid at that exact token position. It does
not mean the declaration used a readonly keyword. Binding and parameter tokens are readonly when
the whole binding cannot be assigned; field access is `property.readonly` when that access path is
not a writable place. Field declarations are not readonly merely because some uses are borrowed.

Package directive names, native test names, interface members, implementation members, closure
parameters, catch bindings, and method receivers retain their semantic declaration kinds and exact
ranges.

## Hover and Signature Help

Hover and signature help render normalized compiler declarations, not raw source excerpts. They
preserve specialized generic arguments, every interface bound, callable capability, outcome layers,
allocation effects, and result provenance supplied by analysis.

Declaration owners are shown when they disambiguate a member. Construction hover presents the
type-owned public construction surface, including its default entry. Raw private construction and
inaccessible members are absent.

## Completion

Completion follows lexical scope, visibility, shadowing, receiver capability, generic bounds, and
the exact package graph. It includes accessible declarations, imports, members, enum-pattern
variants, unused struct fields, construction entries, native-test syntax, and relevant keywords.

Automatic imports consider only reached public exports in the active package, direct dependency
aliases, and `std`. The compiler supplies the additional top-level `use` edit while preserving
leading documentation and existing import groups. Private, unreachable, dependency-internal, and
standard-library-internal declarations are not candidates.

Member completion for a generic receiver combines its declared capability set. Distinct interfaces
with the same applicable member name are ambiguous; order never chooses one.

## Navigation, References, and Rename

Definition and references use semantic declaration identity rather than spelling. Package-wide
operations start from package roots and explicit executable/test entries, then follow normal imports;
they do not scan ambient `.nct` files.

Rename focuses one identifier, validates the replacement as a language identifier, rejects
collisions, and returns one atomic workspace edit. Open documents receive versioned edits; closed
documents receive unversioned edits. Dependencies and `std` are read-only regardless of filesystem
location, so a plan containing any non-owned occurrence is rejected as a whole.

A generic-bound call defines to the interface declaration. Concrete specialization may retain its
selected implementation target internally without changing that source-level definition result.

## Diagnostics, Code Actions, and Hints

Diagnostics are compiler results with stable codes, exact spans, related information, and optional
fix plans. The server clears diagnostics absent from the next complete publication and includes the
accepted document version.

Code actions expose compiler-planned edits, including imports, required interface members, and
optional/fallible callable contracts. Generated edits must parse and typecheck as ordinary Nocter;
the protocol layer does not synthesize source templates independently.

Inlay hints project retained inferred binding types, current-allocation effects, and result
provenance. Explicit source annotations suppress redundant hints. The language server performs no
second inference pass.

## Incomplete Source Recovery

Recovery may create a temporary syntax overlay for missing delimiters, call operands, imports,
member access, iteration headers, interpolation bodies, literal declarations, or provenance clauses.
Semantic results are returned only when the ordinary compiler query resolves the required
declaration and type identities. Recovery must not invent conformance, imports, members, or types,
and it never replaces the authoritative document generation.

## Protocol Lifecycle

The server validates initialization, shutdown, exit, request parameters, document versions, and
UTF-16 position conversion before invoking analysis. Requests before initialization, repeated
initialization, and requests after shutdown return protocol errors. Stale document changes are
ignored.

The server advertises saved-text synchronization and dynamically registers a `**/*.nct` watcher
when supported. Included `didSave` text is analyzed before diagnostics are published. Semantic-token
results carry a generation identifier suitable for rejecting stale cached results.

## TextMate and AI Tools

TextMate grammar may provide lexical highlighting, comments, brackets, quote completion, and
snippets. It must not define semantic validity, imports, types, ownership, or borrows. Semantic
tokens supersede guesses when compiler results are available.

AI tools should prefer compiler formatting, diagnostics, tokens, AST output, and LSP queries over a
separate Nocter parser. The compact generation guide is [AI Guide](guides/ai.md), and executable
packages live under the repository-root [examples directory](../examples/README.md).

## Non-goals

The tooling contract does not require VS Code, an editor-local module graph, editor-local type or
borrow checking, TextMate scopes as language semantics, a separate AI-only grammar, background
concurrent analysis, or persistent cross-session semantic caches.
