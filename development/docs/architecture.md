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

`SourceProgram` owns normalized UTF-8, source identities, lexical tokens, byte spans, newline
tokens, and each token's joint-to-next fact. String starts, text segments, interpolation boundaries,
and string ends are ordinary lexical tokens in that same stream. `SyntaxProgram` consumes those
facts once; it never re-reads source bytes to distinguish indexing from typed literals or rescans
string contents to discover interpolation.

`SyntaxProgram` stores nodes and child elements in flat immutable arenas addressed by tree-local
`NodeId` values. Deep valid prefix syntax therefore has no recursive ownership chain to overflow
the process stack during traversal or destruction. A parser event stream builds the arenas once;
missing syntax and subdivided token views are ordinary child elements. Every syntax token retains
its lexical-token identity, and the syntax pieces for one subdivided token must exactly partition
that token's normalized range.

Left-associative expression nodes use forward-parent links in that event stream. The builder
resolves each link once while opening arena frames; the parser does not recursively reparent an
already built subtree or allocate wrapper nodes for precedence levels that have no authored
operator. Deep unary and binary expressions therefore share the same bounded-memory arena path as
deep type prefixes.

Bounded syntactic ambiguity is parsed transactionally. A successful branch keeps the events it
already produced; a failed branch restores its cursor, token subdivision, nesting, events, and
ordinary diagnostics. The parser never performs a successful lookahead and then parses the same
branch again. Safety-limit diagnostics survive rollback. This rule keeps nested type-argument
recognition linear while leaving an unmatched `<` available to the enclosing expression grammar.
Token discriminators commit assignment, closure, and construction-owner branches independently
from the validity of their interiors. Once committed, malformed interiors retain the selected node
identity and focused diagnostics instead of being reinterpreted as another expression family.

One line-sequence parser owns newline-separated source and member containers. Leaf declarations
never consume their enclosing separator. A missing separator recovers to that container's next
newline or closing delimiter instead of letting one member reinterpret the following member's
tokens. Comma-delimited and line-delimited declarations therefore cannot silently accept each
other's separators.

One continuation-newline component owns leading-token and incomplete-expression consumption.
Statement-level syntax accepts exactly one continuation newline and never crosses a blank line;
delimiter-owned expressions consume the newlines admitted by their delimiter. At the active header
delimiter depth, the parser's control-header mode reserves the first `{` for the control body. A
struct literal, closure, recovery clause, or nested control expression at that level must therefore
be grouped; the parser never speculates toward a later brace or consults name resolution. Every
block classifies its final expression as a body result by source position before semantic checking.
Error recovery may retain missing or unexpected syntax nodes, but cannot revise any of these
choices after resolution or typing.

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
Each nominal type definition owns at most one resolved top-level drop body. Declaration validation
admits it only for an ordinary struct or payload-bearing enum declared in the same module and
rejects it independently of `instance` and conformance lookup. Copyability is derived from the
type declaration before that association and is never changed by cleanup availability. Checked
ownership and MIR consume the resolved drop-body identity; neither performs method lookup to find
cleanup.
The checked place model rejects a field move when any proper-prefix aggregate would become partial
while owning a drop body. Cleanup plans may therefore invoke a type-owned body only with complete
`Self`, then traverse remaining structural children. They never carry a partial-self drop ABI or a
runtime flag that conditionally suppresses user cleanup.
Every checked move place also records its owned root and projections. A borrow projection can
produce a writable place but never an owned move place. Returns, arguments, assignments, captures,
patterns, iteration, and spread consume this same classification rather than maintaining
context-specific move-source rules.
Initialization dataflow is keyed by those semantic places. Joins merge each named-field state to
initialized, uninitialized, or maybe initialized. Assignment consumes that one state to select no
drop, unconditional drop, or conditional drop before storing and marking the place initialized;
scope cleanup and whole-parent replacement consume the same state instead of recomputing liveness.
The type store derives optional and fallible copyability structurally from their success or present
payload and the built-in copyable `error` type. The result is attached to the complete semantic
type before body checking. Flow analysis never reclassifies an outcome from its active tag, and
MIR consumes the same type fact instead of deciding whether a particular tagged value may be
copied.
Generic `copy struct` validation builds one normalized conjunction of field-copy requirements.
Concrete substitution evaluates that expression once in the type store. An unconditional
move-only field rejects the declaration, while a generic-dependent atom remains a specialization
condition; body checking and MIR do not traverse fields again to rediscover copyability.
Checked closure construction records one environment field for each resolved capture and derives
copyability with the same structural operation used by aggregates. Invocation capability is a
separate callable fact and cannot overwrite the environment's ownership class. Later
monomorphization and MIR consume both decisions without reconstructing either from closure syntax.
Each closure-body capture binding resolves directly to one typed environment-field projection with
readonly, readwrite, or owned access. Body checking never discovers captures by free-name scanning,
and later stages never reinterpret a stored borrow as an implicit source dereference. Copies and
moves of a closure retain the same capture and loan identities through ordinary value flow.

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

Interpolation lowering owns its partial `String` as an ordinary MIR temporary. Recoverable exits
use normal cleanup edges, while the shared safety-trap operation has no cleanup edge. Interpolation
cannot install a special failure rule for bare calls or forced unwrap.

Authored string-literal expressions retain distinct semantic and source identities through checked
IR. Machine constant layout may pool or overlap their decoded static byte ranges and rewrites each
slice constant through the linkage table. Pooling never merges semantic nodes or becomes a basis
for type, equality, or editor identity.

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
