# Future Private Value Access Declarations

This document owns the bounded handoff between syntax, declarations, typed bodies and accepted
private units for the adopted value-binding goal. Language meaning belongs to the
[future value model](../../spec/language/value-access.md) and
[bounded grammar](../../spec/language/syntactic-grammar.md#bounded-value-binding-fragment).
The earlier type-qualified/result-access checkpoint is retired; its historical records are not
an alternate source mode. This boundary has no released-program adapter or native consumer.

```text
V2CoreUnit syntax + exact source
  -> reusable access-declaration query
       immutable semantic program + neutral header surface + projection recipe
  -> exact-current context
       the same semantic program + current source/header/body binding
  -> reusable typed-body query
       same declaration authority + selected typed operations + body/source recipe
  -> canonical replay of body application types + complete body data projection
       shared declaration facts and one final type authority; exact header source binding retained
  -> source-neutral control/evaluation graph
  -> reachable callable-result producers + recursive closure + identity/value-supply closure
       same graph; closed existing TypeIds, no second type authority
  -> projected initialization facts over the frozen supply operations
  -> permission-demand liveness + alternative referent/access worlds
       + exact payload contracts + logical resource/cleanup transitions
  -> accepted private unit (all definitions validated)
  -> root-closed private InstanceProgram (selected concrete calls/data; no ABI or native admission)
```

## Responsibilities

`nocter-syntax` owns recognition, exact source/tree identity, declaration-surface invalidation,
and stable locators that exclude body contents. The lowered query uses that one projection; it
does not parse source again or invent a second declaration fingerprint.

`nocter-model` owns distinct access-callable/input identities and the independent input operations, local binding
kinds, transient operation access and logical field paths. Data types are independent; access is
not a borrow-type constructor or a general coercion rank. A path contains field identities, not
source names, physical offsets or an access grant.

`nocter-declarations` owns the source-independent program, namespace, closed data authority,
ordered input identities and operations, value results, and concrete named-function
witnesses. A flat callable input introduces a symbolic formal witness; a flat callable result
introduces a symbolic result family. Structural callable
annotations constrain these witnesses rather than introducing erased runtime datatypes. Invocation compatibility and exact input/result matching belong to declarations;
checking freezes the selected witness/view. Header freeze interns every named invocation view and
all permitted formal/result-family views in the header TypeAuthority. Body checking opens a narrow
application-type branch over that prefix; canonical replay validates lineage and owner domains
before all bodies share one final authority. Declaration metadata and source reuse remain shared;
no consumer chooses between header and body stores as lookup fallbacks.
Its builder alone mutates reservations/definitions; freeze rejects incomplete or invalid headers.
Admitted built-in data and finite nongeneric record schemas share the parsed callable/body
boundary. Data-name resolution belongs to that declaration namespace; lowering holds no parallel
type table. Declaration-owned projection validates each selected field's containing record and
data identity. Checking consumes
those schemas to certify concrete payload operations; it does not rebuild nominal type meaning.
No borrowed wrapper or released storable-type heuristic supplies a missing future contract.

`nocter-declaration-lowering` owns canonical header collection, normalization and diagnostic
projection. It reserves functions and records in namespace order and maps resolved identities to neutral
syntax locators. Its reusable query contains no source identity, syntax ID, range, or body text.
Binding checks the complete header surface before resolving locators to current syntax. The
current context has no public parts constructor; a caller cannot pair unrelated program/syntax
products or relabel released source as future source.

## Contract Facts and Limits

All headers are resolved before body semantics. Body contents, source coordinates, declaration
traversal order, and later Copy specialization cannot change a declared input operation. Definition
order cannot renumber parameters. Reordering source declarations may invalidate a
source-surface cache entry but must preserve canonical semantic identities when recomputed.

Every callable result delivers a value of its declared data type. Parameters separately state
Read/Edit/Take, and locals separately state value mutability or scoped aliasing. Type annotations
cannot change those operations. Source/body/definition order cannot infer a new input operation.
Copy-or-move is selected once for value supply. A result whose payload evidence is unresolved
retains a pending operation until witness closure, before initialization. Existing Copy taking
inputs snapshot rather than transfer the original. Exact payload contracts remain checking
evidence, not a generic D/S promise.

Declaration acceptance proves only header integrity. A syntax-valid body with an invalid non-Copy extraction,
invalid update, unavailable local, or unsafe backedge may rebind that same declaration program and
must later fail checking. Body typing consumes those identities/contracts and performs lexical body resolution,
structural data/operation checks, and direct-call selection. It cannot recover input operations
from body tokens, redo declaration normalization, or treat source projection as dispatch authority.
Projected initialization and referent/permission validation share a source-neutral body-flow
graph. Body typing freezes each selected place's local root, field identities and data TypeId;
later passes consume the selection rather than repeating member lookup. Logical resource transitions and the exact first-fragment payload contracts share those
alternative worlds. Only the complete private-unit constructor closes all body/callee assumptions;
intermediate facts cannot be treated as its accepted product.

The released compile-input/target-selection boundary rejects this future root, and no conversion
from this declaration product to the released program exists. Public roots, module joining,
general compound header types, generic/interface evidence, dependent results without an
established required input witness, nested callback contracts, captured environments, outcomes, async/effects, layouts,
standard-library migration and native output remain outside this implemented boundary.


## Typed-Only Body Boundary

Checking owns one leaf query per callable. The declaration context shares the exact immutable
program authority; the query owns that authority and callable together with source-neutral body
arenas. Place reads, update/transfer operations, boundary Copy/delegation, argument preparations,
RHS-first stores, checked primitive arithmetic and structured control retain separate operations.
Direct and value calls store one statically selected callable and one input operation per ordered
input. Value-supply preparation can remain pending until result closure, without changing that
authored operation. A value call prepends its environment to explicit inputs; direct declaration
calls have no runtime environment input. Downstream validation consumes frozen operations rather
than reinterpreting indices against declaration parameters. A direct
place reservation is not an early read/update/transfer; explicit computed operations execute at
their own operand point. A direct Copy input snapshots its selected place at preparation,
without a child read or retained source access. Copy/transfer/pending direct supply share one
graph topology; closure cannot reorder nested evaluation or cancellation. Later validation must
consume those plans, not repeat dispatch or infer intent from source punctuation. Structural
record values retain the declaration-selected field and one ordinary value operation for each
initializer in authored order. Their component values
remain owned evaluation temporaries until a single assembly step transfers them into the complete
record. An interrupted initializer follows existing evaluation-frame cleanup; it cannot publish
a partially assembled record or introduce a constructor-only resource authority.

Named function references retain code identity through ordinary Copy supply, scoped aliases and
same-witness joins. A selected invocation adapter materializes a value and changes only its view;
initialization and replacement share that boundary. Environment preparation follows the same
reservation, atomic commit, cancellation and cleanup graph as explicit arguments. Exact
capture-free witness schemas justify Copy, trivial destruction and empty outside D/S; structural
annotations alone do not justify any of those payload facts. Formal inputs have no Copy evidence
and retain symbolic cleanup. Each selected argument can bind a callee formal to a named or caller
formal witness; invocation through the formal retains that semantic target. All definitions are
checked against their declared contracts, including unused/recursive bodies. The supported producer
domain still excludes captured environments. Result-producing bodies establish one named
or own-input relation through reachable value flow; signatures alone cannot select it. Each
selected application substitutes its actual witnesses into that relation. General dependency
footprints require further source closure. Concrete instance closure consumes
only the already selected witnesses.

Known result types and possible normal completion are independent typed facts. Interrupted
operands do not erase declared callable/record data, while the shared graph cannot evaluate an
unreached value merely because its data is known. A computed member chain retains one producer,
resolved field path and selected Copy/Move operation. Its producer executes once; later passes
consume the path without member lookup. Existing-place selection and construction use that same
field-resolution boundary. Computed selection delivers a value and cannot supply a scoped alias,
assignment target or edit input by extending a temporary's lifetime.

The semantic body has no source/syntax IDs, ranges, names, or symbol IDs. Exact body bytes and
body-local locators form a separate reusable source recipe. One locator projection serves the
initial checking context. Rebinding checks shared declaration-authority identity and exact own
body bytes, then resolves current origins without repeating typing. A result-closed query also
verifies its shared producer-body dependencies; changing a factory body invalidates that proof
even when the caller is unchanged. Complete-unit binding verifies this evidence once before
lending an opaque proof to body projections. Equal declarations rebuilt by an unrelated query
cannot substitute for that authority. A raw pending query can rebind its own unchanged body but
cannot enter flow or accepted-unit consumers until result closure succeeds.

No public constructor assembles a typed body or its semantic/source pairing from separate parts.
Structural failures expose their typed rule and current diagnostic; no partial typed body escapes.
A structurally valid transfer still requires initialization and conflict proofs. An ordinary
return must leave one value resource of its declared data type after frame cleanup. Passing an
intermediate query does not accept a unit. This product has no adapter to released checked
evidence. Its complete accepted Unit is consumed by the target owner's private instance closure;
typed-only evidence cannot enter that consumer.


## Flow and Initialization Handoff

Checking shares one immutable typed query into its body-flow query. The latter owns one closed
source-neutral control/evaluation graph and the fixed-point projected availability facts derived from it.
Semantic operations reference the already selected expressions, statements, locals, block exits and
call plans. No source identity, syntax node, rendered name or independently supplied declaration
program can influence this computation. Binding goes through the query's own typed source recipe;
changed authority/body is rejected before diagnostics can use stale coordinates.

Before access/resource validation, checking propagates static code/view producer facts through
this same graph's reachable successors, without depending on initialization. Actual staged
return values and prepared argument producers, rather than provisional expression types,
establish result equations. Finite monotone closure requires a named or own-input anchor and
preserves invocation views. Selected call applications substitute actual bindings into those
established relations, then deferred identity equalities are proved. Projection closes existing type/witness identities and freezes pending value supply
from the exact payload schema and source access. It does not repeat typing or graph construction;
known moves remain moves. Initialization then runs once over the frozen operations, followed by
access/resource validation. Producer facts do not prove owner availability and cannot replace
resource validation.

The initialization report distinguishes unreachable points, selected-value completeness and
container presence. Sparse abstract states join projected availability and preserve reaching
invalidation evidence and potential cleanup conditions without expanding shared record schemas. These are not an
access-root proof or executable cleanup schedule. The referent analysis now shares this graph and authority rather than reconstructing control or
dispatch. It preserves alternative origin worlds and staged return protection. Evaluation-frame
operations carry normal and interrupted temporary cleanup through the same graph. Logical owner
transitions are validated alongside referent survival; executable target realization remains separate.
The development product still has no public language-version switch or released/native adapter.


## Referent Report Boundary

The same flow query now owns read/update demand liveness and the referent/permission report.
Initialization failure is an explicit prerequisite status, not an empty access proof. Root and
creation/use identities remain semantic; exact-current diagnostics bind through the owning typed
query and locate the attempted operation, retained access, required use and referent declaration.

An owned answer with no outside referent can still carry D/S, and
non-owning access release differs from destroying a prepared owned resource. No consumer may
reinterpret an empty violation list as a complete accepted-body, execution or native certificate.
The exact first-fragment payload evidence is independent of R: it comes from the admitted
scalar/unit and owned-error snapshot contracts. Future dependency-bearing data cannot inherit
that evidence from ownership, Copy, or an empty referent set.

## Logical Resource and Unit Acceptance Boundary

Each reached action retains alternatives of before/after logical ownership and ordered cleanup
transitions under the same typed/control authority. The solver tracks expression, named, prepared,
retained-read-temporary and staged-result owners without dynamic execution counters. A complete
normal return preserves exactly its declared value result and no live evaluation frames. Cleanup
does not disappear when access liveness ends. A fresh value supplied to a read input retains one
temporary owner until its enclosing evaluation frame ends or unwinds. Call results are values;
they do not carry an implicit alias to an input. Computed field extraction transfers only the
selected component, leaving the source remainder in its existing evaluation frame. Copy leaves
the source complete. The same resource ledger proves result staging, read retention and remainder
cleanup; known-but-interrupted types are not resource owners.

Resource transitions use logical holder/field places and retain sparse unavailable selections.
Container presence is distinct from complete-value initialization. The ledger proves both in
each exact alternative; the initialization report joins availability for diagnostics over that
same selected graph. These products do not grant competing move or cleanup semantics. Closed
nominal cleanup retains an intact subtree as one schema operation and expands only around missing
selections. Partial drop ends the present container after cleaning its remaining fields.
Referent regions use the same selected field paths: parent/child paths overlap, sibling paths do
not. Source admission closes projected initialization, demand/reservation and resource validation
together; native realization remains separate.

The accepted private-unit constructor derives its membership from the declaration authority and
checks every definition before publication. A caller cannot omit an unsafe unused callee, combine
different header generations, or construct acceptance from intermediate reports. Rebinding checks
the whole original body set before exposing any exact-current projection. No lookup, typing,
dispatch, or flow computation is repeated by that projection.

This is closed front-end evidence for the exact private fragment. Target layout, error-storage
runtime realization, executable cleanup/code generation, module/package integration, general
dependencies, user destructors, outcomes and async remain separate contracts. There is no released
checked-program conversion or public compatibility mode.

## Private Instance Closure Handoff

`nocter-target-program` consumes the complete accepted Unit and an explicit semantic root to produce
one private InstanceProgram. It retains the exact Unit/body/type authority, never a released-program
adapter or source context. An entry with unbound callback formals is not a concrete root. This
internal failure does not introduce a public CLI or target ABI rule.

Checking supplies selected declaration/formal targets, formal-to-actual bindings, direct/value
invocation kind, explicit preparation actions, initialization reachability and resource obligations.
The target owner substitutes these facts into concrete instance keys and call edges. It does not
repeat name lookup, signature compatibility, body checking, access proof or Copy selection. The
source-level generic move remains selected even if its concrete named payload is Copy.

One immutable instance arena closes root reachability; recursive keys are reserved before their
edges are followed. Only reachable invocation commits add instances. Interrupted inputs lacking a
witness therefore create no unbound executable edge. Source data IDs map once to concrete payload
obligations or checked unproduced structural contracts in the same declaration TypeAuthority.
The accepted Unit owns that distinction; a type-only application has no payload, cleanup or
environment to specialize. Checking rejects any reached value use of it before flow admission.
Checking owns the payload classifier; the
projection resolves a symbolic named-code environment to its exact cleanup contract without
inventing an owner. Closed record schemas are visited once per data identity, not expanded by path.
Consumers receive each instance with its exact checked body and those frozen maps.

The body's checking-closed result data enters that same payload projection. The target owner
never re-infers a result producer. Returned formal data uses the instance's existing binding and
payload maps. Captured environments and general retained footprints still need their own source
closure. Layout, physical environment/argument realization, executable cleanup, target services,
MIR and native output are not established by this graph. Later implementation must extend this
same instance authority rather than create another call graph or infer environment shape from
parameter counts.
