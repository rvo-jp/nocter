# Specification-First Compiler Architecture

This document defines only implementation boundaries that cannot decide public language behavior.
The [language specification](../../spec/README.md) is the sole semantic authority. If a public
choice is missing there, implementation stops until the specification is amended.

## Program Pipeline

```text
SourceProgram
  -> SyntaxProgram
  -> DeclarationProgram
  -> CheckedProgram
  -> TargetProgram
  -> ExecutableProgram
  -> MirProgram
  -> MachineProgram
  -> executable image
```

Each arrow is a one-way lowering boundary. A later program cannot recover a decision by revisiting
an earlier representation.

## Separate Source Projection

Source files, byte ranges, comments, and editor offsets live in a `SourceIndex` beside the semantic
pipeline. Semantic nodes may retain a source-origin ID for diagnostics, but source locations never
identify types, declarations, callables, generic parameters, bodies, or executable instances.

```text
URI + offset -> SourceIndex -> semantic ID -> presentation
semantic ID -> SourceIndex -> diagnostic range
```

No inverse source lookup participates in type equality, dispatch, ownership, monomorphization,
reachability, ABI selection, or code generation.

## Semantic Identities

Distinct domains represent packages, modules, authored declaration sites, semantic type
definitions, callables, fields, variants, associated types, generic parameters, parameters,
bodies, closures, expressions, statements, types, substitutions, and monomorphized items.

The compile-unit type store interns structural types. Its keys contain typed semantic IDs and
normalized constants, never rendered names, source text, or byte positions. `TypeExpr` belongs to
syntax lowering and presentation; it does not cross into checked semantics.

Type well-formedness is validated on interned semantic types after alias expansion and concrete
generic substitution. Invalid constructed types, including an optional layer whose eventual
payload is `void` or any outcome whose eventual payload is `never`, cannot enter checked bodies,
monomorphized keys, MIR, or ABI layout.

## Checked Program

Every checked body owns one typed node arena. Authored and compiler-generated operations use the
same node model and carry explicit body ownership. Comparison, indexing, conversion, iteration,
interpolation, construction, calls, ownership transitions, and failure handling are not retained as
unrelated side maps selected by source containment.

A checked body records reachability on its control-flow nodes. Source after a proven terminal node
still receives declaration and type identities, but no synthetic initialization, move, loan, or
provenance state is created for the impossible continuation. Such nodes remain available to
diagnostics and editor projections and are excluded from executable reachability and MIR.

Contextual return checking lowers optional and fallible construction into explicit checked nodes.
Each node names its expected outcome type, selected tag, and recursively checked payload. An
expression already having the complete expected outcome type requires no injection node. MIR must
consume these decisions; it cannot reopen a rendered type spelling or reconstruct outcome order.
Checked propagation nodes identify the exact declared outcome layer they target and reuse the same
injection path as an explicit `return`; they do not encode only an unqualified "failure" or
"absence" action.

Type checking selects either a direct callable or an exact abstract requirement. When generic
substitution makes an abstract receiver concrete, instantiation resolves that requirement once
through one conformance table. MIR and later stages have no dispatch API.

## Target Program

`TargetProgram` owns the selected target and exact toolchain capability validation for the complete
checked compile unit. It is the common public acceptance boundary for `check`, `build`, and `run`
under one target and toolchain snapshot. A library-only `check` may stop there without inventing an
executable entry. Frontend-only experiments remain internal tests and never create a second public
language subset.

## Executable Program

Entry-driven instantiation produces the only reachable callable graph. A monomorphized key contains
semantic callable identity, optional concrete receiver type, and substitutions keyed by generic
parameter identity. Duplicate keys with different values are errors.

MIR construction and linkage consume this graph. They cannot build parallel callable indexes.
Runtime symbol spelling is generated after item selection and cannot be used to find a semantic
item. `build` and `run` cannot reject a source-language construct that the corresponding
`TargetProgram` accepted; a later failure is an internal compiler or output-system failure, not a
second language diagnostic.

## MIR and Machine Program

MIR represents control flow, places, initialization, moves, loans, regions, cleanup, calls, and
outcomes for concrete executable items. Calls target monomorphized item IDs. MIR construction does
not receive AST, a resolver, rendered types, or runtime names.

Machine lowering projects validated MIR into ABI storage and target-independent operations.
Target code generation consumes only the machine program, target description, and one-way linkage
table. Optimizations may replace ordinary operations with constants but cannot define source
semantics. In particular, built-in `error` values use ordinary value and call paths.

One target-independent trap operation represents always-on safety failure, including failed
postfix `!`. Machine lowering does not route that operation through standard-library formatting,
stderr, entry-wrapper failure, or cleanup edges.

## Dependency Enforcement

The new workspace will encode the dependency direction in crate boundaries. Architecture tests
will additionally reject:

- source or syntax types in semantic identity
- rendered-name type equality
- source-range semantic lookup after syntax binding
- resolver iteration used for candidate selection
- dispatch outside checking or instantiation
- AST or resolver inputs to MIR and code generation
- runtime-name lookup of semantic targets
- multiple executable-program registries
- compatibility imports from archived compiler code

Crates are introduced only after their required specification gate closes. Empty crate scaffolding
is not evidence that a responsibility has been designed.

## Error-Tolerant Tooling

Editor analysis may retain explicit invalid syntax and error semantic nodes in an immutable
snapshot. It never converts incomplete source into a second successful semantic model. Hover,
completion, navigation, rename, tokens, diagnostics, and code actions consume the same checked IDs
when they exist and use syntax-only recovery only when no semantic fact is available.
