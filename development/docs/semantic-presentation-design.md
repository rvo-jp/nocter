# Semantic Presentation Design

Compiler-owned semantic presentation is the sole authority for editor text that describes a
resolved program entity. Protocol adapters may wrap that text in LSP or another transport, but
they do not inspect declarations, reconstruct types, qualify names, or slice an authored
declaration from source.

## Query Boundary

One query consumes a successful immutable `AnalysisSnapshot`, a `SourceId`, and a normalized byte
offset. It follows exactly one direction:

```text
SourceId + byte offset
  -> SourceIndex binding
  -> semantic entity
  -> checked declaration/type data
  -> SemanticPresentation
```

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

Structural callable types erase parameter names for equality. When an explicit structural
provenance needs names, presentation creates stable positional names such as `p0` and uses those
same names in `from`. This changes no type identity.

Callable declarations retain a separate `ProvenanceAnnotation`. `Elided` means the compiler may
infer a storage origin but presentation must omit it. `Explicit` preserves whether the source named
`static`; semantic provenance continues to store only caller-managed origins. The annotation never
participates in checking, dispatch, ownership, or code generation.

## Protocol Surface

The server advertises hover only after this complete path is active. Hover returns Markdown Nocter
code and the exact semantic binding range. Keyword, visibility, brace, whitespace, and synthetic
whole-file ranges do not become hover targets merely because a broader compiler projection overlaps
the cursor.

Semantic tokens, definition, references, rename, completion, and signature help must reuse the
same source selection and presentation authority. None may add an editor-owned semantic model.
