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
The `never` semantic type is admitted only in callable result slots and as the type of terminating
checked nodes. It cannot appear in a data layout or concrete substitution. Constraint collection
treats a `never` expression as non-producing and never binds a generic parameter to `never`.
The `void` semantic type is admitted only for normal-completion nodes, callable result slots, the
payloadless success branch of `void!`, and opaque `*void`. It never receives a zero-sized value
layout or enters a concrete data substitution.
Checked contextual injection represents `void` consumption as a sequencing edge, not a value
operand. Construction of `void!` writes its success tag after that edge; a terminal edge cannot
produce or initialize the outcome.
Declaration validation rejects zero-variant enums before semantic type interning creates usable
enum identities. Later exhaustiveness and layout stages may therefore assume every enum has at
least one valid tag.

## Checked Program

Every checked body owns one typed node arena. Authored and compiler-generated operations use the
same node model and carry explicit body ownership. Comparison, indexing, conversion, iteration,
interpolation, construction, calls, ownership transitions, and failure handling are not retained as
unrelated side maps selected by source containment.

A checked body records reachability on its control-flow nodes. Source after a proven terminal node
still receives declaration and type identities, but no synthetic initialization, move, loan, or
provenance state is created for the impossible continuation. Such nodes remain available to
diagnostics and editor projections and are excluded from executable reachability and MIR.

Contextual expression checking lowers optional and fallible construction into explicit checked
nodes at every authoritative expected-type boundary. Each node names its expected outcome type,
selected tag, and recursively checked payload. An expression already having the complete expected
outcome type requires no injection node. MIR must consume these decisions; it cannot reopen a
rendered type spelling or reconstruct outcome order.
Checked propagation nodes identify the exact declared outcome layer they target and reuse the same
injection path as an explicit `return`; they do not encode only an unqualified "failure" or
"absence" action.

Generic inference may project a statically known expected outcome shape to collect payload
constraints. It completes the unique substitution before checked injection nodes are built.
Contextual tag literals contribute no payload constraint, and injection never feeds a guessed type
back into callable selection.

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

One interned machine-layout store is the authority for field offsets, outcome payload offsets,
aggregate size and alignment, argument and return classification, active-payload validation, and
drop-glue addressing. Optional and fallible layers receive distinct semantic type identities but
use the same recursive binary tagged-union layout operation. Code generation cannot calculate a
second layout from rendered types or special-case a different tag width for calls.
Stored scalar size and alignment are target facts in that layout store. ABI argument extension is
a transport projection of an existing layout and cannot mutate or replace the stored type layout.
The target description supplies endianness and integer width, while signed semantic types require
two's-complement interpretation. Machine lowering cannot select an alternative signed
representation or defer that choice to individual primitives.
Syntax retains unary negation and integer literal nodes separately. Contextual checking may
normalize that exact grouped shape into one signed integer constant, with a source origin covering
both syntax nodes. MIR never receives an out-of-range positive constant for the signed-minimum case.
Checked integer shift nodes distinguish left, unsigned-right, and signed-right operations and
retain the validated source width. Machine lowering emits an explicit count-range trap before a
target shift and cannot inherit the target instruction's count masking behavior.
Checked division and remainder nodes retain signedness and width. Machine lowering emits zero and
signed-minimum/`-1` guards before either operation and cannot inherit a target's overflow result or
remainder convention.
Checked assignment and compound assignment each own one target-place plan and one right-hand-side
expression; compound assignment additionally owns the selected numeric operation. MIR emits RHS
evaluation before the place plan and emits that plan only once. No stage expands compound
assignment into source-shaped duplicate target expressions.

Zero-sized types retain logical initialization, ownership, element counts, and drop operations in
MIR. Machine lowering erases only their storage and transport; it cannot erase evaluation or
destruction merely because a layout has size zero.
MIR place identities and projections remain authoritative for borrow conflicts. Machine lowering
may coalesce zero-sized storage or synthesize aligned non-null borrow addresses, but neither it nor
an earlier semantic stage may merge places by comparing those addresses.

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
