# Semantic Presentation Design

Compiler-owned semantic presentation is the sole authority for editor text that describes a
resolved program entity. Protocol adapters may wrap that text in LSP or another transport, but
they do not inspect declarations, reconstruct types, qualify names, or slice an authored
declaration from source.

## Query Boundary

One positioned query consumes a successful immutable `AnalysisSnapshot`, a `SourceId`, and a
normalized byte offset. Selection follows exactly one direction:

```text
SourceId + byte offset
  -> SourceIndex binding
  -> semantic entity
  -> SemanticSelection
```

Hover then renders checked declaration/type data. Definition and references retain the selected
semantic identity and project its existing `SourceIndex` bindings; they never search source text.

Failed generations expose no semantic answer. A query never consults an older successful snapshot.
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

All semantic requests select one current successful snapshot through the shared semantic-document
boundary. An unopened dependency source may therefore use the same package generation as its open
root, while a failed current generation returns no stale semantics.

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

Completion must reuse the same source selection and presentation authority. It may not add an
editor-owned semantic model.
