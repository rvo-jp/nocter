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

Source after a proven terminal statement remains name-resolved and typed when independently valid,
so definition, references, hover, completion, and semantic tokens use the same declaration and type
identities there. Flow-sensitive ownership facts are absent rather than fabricated for that
unreachable continuation. Tooling may later expose an unreachable-code lint, but such a lint is not
a language diagnostic.

## Documentation

Hover and generated API documentation use compiler-attached Markdown documentation:

- `///` and `/** ... */` document the following declaration or documentable member.
- `//!` and `/*! ... */` document the source module.
- Ordinary `//` and `/* ... */` comments are not documentation.
- An empty line ends attachment.
- Adjacent documentation comments are concatenated in source order.

The lexical chapter defines the single Markdown-extraction rule used by AST output, semantic
indexes, hover, and future generated documentation. These consumers do not independently strip or
reformat comment text.

File documentation has one public owner. Documentation in `nocter.nct` belongs to its package.
Documentation in a directory module's `index.nct`, or in the source of single-file mode, belongs to
that module. File documentation in a module implementation source remains available on that
source's syntax snapshot and AST output, but it is not appended to or allowed to replace the public
module documentation.

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
`std/iter.Type` into ordinary signatures. The compiler derives that spelling from the resolved
namespace graph, including local import aliases and visible module exports; protocol adapters do not
recover it from source text.

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
intrinsic `copy` requirements, callable `where` clauses, and source-visible result provenance
supplied by analysis. Compiler-owned execution allocation and
fresh-result storage do not appear as signature prose.

Declaration owners are shown when they disambiguate a member. Construction hover presents the
type-owned public construction surface, including its default entry. Raw private construction and
inaccessible members are absent.

When type hover expands a construction surface, it uses Nocter declaration syntax rather than an
invented surface language. An accessible structural entry is represented by the nominal `struct`
declaration with every required visible field. Intrinsic enum entries are represented by the
nominal `enum` declaration with its visible variants. An authored surface is represented by a
bodyless `construct Type { ... }` declaration;
its members remain unqualified inside the block, use `Self` in their result contracts, preserve
declaration order, and retain the explicit `default` modifier. The compiler does not invent a
`fields` declaration, revive removed top-level literal or qualified-function forms, or include
member bodies. A struct whose structural entry is unavailable remains a header-only type
presentation unless it has another visible construction entry.

Associated type hover uses the interface-owned declaration identity. A declaration or projection
is shown as `associated type Interface.Name`; a conformance binding is shown as
`type Interface.Name = ConcreteType`. Normalized callable presentation preserves a generic
`Self.Name` or `T.Name` projection until concrete specialization selects a binding.

## Completion

Completion follows lexical scope, visibility, shadowing, receiver capability, generic bounds, and
the exact package graph. It includes accessible declarations, imports, members, enum-pattern
variants, unused struct fields, construction entries, native-test syntax, and relevant keywords.
`copy` is offered only in a generic requirement context where it is not already present.
Construction completion uses the use-site construction view, not hover's public-presentation view,
so private construction available inside the defining module remains available to its source.
After a construction owner followed by `.`, completion offers only named entries expressible in
that position: accessible enum variants and construction functions. Structural construction and
typed literals use their own delimiters and are not invented as dot members. Explicit generic
arguments select the same type-family surface as inferred arguments, and a built-in type with no
named construction surface produces no dot candidates. Complete members, an invalid member name,
and a missing member name use the same compiler-owned entry identities, visibility decision, and
declaration order.

Automatic imports consider only reached exports whose resolved boundary contains the current
module: ancestor- and package-visible exports in the active package, bare-public exports in direct
dependencies, and the public surface of the implicit `std` package. The compiler supplies the
additional top-level `use` edit while preserving leading documentation and existing import groups.
Private, unreachable, dependency-internal, and standard-library-internal declarations are not
candidates. A candidate whose added module edge would create an import cycle is also excluded;
top-level and block imports both participate in that proof.

Member completion for a generic receiver combines its declared capability set. Distinct interfaces
with the same applicable member name are ambiguous; order never chooses one.

Type-position completion after `Self.` or a bounded parameter offers only associated types from
the resolved interface requirements. It does not infer a member from a spelling or from an
unresolved interface.

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
The target name in a callable `where` clause defines, references, and renames as the corresponding
generic parameter; contextual `copy` and `where` tokens never acquire declaration identities.
An associated type declaration owns the identity shared by its conformance bindings and projected
uses. Definition, references, and rename cross imports through that identity rather than treating
each `Item` spelling as a separate symbol.

In `some Interface<Item = T>`, semantic tokens classify only `some` as a contextual keyword. The
interface and associated binding names use their declaration identities. Hover, signature help,
completion, navigation, references, rename, and inlay text render the authored opaque contract and
must not reveal the concrete witness. Member completion offers only the advertised interface
surface.

## Diagnostics, Code Actions, and Hints

Diagnostics are compiler results with stable codes, exact spans, related information, and optional
fix plans. The server clears diagnostics absent from the next complete publication and includes the
accepted document version.

Code actions expose compiler-planned edits, including imports, required interface members, and
optional/fallible callable contracts. Generated edits must parse and typecheck as ordinary Nocter;
the protocol layer does not synthesize source templates independently.

Inlay hints project retained inferred binding types and source-visible result provenance. Explicit
source annotations suppress redundant hints. The language server performs no second inference pass
and does not expose compiler-owned allocation dataflow as source syntax.

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
single-file and package examples live under the repository-root
[examples directory](../examples/README.md).

## Non-goals

The tooling contract does not require VS Code, an editor-local module graph, editor-local type or
borrow checking, TextMate scopes as language semantics, a separate AI-only grammar, background
concurrent analysis, or persistent cross-session semantic caches.
