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

When a lowering boundary creates new semantic identities, it consumes the existing `SourceIndex`
into its duplicate-checking builder, adds exact projections, and freezes both deterministic lookup
orders again. It does not create a phase-specific parallel source index. Canonical semantic
programs never depend on this projection value.

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

`nocter-model` is the syntax-independent owner of these IDs and structural type keys. It has no
dependency on `nocter-source` or `nocter-syntax`. Identifier spellings live in one immutable symbol
table built by sorting and deduplicating the complete input set, so symbol IDs do not depend on
source discovery order. Symbol IDs support lookup and presentation metadata only; nominal,
associated, generic, callable, and opaque semantic IDs form type identity.

`nocter-declarations` owns immutable typed arenas addressed by those ID domains. A consumed builder
is the only mutation path. Exact module identity is a package identity paired with a normalized
directory path; `.` and `..` never enter that path. Authored visibility is resolved once to private,
an ancestor-module descendant boundary, the declaring package, or all packages. Later lookup never
reinterprets `pub(../)` from a physical source path. The crate depends only on `nocter-model`, so a
semantic consumer cannot acquire source or syntax access through the declaration graph.

Mutually referential headers use one closed two-pass construction protocol. The builder reserves
each identity in canonical order, resolves generic parameters, member identities, requirements,
and structural types against those reservations, then defines each slot exactly once. Freezing
fails if any reservation remains incomplete or if a reference, reciprocal owner edge, member
position, callable shape, receiver capability, provenance origin, import target, or visibility
boundary is inconsistent. The immutable program therefore never carries a partially patched
declaration graph.

Declaration result provenance names either the exact receiver or exact `ParameterId`. Structural
callable-type provenance remains a separate normalized set of ordinary parameter positions. This
prevents `from self` from being encoded as a forged explicit-parameter position while keeping
parameter spellings out of both identities.

`nocter-source-index` is a sibling projection, not a field that defines declaration identity. It
stores semantic-to-source bindings twice in immutable deterministic orders: once for lookup by
semantic entity and once for lookup by source coordinate. Each origin can select either a complete
syntax node or an exact syntax-token view, so a declaration name never needs the keyword,
visibility, whitespace, or body as its editor range. Contract declarations, separate
implementations, and references remain explicit roles. Structural `TypeId` values are not source
entities because one interned type can occur at many sites; source type uses attach to their owning
declaration or checked expression instead.

`nocter-declaration-lowering` accepts one explicit compile-unit input after package discovery. A
package has an opaque resolved identity distinct from its display name. A module has that package
identity plus normalized directory segments. Physical package declarations, module roots,
implementation files, and single-file inputs carry canonical path keys, but those keys are used
only to reject duplicate ownership and to produce deterministic source projections. Lowering does
not probe directories or reinterpret paths. It sorts packages, modules, and sources by their exact
input identities before allocating semantic IDs, so discovery order cannot change the declaration
program. Declared packages require one package declaration and one root module; single-file mode
requires exactly one root module source and no package declaration. The two layouts cannot be
silently substituted for each other.

Package discovery also supplies one resolved edge for every top-level or block `use` node. The
edge identifies either an exact physical implementation source or an exact module identity;
lowering never derives that distinction from path text or filesystem layout. Validation requires
all implementation sources to be reachable from their module root through private bare relative
source imports. Those edges may cycle and remain idempotent. Module edges must target a module in
the compile unit and form an acyclic graph. The canonical surface retains the normalized edge, so
later import lookup has no path-probing fallback.

Module edges also retain the exact authored `use` node selected by discovery. Acyclic validation
first removes the deterministic acyclic prefix, then derives one canonical complete cycle from the
residual graph. The cycle is rotated by canonical module identity, so compile-unit input ordering
cannot change its primary edge or ordered notes. Missing, duplicate, stale, and unreachable
discovery inputs remain internal boundary failures; only authored source-import shape and module-
cycle rules receive source diagnostic codes.

Before semantic identities are reserved, lowering produces one temporary declaration-surface
inventory. It visits module roots before implementation sources, sorts implementation sources by
canonical physical identity, and records each declaration or member with its exact syntactic
owner. Blocks are opaque to this pass; body syntax cannot create or alter a declaration header.
The pass also enforces the module rule that implementation sources cannot add visibility,
re-exports, fields, or interface members. A following contract pass rejects construction and
coercion entries that do not supply a declared root contract. This inventory is consumed by
declaration reservation and never becomes a second long-lived program model.

Before allocating callable IDs, a contract-joining pass compares canonical header token sequences.
It excludes visibility, bodies, newlines, and the `default` marker that construction
implementations do not repeat, while retaining names, owner patterns, generic requirements,
parameter names and types, results, and authored provenance. One eligible public bodyless root
contract must match exactly one private implementation body. The implementation and any container
used only to carry matched bodies map to the contract representative; missing, mismatched, and
duplicate bodies fail before reservation. This prevents a later name resolver from trying to
merge already-distinct semantic IDs.

One production declaration-lowering facade owns the pass sequence: surface collection, callable-
contract joining, identity reservation, header preparation, generic preparation, authored imports,
compiler-selected prelude composition, type binding, type normalization, and header definition.
The facade performs no semantic work of its own. It prevents tools and later compiler stages from
assembling a partial or differently ordered declaration graph while keeping each pass independently
testable.

Reservation consumes that grouping in canonical surface order. Nominals, aliases, interfaces,
associated types, callables, construction surfaces, instances, conformances, variants, drops,
tests, and opaque result types receive their final typed IDs before any header type is resolved.
Fields, generic parameters, ordinary parameters, requirements, and bodies are added later because
their identities cannot participate in recursive header lookup before their owner exists.

Header preparation reads the exact name-token identity already selected by the surface inventory,
resolves `pub`, `pub(./)`, ancestor scopes, and `pub(/)` once to semantic package/module
boundaries, and allocates declaration sites. It rejects module, member, and test-name collisions in
canonical order. Only after a name is known does source projection bind a named entity to its exact
token; unnamed containers use their syntax node. A joined body is an implementation binding of the
contract entity rather than a second declaration. This prevents editor ranges from expanding to a
keyword, visibility prefix, brace, or surrounding whitespace.

Generic preparation runs only after recursive owners and declaration names exist. Explicit generic
lists allocate unique, non-shadowing parameters in authored order. A declaration type pattern
allocates a parameter at the first spelling and resolves later occurrences of that spelling to the
same identity; declaration and reference source roles preserve that distinction. Members inherit
an immutable, symbol-sorted owner scope and add only their own explicit binders. Joined callable
implementations reuse the representative contract's parameter IDs and must repeat the exact binder
sequence. Structural type lowering therefore receives one complete lexical generic environment and
never infers binding identity from a type occurrence. Each lexical entry also retains the exact
declaration token selected when the binder was allocated. Reserved names, explicit same-list
duplicates, and nested shadowing project distinct `E0280`-`E0282` diagnostics from those stored
subjects; the diagnostic adapter never searches syntax or reconstructs scope ancestry.

Authored module imports are resolved after generic scopes but before type construction. Every
module owns one symbol-sorted namespace whose entries pair an exported semantic entity with its
effective visibility. Direct declarations and imported names occupy that same table, so selected
aliases cannot collide with a declaration or another import. Dependency modules are completed
before importers. Selection checks the target entry from the importing module, and a re-export's
normalized visibility must denote a subset of the target boundary. Same-module source edges add no
semantic import; declarations from their already-composed sources entered the module table during
the direct pass. The compiler-managed prelude remains a separate fallback layer so it cannot turn
two authored collisions into priority rules.

Import preparation retains the exact module-path node for every semantic module import. Prelude
composition consumes that retained origin instead of reading the syntax tree again. An authored
import of the compiler-selected standard prelude is therefore the shared import rule `E0262`, while
a missing selected prelude, an absent retained origin, or rejected program authority is an internal
composition failure.

Declaration freezing stores the authored namespace and compiler-selected fallback as separate
tables in `DeclarationProgram`. Authored entries retain effective visibility; fallback entries are
local-only and cannot become re-exports. Body checking consumes these tables directly instead of
rebuilding lookup from declaration/import iteration. Block imports remain lexical checked-body
data and are not represented as declaration imports.

Header type binding owns a closed distinction between authored rules and broken compiler input.
Authored name, entity-kind, arity, `Self`, fixed-array, callable provenance, opaque-binding, and
requirement failures retain `SyntaxOrigin` values at rule selection and project `E0290`-`E0302`.
Path segments retain their own name token and optional argument node, while duplicate constructs
retain the first and later tokens. Missing syntax nodes, missing discovery-owned sources, stale
symbols, and duplicate source-index insertion remain internal `TypeBindingError` variants. The
production facade is the only layer that converts an authored violation to `SourceDiagnostic`.

The mutable binding boundary is one `BindingArena`: a bound-kind arena, syntax-root index,
declaration-context index, and temporary `NormalizationOrigins` side index. The side index records
only subjects that a later normalization rule can select. It retains alias declaration tokens,
exact associated-selection tokens, and callable type nodes without contaminating `BoundTypeKind`
or canonical `TypeKind`. Normalization projects `E0310`-`E0313` and `E0320`; alias cycles are rotated by
canonical declaration identity and retain every declaration in the cycle. Missing bound state,
alias definitions, normalized `Self`, or associated-index invariants remain internal failures.
General type equalities are validated only after alias expansion. Their temporary requirement
origins are keyed by declaration and predicate position, so the normalizer can reject an equality
without an associated projection without retaining syntax in `RequirementKind`.

Callable type keys erase parameter spellings after resolving authored provenance names to sorted,
unique parameter positions. Result provenance is therefore part of the structural callable
contract without making a rendered name part of type equality. Static and fresh storage retain no
caller-managed place and normalize to an empty external-origin set.

The compile-unit type store interns structural types. Its keys contain typed semantic IDs and
normalized constants, never rendered names, source text, or byte positions. Phase 2 freezes a
`DeclarationProgram` containing the immutable declaration graph and the header-type prefix. Phase
3 consumes that value exactly once into `DeclarationGraph` plus the owned `TypeStore`, then interns
body, closure, inference, and specialization types after the existing IDs. The checked program
freezes the extended store. It never copies a header store, translates a `TypeId`, or creates an
overlay lookup authority. `TypeExpr` belongs to syntax lowering and presentation; it does not
cross into checked semantics.

Header definition consumes the normalized roots and the temporary surface inventory exactly once.
It allocates fields, parameters, receivers, requirements, and bodies in canonical order, then
defines every reserved nominal, alias, interface, associated type, callable, construction,
instance, conformance, drop, test, variant, and opaque-result slot. A joined public contract and
private implementation share callable and parameter identities; their declaration and
implementation projections remain distinct in `SourceIndex`. Authored result-provenance clauses
are stored as declarations, while elided body-owned provenance remains explicitly inferred rather
than being guessed from a header.

Rule selection at this final syntax-consuming boundary retains exact subjects for declaration
facts that do not exist in canonical type identity: construction `default` markers, declaration
provenance origins, the result type of an ambiguous bodyless callable, and conformance associated-
type binding names. These rules project as `E0314`-`E0319`. Duplicate rules store both authored
tokens when the second occurrence is observed. `HeaderDefinitionError` separately carries
malformed normalized state and program-builder failures; the production facade cannot expose those
failures through the source-diagnostic variant.

Compilation setup resolves the selected standard package and each compiler-owned built-in surface
to exact package and module IDs. The immutable declaration program stores those authorities.
Freeze-time attachment validation compares IDs, never a package display name or textual `std`
path. The same validator owns empty-enum, construction-result, unique construction-family, drop,
opaque-result, associated-binding, and owner-site invariants; lowering does not maintain a second
semantic prevalidation table.

Program validation separates authored declaration-rule violations from malformed compiler graph
integrity. A declaration rule owns its stable error code, source-level message, correction
direction, primary declaration-site ID, and optional related declaration-site ID. Only after rule
selection does declaration lowering project those IDs through the completed `SourceIndex` to exact
syntax origins. Changing a diagnostic span therefore cannot change rule selection, and adding a
diagnostic cannot create a second attachment or declaration-shape evaluator.

All source-backed compiler diagnostics share one phase-neutral envelope containing a stable code,
primary source origin, zero or more related source notes, and optional correction guidance. The
envelope does not select a rule and has no dependency on declaration or checked-program models.
Each phase owns its rule selection and exact syntax-to-source projection. Contract joining projects
its exact syntax subjects into the envelope before reservation; module-surface validation does the
same only for authored root-versus-implementation rules. An unmatched private construction,
literal, or coercion entry has its own `E0254` rule rather than being rendered as a mismatch against
a contract that does not exist.
Freeze-time validation projects semantic declaration-site subjects through
the completed source index. Stage-specific wrappers preserve the selected rule identity. Errors
that indicate malformed syntax snapshots, incomplete discovery inputs, or inconsistent compiler
state stay outside this envelope and have no public language code.

Name collision and authored visibility rules form one namespace-rule domain rather than separate
header and import diagnostics. Header preparation stores exact name-token or visibility-node
subjects in a `NamespaceViolation` at rule-selection time, before its temporary surface IDs are
consumed. Diagnostic projection therefore never searches by rendered name or reevaluates a
namespace to recover the first declaration.

Authored import resolution consumes that same namespace-rule domain for reserved local names,
collisions, and visibility boundaries. Every namespace binding records the exact token that
introduced its local name rather than the enclosing declaration node. Import-specific violations
record the selected-name token and, for access or widening failures, the target binding's exact
origin. Missing names, access boundaries, and re-export boundaries are therefore selected once and
projected without repeating import lookup in a diagnostic adapter.

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
The program-wide copyability table derives optional and fallible copyability structurally from
their success or present payload and the built-in copyable `error` type. It memoizes the result by
canonical `TypeId` and closes over the complete semantic type store before checked-program
construction finishes. Flow analysis never reclassifies an outcome from its active tag, and MIR
consumes the same type fact instead of deciding whether a particular tagged value may be copied.
Generic `copy struct` validation builds one normalized conjunction of field-copy requirements. A
concrete specialization substitutes its arguments and evaluates its field types once through the
same copyability table. An unconditional move-only field rejects the declaration, while a generic-
dependent atom remains a specialization condition; body checking and MIR do not traverse fields
again to rediscover copyability.
Checked closure construction records one environment field for each resolved capture and derives
copyability with the same structural operation used by aggregates. Invocation capability is a
separate callable fact and cannot overwrite the environment's ownership class. Later
monomorphization and MIR consume both decisions without reconstructing either from closure syntax.
Each closure-body capture binding resolves directly to one typed environment-field projection with
readonly, readwrite, or owned access. Body checking never discovers captures by free-name scanning,
and later stages never reinterpret a stored borrow as an implicit source dereference. Copies and
moves of a closure retain the same capture and loan identities through ordinary value flow.

## Checked Program

Body name resolution is a one-way checking input, not a second program graph. One iterative action
machine creates dense body-local scope, local, and capture identities while walking each exact
`BodyId` source projection. Initializers are resolved before their binding is inserted; loop,
region, pattern, catch, and closure names are inserted only into the scope defined by the language.
Closure roots cut the ordinary lexical lookup edge, resolve explicit captures to immediately
enclosing callable bindings, and expose new capture identities inside the closure body. A free use
cannot become an implicit capture.

Block imports enter the same lexical insertion path as body bindings but create no storage
identity. Their discovery-owned module identities are converted to `ModuleId` only through exact
physical-source projections, then selected names use the frozen authored export namespace and
normalized visibility rules. Synthetic prelude names remain a separate shadowable fallback.
Successful resolution extends `SourceIndex` with local/capture declarations and exact references
only after every body succeeds; temporary syntax-keyed uses are consumed by typed-node
construction.

Every checked body owns one typed node arena. Authored and compiler-generated operations use the
same node model and carry explicit body ownership. Comparison, indexing, conversion, iteration,
interpolation, construction, calls, ownership transitions, and failure handling are not retained as
unrelated side maps selected by source containment.

Typed-HIR construction freezes a body exactly once before flow-dependent ownership analysis. The
ownership walker may revisit immutable control nodes to compute loop fixed points, but it cannot
allocate a node, place, local, or loop identity. This separation prevents analysis order and the
number of fixed-point iterations from changing semantic identity or source projection.

One checked-place construction path handles field and index projections for reads, borrows, and
assignment. It records implicit borrow dereferences rather than collapsing them into the final
readonly/readwrite authority. This preserves the exact owned field prefix required by
initialization analysis and gives MIR an ordered projection plan. Dynamic index expressions are
stored as checked node identities once; consumers execute those nodes in projection order instead
of reconstructing an index expression from syntax.

Name resolution assigns each syntax block one exact body-scope identity and checked construction
stores that identity on the block node. Ownership analysis therefore computes scope-exit edges
without source containment queries. Its dense cleanup table is keyed by the operation that owns
the schedule. Each nonempty entry distinguishes cleanup immediately before control transfer from
replacement cleanup immediately before assignment storage. Path actions retain only an owned
root, exact field identities, type, and unconditional-or-conditional state; borrowed replacement
actions retain an already evaluated place; discarded owned temporaries name their checked value
node. MIR expands those semantic targets into control-flow cleanup blocks and structural drop glue
without inferring execution order from a control-operation variant.
Explicit source `drop` is a checked control operation over the same owned root path. It attaches an
unconditional action to that statement's outgoing edge and consumes the path state; it does not
call a method, allocate hidden storage, or maintain a second explicit-destruction liveness table.

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

The program-wide instance-operation table owns normalized target patterns, binder refinements,
retained requirements, member identities, and overlap rejection. Body checking queries it for
source-defined indexing and the permitted one-step receiver coercion. A unique direct operation
outranks coercion-derived candidates; peers are ambiguous. The resulting checked place freezes the
selected dispatch and canonical declaration-generic arguments together, so instantiation and MIR
cannot rescan instance declarations or derive priority from source order. Conditional index and
coercion requirements recursively call that same selector under an active-predicate guard; there
is no weaker requirement-only operation registry.

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
Checked comparisons retain source operand order separately from strict-order reversal and result
negation. Each operand records whether lowering borrows a place or temporary, uses a readonly
borrow, weakens a readwrite borrow, and invokes one selected coercion. The plan also retains either
primitive implementation or exact static dispatch. Machine lowering always evaluates source nodes
left-to-right before preparing and arranging the semantic operands; it performs no operator or
coercion lookup. Short-circuit logic is a control operation whose ownership state joins the
executed RHS edge with the bypass edge; it never enters an eager binary-operation lowering.
Checked division and remainder nodes retain signedness and width. Machine lowering emits zero and
signed-minimum/`-1` guards before either operation and cannot inherit a target's overflow result or
remainder convention.
Checked assignment and compound assignment each own one target-place plan and one right-hand-side
expression; compound assignment additionally owns the selected numeric operation. Simple named-
place assignment already records old-value destruction as a `BeforeStore` cleanup schedule and
uses the same ownership-state transition for reassignment, reinitialization, and partial-field
repair. Integer compound assignment uses the same closed arithmetic selector as ordinary integer
expressions, but remains one control operation rather than a desugared expression tree. Ownership
analysis visits its RHS first and then requires the target to be initialized. MIR emits RHS
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
