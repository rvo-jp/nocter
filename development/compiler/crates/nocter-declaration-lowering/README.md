# nocter-declaration-lowering

## Responsibility

Lower one closed compile input into an immutable declaration program, validate all declaration-level
contracts, and publish accepted or explicitly recoverable declaration evidence.

## Contract

The crate consumes syntax, compile-unit topology, target selection, toolchain/runtime contracts, and
model construction authority. It produces `DeclarationProgram`, `AcceptedDeclarationProgram`, and
one source-neutral `ReusableDeclarations` result containing the accepted authority, primitive,
trusted target-service, and runtime-storage bindings, and `FrontendProjectionRecipe`. The recipe materializes frontend
bindings, diagnostic origins, and source projection together for one current syntax generation. It
does not check callable bodies.

## Internal Responsibilities

- deterministic identity reservation and definition
- a bounded private access-declaration leaf query and exact-current context, sharing the syntax
  owner's declaration-surface/locator projection rather than a second parser or released borrow
  lowering; header/body checking and public-module migration remain separate
- constant/static metadata defined independently of initializer eligibility or value production
- one restricted dependency query for fixed-array lengths and their scalar structural constants,
  keyed by semantic identities rather than evaluation-pass order
- one canonical `DeclarationSurface` topology consumed by both reservation and focused topology tests
- define-once semantic projection recipes and current-generation materialization
- declaration-only authority projection separated from current body imports and spellings
- stable path-plus-block identities mapping body query keys to declaration-owned `BodyId` values
- canonical source-domain and body-import rebinding for reused declarations
- module namespaces, imports, visibility, and exports
- generic and type-position normalization
- simultaneous receiver/parameter reservation followed by value-provenance clause resolution
- one exported projection of authored callable modifiers into declaration and structural-type
  contracts, reused by body checking for body-local callable types and kept separate from callable
  execution scheduling
- construction-time binding of inherited associated names before declaration capability freeze
- declaration surfaces and contract/definition joins
- primitive, trusted target-service, runtime-storage, builtin, standard-role, and package-target
  projection
- declaration recovery and diagnostic classification

`src/pipeline.rs` owns only production stage composition and failure classification. Its
cross-stage integration tests live in `src/pipeline/tests.rs`; focused responsibility tests remain
beside their owning modules.

## Invariants

- Accepted declaration semantics are built once and cannot contain a deferred invalid edge.
- The eager whole-stage convenience entry exists only behind the `test-api` feature or the crate's
  own test build. Production consumers can use reusable query contracts but cannot invoke a second
  declaration scheduler.
- Target directives, primitive roles, target-service roles, and standard roles are selected once
  upstream or here.
- A selected non-builtin primitive type becomes one opaque nominal identity and one runtime-storage
  binding; lowering does not invent or expose a field representation for it.
- `SourceIndex` is output projection, never semantic input.
- A projection recipe contains semantic identities and declaration-surface locators, never
  `SourceId`, `NodeId`, token ranges, documentation text, or body-local syntax.
- A recipe retains the source-neutral module identity, canonical path, and source kind for every
  source ordinal. Current materialization rejects a different domain before resolving any locator.
- `FrontendBindings` and `SourceIndex` are both interpreted from the same recipe. Documentation is
  read from the current syntax tree during materialization, so trivia-only edits cannot retain
  stale hover text.
- Current checking materialization computes the exact body-symbol suffix once and returns it with
  frontend bindings and source projection. Session cannot rescan syntax to construct a parallel
  symbol domain.
- Body-local imports remain current body input and cannot enter the reusable declaration recipe.
  Their current syntax identities are joined only through the exact source-neutral module mapping
  frozen with `ReusableDeclarations`; materialization neither repeats module lookup nor declaration
  lowering.
- Contract and private definition joins use exact identities, not text matching downstream.
- Lowering evaluates only constants eligible for structural type construction. It never evaluates
  immutable statics or publishes a final initializer value. Metadata freeze is driven by reserved
  declaration identities, not by whichever values the restricted evaluator can produce.
- Each fixed-array length and its structural constant dependencies share one memoized query.
  Speculative ordinary-constant planning is isolated and cannot leak source projections or a
  partial semantic product.
- Public contracts and private bodies must retain the same authored guarantee fingerprint before
  either occurrence reaches semantic declaration storage.
- Callable execution is derived only from the syntax-owned `async` modifier. Result normalization,
  aliases, and generic substitution cannot classify or reclassify execution.
- Callable input clauses resolve only after the complete header identity domain exists. Forward
  references and mutually constrained inputs therefore cannot depend on source walk order.
- A derived associated binding resolves through bound prerequisite identities to the original
  declaration; lowering never creates an alias declaration for inheritance. The accepted
  declaration graph then freezes the effective identity so later stages do not repeat this binding.
- Declaration recovery remains one closed product across the computation and session boundary;
  consumers cannot recombine its graph, type authority, source ownership, and source projection.


## Future Private Access Declarations

The access-declaration query accepts only complete `V2CoreUnit` syntax. It collects all headers,
canonicalizes declaration symbols, reserves functions and records by name order with their
syntax-owned input/field counts, and consumes
reservation closure before resolving any header type. That transition validates the complete
declaration namespace independently of missing/unsupported data; a name conflict is projected at
the current declaration with its related declaration even when both signatures have invalid types.
Preparation retains raw data-type and operation recipes beside the fixed names and source catalog.
A requested header loads its define-once raw definition through the declaration owner's namespace,
without examining body semantics.
`headers` extracts syntax-only header evidence for functions and explicitly typed required constant/static initializers.
Initializer expressions use the syntax owner's existing body-surface exclusion and current-body
locator mechanism, so initializer-only edits rebind headers without retaining stale values. `type_syntax::value_type_syntax` projects one
`ValueTypeSyntax` for input/result headers and checking's local annotations. Recursive
`DataTypeSyntax` preserves tuple/group shape, named leaves and array element/length syntax for
both consumers; record fields use that same data projection. A recognized array retains its
ordinary length-expression node under its actual annotation. `definitions` requests that root's
completed usize value before constructing a closed array reference; it never evaluates syntax,
substitutes a provisional count or interprets a literal specially. Local body annotations remain
a separate checking admission boundary.
The shared typed invocation and input-operation projection is the only
decoder for these contexts; neither consumer recovers a mode from names, types or body use.
`definitions` resolves only the selected input/result or record recipe through one value
normalization path and the declaration owner's single data namespace. Callable recipes retain
kind, type syntax and input operations; parameter names and source positions remain solely in the
shared source catalog. Every callback leaf retains its owning annotation
as the diagnostic location for unknown/unsupported data. Forward record references, including tuple components, invalid fields,
containment cycles and Copy eligibility therefore share the declaration authority instead of a
lowering-owned type table. Existing built-in-name and no-authored-shadowing rules apply. Duplicate declarations/parameters retain
the first conflicting declaration; forward references cannot alter collision selection.

The reusable result owns a single immutable value-access declaration program, its exact neutral
header surface, and projection locators. It contains no current syntax IDs, source identity,
body syntax, or source coordinates. Binding an equal surface constructs one current context
with that same program and the current header/body nodes. Changed headers, a wrong parse goal,
invalid syntax, and mismatched locators cannot create a context or trigger fallback lowering.
Independent body-only edits can rebind, but selected supplier guards must also remain equal.
Declaration reuse grants no independent body-validity certificate.
The complete neutral surface guards record/field and const-capability header changes too. Record
recipes retain only neutral name/field-type locators. `source::recipes` owns capture and rebinding
of the callable and record source catalog as one product. Binding materializes current source
metadata from those locators, without rebuilding a raw type recipe or copying semantic record schemas.
`projection` maps declaration-owned failures through current header evidence; it does not select
semantic failures from diagnostic codes or recover meaning from source coordinates.

The source-neutral declaration product remains owned by `nocter-declarations`. The pairing with
syntax remains here; there is no public parts constructor or conversion to the legacy declaration
program. The [cross-stage boundary](../../../architecture/pipeline/value-access-declarations.md)
owns the handoff to typed-only body checking and the still-required flow/safety validation.

`Preparation` owns one exact-current syntax pairing and the declaration owner's incremental
header publication. Its contexts share the source catalog and expose complete callable or record contracts
as requested, without re-reading or checking a body. `finish` yields the complete reusable `Query`;
the complete query entry drives those same steps through the construction dependency query.
Required contracts close transitive record payloads, validating finite containment and Copy before exposing their field TypeIds.
Raw source data names resolve as their selected recipes load; an unknown type in an unrelated
header cannot block an independent required value. Complete closure still validates that header.
`advance_with_values` exposes `HeaderProgress`: ready, required declaration headers or required
expression values. The caller lends completed `FrozenData` facts by code ID; reading them can
record the same dependency edge in the common query. Only an integer in the target usize domain
can become a length. Missing values leave the define-once raw header unloaded; all missing counts
in a header are returned in authored order without publishing a partial shape. `advance` drives
the same path without value facts and reports `Failure::RequiredValues` when needed; there is no
literal-only standalone evaluator or alternate normalization path. Payload/Copy readiness still
belongs to declarations, not a source-side dependency graph. Its define-once readiness query belongs to declarations,
not to a parallel source-side loaded set. Reserved identity enumeration also delegates to that
owner rather than deriving semantic membership from source recipes. Checking can schedule these
steps iteratively through its common query, including unused records at final closure. Exact-current
record header evidence stays with preparation so late declaration rejections project through the
same field/name projection as initial failures. `reject_cycle` delegates semantic classification
to declarations; it does not inspect payloads. `finish` is fallible, preserving unused invalid
contracts rather than turning them into internal failures. Raw record recipes and current contexts
share one `RecordSource`; declaration diagnostics and type-root capture use its same position
projection. Reusable completed queries retain only neutral record source recipes.
`current` shares the already established view without publishing another contract. Cloning a
context shares that exact immutable program and source pairing; it cannot extend readiness.

Body queries retain their origin header view and open declaration-owned type branches over that
prefix. A later context can bind existing evidence only if its header view preserves the earlier
contracts and type prefix from that same construction. Unit construction canonically replays the
body suffixes into one final authority; lowering does not inspect their contents. Structurally
equal declarations rebuilt by another query cannot substitute for the original lineage. Source
projection remains outside semantic type lookup.


## Exact Source Evidence

`access_declarations::source` owns exact source evidence shared by declaration and checking
consumers. A current declaration `Context` supplies `BodySource` capture, pairing the selected
code's source owner, original immutable header view and exact body bytes. No consumer can provide
an owner and unrelated bytes independently. Byte/ancestry evidence contains no current source
identity, node or span and grants neither contract readiness nor semantic body acceptance.

Binding validates exact header ancestry, then compares the selected body and returns its current
source metadata. Relocated source and unrelated body edits remain reusable; changed guarded bytes
or an equal but independently rebuilt declaration authority reject. Closures can share their
containing body's evidence while retaining separate executable code identities. `BodySource::code`
identifies the selected checking root separately from `owner`, which owns the complete guarded
body. Declaration roots borrow current `CodeSource`; body-local expression roots additionally
retain a neutral body locator and resolve it only after the whole owner passes verification.
Such a selected expression has no source parameters, name or inherited result annotation.

`BodySources` collects these already-paired dependencies. Adding conflicting bytes or an
incomparable authority fails without replacing earlier evidence. Compatible duplicate owners
retain the strongest header prefix, independently of insertion order; adding an older guard
cannot weaken reuse requirements. Required evaluation and callable-result evidence consume this
same source mechanism. Source comparison never performs name lookup, checking or evaluation.
Dependency verification checks the full owner directly, without rebuilding expression projections.
`source::dependencies` projects successful query prerequisites into a flat `BodySourceGraph` of
paired source guards and conjunction edges. `BodyDependencies` selects a root over that shared
graph; unrelated nodes impose no binding requirement. Iterative verification visits each reached
node once, without rechecking semantics or retaining evaluator values. Unknown, self and forward
edges reject during graph construction. Deep chains have no recursive destructor chain.
`collect_sources` compacts one root into the existing checked `BodySources` collection; a standalone
header query therefore retains only selected supplier guards, rather than the whole Unit graph.
`Preparation::finish_with_dependencies` validates that evidence against its completed authority
and retains it with `Query`. A value-dependent header cannot finish without source evidence
covering every registered length expression. The producer additionally owns complete transitive
supplier evidence; guard membership alone is not proof of evaluation. Binding checks both header
syntax and those supplying bodies.
This source proof does not establish a computed value or authorize unfinished type normalization.

`HeaderSource` separately identifies one parameter/result or record-field type annotation by its
own tagged declaration and header position. It shares the same private byte/ancestry verification
as body evidence, but its current root is an annotation node, never executable code. Capture works
before contract readiness and does not resolve its type names. Non-type positions, unknown slots
and foreign authorities reject. Record-owned source is never relabeled as a callable body.

`source::registration` pairs every header length expression with its declaration-owned anonymous
code ID before closing reservations. It traverses record fields, callable parameters/results and
nested tuple/array/callback shapes in canonical owner/slot/source order. Each count has its own
ordinary executable expression root; its enclosing declaration and header position remain in the
semantic code role. `CodeSource` stores a named header only for authored named declarations.
Anonymous code has no name, source parameters or authored result annotation. Its diagnostic origin
and result cause are the real count expression, whose expected usize result belongs to declarations.
`source::recipes` captures and rebinds both named and anonymous executable roots through the same
neutral locator catalog. `BodySource` guards their exact bytes and ancestry without permitting
caller-supplied code/source joins.

`source::expressions` supplies the exact-current `BodyExpressions` catalog on demand for one body.
It uses the shared type projection to register local array lengths, including nested tuple,
callback and closure annotations and annotations inside count bodies. Repetition counts use
syntax's shared seed/count projection; nested seed/count subtrees enter the same traversal.
Each root receives a
`ValueExpression` key in that source owner's local domain; no declaration reservation or global
namespace changes. The node/code reverse index derives from those same roots. Selected sources
reuse `BodySource`, including its full-body guard, rather than a second byte/ancestry mechanism.
An unchanged count does not certify reuse after its lexical body changes. Sibling-body edits and
physical source relocation preserve its keys and guarded binding. Registration establishes no
lexical value availability, expected-type checking, computed length or body acceptance. The
[checking owner](../nocter-checking/README.md#body-local-required-expressions) connects these roots
to ordinary scoped checking and the shared value query, and defines its current admission limits.

Source header normalization uses those anonymous counts through ordinary body checking,
const-domain validation and required usize execution while the enclosing header remains unready.
Closed array references enter the declaration owner's same finite-containment, Copy and type
identity path. A proven zero count removes only the element-payload edge; it does not skip count
computation, type-name validation or independent declarations. Positive containment cycles retain
the owning field diagnostic. Mixed type/value cycles belong to the shared coordinator.

Computed array fields in otherwise admitted private source can close into a complete Unit.
Reusable header queries retain only selected supplying-body guards, including transitive constants
and authored unexecuted callees. Unrelated body edits remain reusable; changing a supplier rejects
binding and a fresh source preparation recomputes the normalized length. The source catalog's
current expression-node/code-ID reverse index is derived once from that same executable-root
catalog and rebuilt during rebinding; it is not another semantic namespace.

Checking owns [listed construction and whole-array handoffs](../nocter-checking/README.md#array-construction-drafts)
through its ordinary body/resource/evaluator products. This header owner grants no such value
proof merely by normalizing a count. Repetition, source element selection and native array storage
retain their separate implementation gates. Native qualification covers scalar code with computed/unused array headers, not array
values or layout. Remaining work belongs to the
[structural request work](../../../design/compiler-realization.md#structural-type-request-closure).
