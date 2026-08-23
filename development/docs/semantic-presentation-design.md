# Semantic Presentation Design

Compiler-owned semantic presentation is the sole authority for editor text that describes a
resolved program entity. Protocol adapters may wrap that text in LSP or another transport, but
they do not inspect declarations, reconstruct types, qualify names, or slice an authored
declaration from source.

## Query Boundary

One identity-presentation query consumes a successful immutable `AnalysisSnapshot`, a `SourceId`,
and a normalized byte offset. Selection follows exactly one direction:

```text
SourceId + byte offset
  -> SourceIndex binding
  -> semantic entity
  -> SemanticSelection
```

Hover then renders checked declaration/type data. Definition and references retain the selected
semantic identity and project its existing `SourceIndex` bindings; they never search source text.

Failed generations expose no checked identity answer. A query never consults an older successful
snapshot. Completion has a narrower recovery contract described below: it may consume only
explicit semantic stages retained by the current failed generation.
One shared source-context resolver selects the unique declaration or implementation module that
owns a physical source. Hover, completion, and signature help consume that identity; module-path
references cannot become source owners. A missing or conflicting owner is an internal query error,
not an ordinary empty editor result.
The source index selects the smallest displayable binding under the cursor, with references before
declarations and implementation sites when ranges tie. Synthetic package, target, and whole-file
module projections are not interactive. An authored module-path reference remains interactive and
keeps its complete contiguous path range.

`SourceMap` owns the exact source-name lookup and every UTF-8/UTF-16 conversion. The LSP adapter
resolves a URI to its stable canonical path, finds that path in the selected snapshot, converts the
request position, and wraps the returned presentation and range. Invalid coordinates are invalid
request parameters; an unanalysed document or failed current generation returns `null`.

## Canonical Rendering

Presentation reads the immutable `DeclarationGraph`, `TypeStore`, and checked body facts. It never
copies raw declaration text. Authored whitespace and line breaks therefore cannot leak into hover
text, module and package implementation identities cannot appear as internal debug paths, and
member names have one owner-qualified form.

The renderer owns:

- declaration visibility and kind;
- owner-qualified fields, variants, construction members, methods, coercions, and operators;
- complete generic parameter, parameter, result, and `where` contracts;
- canonical structural type spelling and required grouping;
- explicit result-provenance clauses only.

For a module-relative query, the renderer finds the shortest visible type or module spelling by
walking the immutable authored and prelude namespace layers. Import aliases therefore remain
presentation facts without source slicing, while filesystem and canonical package paths remain
absent. Nominal declaration heads retain their authored declaration names. A construction-pattern
position uses a direct one-segment alias when available and otherwise retains the declaration name,
because a qualified path is not valid `DeclarationTypePattern` syntax.

Nominal hover additionally consumes `ConstructionSurfaceTable::public_surface` with the module that
owns the hovered source occurrence. The table retains one canonical ordered surface and derives a
separate `accessible_surface` for use-site tools. The latter follows ordinary language visibility,
including direct-source private access, while the public presentation view removes raw private
construction. A contract-sealed empty representation remains absent from both external use-site
and public-presentation views; presentation never infers exposure from an empty field list. Both
views preserve the same structural, variant, member, source-order, and default identities.
Presentation does not scan nominal or construct declarations to decide membership. It renders the
selected structural or variant subset in the nominal declaration and selected authored members in
a bodyless, unqualified `construct Type { ... }` block. Exact construction-target occurrences
become `Self` recursively in member results.

Semantic presentation distinguishes absence from failure. No binding at the requested coordinate
returns no result. A source-context conflict, construction-table disagreement, or invalid checked
entity returns a typed internal query error that the LSP boundary reports as an internal error rather
than silently converting it to `null`.

Declaration lowering also owns the exact interactive anchor for every declaration identity. Named
declarations use their name token; coercions use `as`; equality and ordering use `==` and `<`;
indexing uses `[`; expansion uses `...`; literal declarations use their compact literal shape; and
opaque results use `some`. Unnamed callables are never projected over their declaration body.

Structural callable types erase parameter names for equality. When an explicit structural
provenance needs names, presentation creates stable positional names such as `p0` and uses those
same names in `from`. This changes no type identity.

Callable declarations retain a separate `ProvenanceAnnotation`. `Elided` means the compiler may
infer a storage origin but presentation must omit it. `Explicit` preserves whether the source named
`static`; semantic provenance continues to store only caller-managed origins. The annotation never
participates in checking, dispatch, ownership, or code generation.

## Documentation Ownership

The syntax tree is the sole authority for recognizing, grouping, and normalizing documentation
comments. It attaches normalized Markdown to the exact file or declaration node before semantic
lowering. Later stages never rescan comment tokens or copy source slices.

Declaration lowering projects that attachment onto the same `SemanticEntity` used by hover,
definition, references, and rename. Canonical declaration documentation belongs to the identity;
documentation on a joined implementation body belongs to the exact identity-and-origin pair. This
allows an implementation occurrence to explain itself without replacing the public contract's
documentation or leaking to another semantic identity projected at the same token. The immutable
`SourceIndex` stores both indexes and preserves them across the sole staged-extension boundary.

Package file documentation comes from `nocter.nct`. Module file documentation comes only from a
root `index.nct` or a single-file program. File documentation in an implementation source remains
available in its syntax tree but cannot silently become public module documentation. Hover selects
occurrence documentation first and canonical identity documentation second, then appends that
Markdown to the canonical Nocter code fence. The protocol layer does not interpret either part.

## Protocol Surface

The server advertises hover and full-document semantic tokens only after their complete paths are
active. Hover returns Markdown Nocter code and the exact semantic binding range. Semantic tokens
classify the same exact source-index bindings through protocol-independent compiler categories;
the protocol adapter only converts source coordinates, maps the fixed legend, and delta-encodes
the result. Source bindings retain occurrence-specific assignment capability where checking proves
it, so readonly and writable field paths do not have to be reconstructed from syntax. Immutable
parameters and bindings, readonly receivers, readonly field paths, and contextual `some` receive
their exact modifiers or category. Keyword, visibility, brace, whitespace, and synthetic
whole-file ranges cannot become interactive or colored merely because a broader compiler
projection overlaps them.

Identity-oriented semantic requests select one current successful snapshot through the shared
semantic-document boundary. An unopened dependency source may therefore use the same package
generation as its open root, while a failed current generation returns no stale checked identity.

Definition prefers an authored declaration binding and falls back to an implementation binding
only when no separate declaration exists. A module path navigates as one contiguous namespace to
the start of its root source. References return only exact bindings of the selected identity from
reached package sources; `includeDeclaration` controls declaration and implementation sites.
Local names, shadowed names, imports, interface dispatch, and associated projections therefore
cannot be conflated by spelling.

Rename consumes that same identity instead of searching text. The compiler rejects non-name
anchors and any plan containing an occurrence outside a selected root package. The server derives
a speculative overlay from every reached source in the immutable generation, applies all edits,
and runs normal package resolution, discovery, lowering, name resolution, and checking without
publishing the candidate. The transaction is returned only when every edited occurrence resolves
back to the same semantic identity. This rejects declaration collisions and subtler shadowing or
capture changes rather than treating a parseable replacement as sufficient. The resulting atomic
workspace edit carries the accepted version for open documents and a null version for closed
documents. Explicit closure captures form a compiler-owned rename family with their source
binding, so a rename crosses the capture boundary without conflating unrelated equal spellings.

Signature help selects the innermost checked call node rather than inferring a callee from nearby
tokens. Static dispatch renders its selected generic arguments; callable requirements and concrete
closures render their checked structural signatures. The compiler records parameter-label byte
ranges while it renders the normalized signature, and the protocol adapter converts only those
offsets to UTF-16. Authored argument-node ranges determine the active parameter.

Name completion retains each checked lexical scope's resolved bindings and gives the scope itself
an exact block projection in `SourceIndex`. The compiler selects the innermost containing scope,
walks only its parent chain, excludes sequential locals declared after the cursor, and overlays
those names on the module's authored and prelude-fallback namespace. Explicit closure roots have
no parent edge, so only declared captures cross the closure boundary. Completion details reuse the
canonical presentation renderer; the protocol layer assigns only LSP item categories.

The production session also has an analysis result that retains the completed, syntax-independent
pre-body semantic stage when typed-body checking rejects the current generation. This is not a
partial checked program: it contains declarations, normalized program-wide authorities, resolved
body names, scopes, and their source index, but no checked nodes, local types, dispatch, ownership,
or provenance. Name completion may consume that exact failed-generation stage. Ordinary command
compilation uses the non-retaining path and does not clone the type or copyability stores.

Receiver-member completion does not scan declarations or infer a type from source spelling. The
instance-operation and conformance authorities retain canonical method-name indexes. For each
indexed name the ordinary selector proves receiver-pattern applicability, lexical or concrete
conformance, `where` requirements, visibility, readonly/readwrite/owned capability, and the same
one-step coercion fallback used by call checking. Visible fields come from the exact nominal shape
and pass through the ordinary field selector. A completed call supplies its exact
`CheckedReceiver`. If body checking rejects an unknown, ambiguous, or missing member, it may retain
a typed interruption containing the failed operation's body identity, source origin, receiver
type, available borrow capability, and consumability together with the monotonic type state reached
at that point. The interruption is not a partial `CheckedBody`, does not manufacture a dispatch,
and is usable only at its exact source range in that failed generation.

The production declaration-lowering and compile-input entries still reject every syntax error.
One separate editor-only declaration entry permits an incomplete syntax tree only when it has no
lexer diagnostics and every parser diagnostic is contained by an executable block. Header,
declaration, import, and package syntax cannot cross this boundary. Original syntax diagnostics
remain the snapshot's public failure; ordinary body checking stops on the explicit missing/error
node and may retain only facts fixed before it. This supports `value.` completion without inserting
a synthetic identifier or compiling modified source.

When lexical name resolution rejects authored source, its editor-only entry may freeze one
`NameAnalysisRecovery`. The value contains the declaration graph and type store together with only
the body scopes, bindings, and source projections completed before the rule failed. The unresolved
spelling receives no synthetic target, later statements are absent, and the value cannot enter body
checking. Name completion can therefore use the exact current generation without editor-side token
lookup or a stale successful snapshot. This recovery stage is distinct from the complete pre-body
stage retained after a typed-body failure.

Construction-member completion is a separate type-owned query rather than a special case of value
receiver completion. One `ConstructionCompletionOwner` identifies a nominal or built-in type
family independently of inferred or explicit generic arguments. The checker-owned selector derives
named candidates from the construction surface's use-site view: enum variants and construction
functions preserve declaration order, while structural and literal entries are omitted because dot
syntax cannot name them. An inaccessible entry never reaches the candidate list; an unconstructed
built-in family yields an ordinary empty list.

A checked member reference resolves back to its semantic variant or construction-callable identity
before querying that surface. Invalid and missing member names retain a typed
`ConstructionSelection` interruption containing the already-resolved owner family. The incomplete
generic form `Type<Args>.` resolves only when the parser owns an exact missing final name; it cannot
repair incomplete arguments or synthesize a member. Complete, body-failed, and syntax-incomplete
generations therefore converge on the same selection authority. Selection-table disagreement is a
typed completion error and becomes an internal LSP error rather than silently falling back to name
completion.

Structural-field completion is another construction-owned query, not lexical name completion.
For a complete body, the checked aggregate supplies its nominal identity and initialized field
identities. When incomplete or invalid construction stops body checking, a
`StructuralConstruction` interruption retains only the nominal identity and field identities
resolved before that failure. Both paths subtract those identities from the construction table's
use-site structural entry, preserving field declaration order and the ordinary visibility
boundary. The CST selects the containing initializer range but never supplies a type or field
spelling. A structurally inaccessible type therefore produces no field candidates, and an empty
result inside a complete initializer does not fall back to unrelated lexical names.

Enum-pattern completion also has a distinct compiler query. Once the pattern subject is typed, the
body checker retains its nominal enum identity before resolving the authored qualifier and variant.
Complete patterns recover the same owner through the selected variant identity. Both paths ask the
construction surface for accessible variants only, so construction functions and other entries
cannot leak into a pattern candidate list. The pattern CST selects the cursor context but does not
infer the subject type or accept a merely matching qualifier spelling.

Associated-type completion is based on the checked body's normalized predicate environment. At a
`T.Name` or `Self.Name` type position, the checker collects associated declaration identities from
the exact interface capabilities proven for that base. A successful body stores one immutable
source context with those identities in `CheckedProgram`; an invalid or missing final name retains
the same identities in an `AssociatedTypeProjection` interruption. Complete, body-failed, and
syntax-incomplete generations therefore share one candidate set. The LSP layer does not rescan
`where` clauses, infer an interface from the base spelling, or expose associated types from an
unproven interface.

Automatic-import completion combines two immutable authorities instead of scanning the workspace.
The declaration graph supplies reached module namespaces, effective visibility, semantic targets,
and canonical details. The discovery snapshot retains the active package's exact dependency
alias-to-package edges. Only another reached module in the active package, a direct dependency, or
the selected standard package can therefore produce a candidate; a public re-export through one of
those modules remains valid, while a transitive package cannot be addressed directly. Candidates
are suppressed when their local spelling is already visible. Discovery also projects every
top-level and block module-import edge; a candidate is excluded when adding its edge would close a
module cycle. The frozen declaration graph retains canonical package-identity and normalized
module-path indexes, so projecting those discovery edges is proportional to their path lengths and
does not rescan every module for every completion request.

Each automatic-import candidate owns a protocol-independent byte-range edit. One syntax-based
insertion planner extends the last top-level import group or inserts before the first declaration
and its attached item documentation. File documentation and existing group whitespace remain
unchanged. The language-server boundary only converts that edit to UTF-16
`additionalTextEdits`; it does not rediscover exports, paths, visibility, or insertion positions.

Inlay hints are another compiler-owned semantic projection. Checked local identities supply their
final types, and the ordinary module-relative type renderer supplies labels; the syntax tree is
consulted only to suppress a binding that already has a `TypeAnnotation`. Callable hints consume
the checked `CallableProvenanceTable` and retain only external receiver or parameter origins that
can be written in a source `from` clause. Explicit provenance suppresses the corresponding hint,
while ambient allocation and temporary sources are never rendered. The analysis result owns byte
positions and protocol-independent hint kinds. The language-server boundary validates the requested
UTF-16 range and converts positions, while `nocter-lsp` only decodes and renders protocol values.
