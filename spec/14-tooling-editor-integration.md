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
so definition, implementation, references, hover, completion, and semantic tokens use the same
declaration and type identities there. Flow-sensitive ownership facts are absent rather than
fabricated for that unreachable continuation. Tooling may later expose an unreachable-code lint,
but such a lint is not a language diagnostic.

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

File documentation has one public source owner. Documentation in a package root `index.nct`
describes both that package and its root module because the same physical source declares both.
Documentation in a child module's `index.nct`, or in the source of single-file mode, belongs to
that module. File documentation in an ordinary module source remains available on that
source's syntax snapshot and AST output, but it is not appended to or allowed to replace the public
module documentation.

Hover on an imported module path presents the resolved module documentation when available.
Tooling must not infer documentation from nearby raw text after the compiler reports no attachment.

## Language Server Snapshot

One accepted document version belongs to one immutable, generation-numbered package snapshot.
Diagnostics, hover, completion, signature help, definition, implementation, references, rename,
code actions, inlay hints, and semantic tokens for that request observe the same generation.

An open document overrides disk content throughout its generation. Package graphs for open
`index.nct` overlays are locked, offline, and read-only: analysis never fetches dependencies,
generates locks, or rewrites source. Changed imports invalidate reverse importers; unrelated modules
and nested packages retain independent state.

A failed source or package load retains every reached source and unresolved candidate needed for
future invalidation. A failed graph is not replaced by a stale successful graph.

A document under the exact standard-library root selected by the running toolchain belongs to one
toolchain-standard snapshot, even when that root is outside the initialized workspace folders. The
language server uses the already selected standard-package identity and catalogs every authored
standard module for that snapshot. It must not reinterpret the same root as a path package or
register a second package identity for it. Contract and implementation sources therefore share one
overlay-aware standard snapshot when opened directly or reached through navigation.

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
recover it from source text. Directly seen declarations and source-local import aliases affect
only that source's presentation; they do not become module exports.

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

Declaration owners are shown when they disambiguate a member. Type hover presents the nominal type
declaration and its documentation; it does not append the type's construction functions or typed
literals. A struct body is shown only when the complete representation and every field are visible
at the requesting source. Otherwise the presentation is the declaration head. An enum body is
shown only when the representation and every variant are visible at the requesting source;
otherwise it also remains header-only. This nominal presentation is independent of the presence
or contents of a `construct` declaration.

Construction-function, typed-literal, field, and variant hover presents the selected declaration
itself. `Type.` completion exposes accessible named construction functions and enum variants;
typed-literal syntax, signature help, go-to-definition, and the public `index.nct` contract expose
the remaining construction API. The compiler does not reconstruct a documentation-only
construction surface for type hover.

Associated type hover uses the interface-owned declaration identity. A declaration or projection
is shown as `associated type Interface.Name`; a conformance binding is shown as
`type Interface.Name = ConcreteType`. Normalized callable presentation preserves a generic
`Self.Name` or `T.Name` projection until concrete specialization selects a binding.

## Completion

Completion follows lexical scope, visibility, shadowing, receiver capability, generic bounds, and
the exact package graph. It includes accessible declarations, imports, members, enum-pattern
variants, unused struct fields, construction entries, native-test syntax, and relevant keywords.
`copy` is offered only in a generic requirement context where it is not already present.

The v0.14.0 compiler-owned keyword set is deliberately contextual and closed:

- `test` is offered at a top-level item position, with `test name { ... }` as declaration-shape
  detail. Semantic completion inserts only the keyword; optional editor snippets may provide
  placeholders.
- `copy` is offered at the start of a `where` predicate when a generic parameter is visible and the
  same clause has no existing `copy` predicate. A partially typed prefix such as `co` is accepted.

Ordinary unconditional keyword lists and snippets are lexical editor conveniences, not semantic
completion results. This avoids presenting `break`, `continue`, visibility, or declaration forms in
grammar contexts where they cannot be used.
Construction completion uses the use-site construction view, so private construction remains
available in its authored source and direct seers without leaking to unrelated sources in the same
module.
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

Definition, implementation, and references use semantic declaration identity rather than spelling.
For a callable split between a public `index.nct` contract and a private body, definition selects
the contract name and implementation selects the body name. For an inline body with no separate
implementation projection, implementation selects the declaration itself. Package-wide
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
Code-action selections and diagnostic spans are half-open ranges: adjacent non-empty ranges do not
match. An empty selection acts as a cursor query and matches a diagnostic containing that position.
All currently generated actions have the `quickfix` kind. A client `context.only` filter that does
not include `quickfix` receives no actions and does not trigger speculative compilation.

A required-interface-method action implements every missing required method in the selected
`conform` declaration as one atomic edit. For a separated directory-module conformance, the
diagnostic remains on the public conformance fact in `index.nct`, while the edit targets its joined
private implementation conformance. An inline conformance is edited in place. The source index's
declaration and implementation roles select this destination; the server does not infer it from a
file name or `see` path. Each generated signature uses the conformance-specialized
associated types, callable generics, parameter and result types, and `where` predicates. Generated
method bodies call `std/process.abort()`; the action adds that import when it is not already visible.
The server offers no action unless the complete edited package passes ordinary compilation, so a
partial method set, unresolved signature type, or conflicting `abort` binding is not published.

When postfix `?` has a typed optional or fallible operand but the enclosing authored callable result
cannot propagate that layer, a callable-contract action may replace the callable's exact result type
with the compiler-selected canonical outcome type. The action changes only an editable authored
result annotation. It does not rewrite fixed-result comparison operators, grammar-restricted index
operators, postfix `!`, or local `catch`/`otherwise` recovery. The checker, not the diagnostic text or
protocol adapter, selects the operand layer and proposed result. The server publishes the action only
when the complete edited package passes ordinary compilation.
Operand checking keeps postfix-propagation payload context distinct from an ordinary complete-result
expectation. A callable already returning `T!` can therefore receive an optional-layer repair to
`T?!` for an operand of type `T?`; the existing fallible layer is not allowed to force the operand
to `T!` before its immediate layer is known.
When the enclosing result already has both layers, a generic call's statically declared immediate
layer selects the matching propagation payload; source order or a preferred wrapper does not.

Inlay type hints appear after inferred local binding names and use the checked type rendered in the
binding's module context. An explicit binding type suppresses the hint. Result-provenance hints
appear after a callable result type only for compiler-inferred external origins expressible as
`from self` or `from parameter`; an explicit `from` clause suppresses the hint. Fresh/current
allocation, temporary storage, and other compiler-owned provenance never become inlay source
syntax. The language server performs no second type or provenance inference pass.
Result positions follow the callable declaration's structural grammar. A nested closure result
never receives the surrounding callable's provenance hint, while coercion, index, and expansion
results participate when their inferred provenance has a source-visible external origin.
Inlay-hint requests use half-open ranges. A hint at the request's end position is excluded.

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
