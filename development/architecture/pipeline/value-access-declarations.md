# Future Private Value Access Declarations

This document owns the currently implemented bounded handoff between syntax, declaration lowering,
declaration metadata, typed bodies and accepted front-end units for the future private-function goal. Language meaning remains in the
[future grammar and header rules](../../../spec/language/syntactic-grammar.md#first-access-validation-fragment)
and [Value Access Model](../../../spec/language/value-access.md). This is not the complete
v2.0.0 pipeline, a second user-facing language version, or a native execution contract.

```text
V2CoreUnit syntax + exact source
  -> reusable access-declaration query
       immutable semantic program + neutral header surface + projection recipe
  -> exact-current context
       the same semantic program + current source/header/body binding
  -> reusable typed-body query
       same declaration authority + selected typed operations + body/source recipe
  -> exact-current typed-body projection
  -> source-neutral control/evaluation graph + named-slot initialization facts
  -> permission-demand liveness + alternative referent/access worlds
       + exact payload contracts + logical resource/cleanup transitions
  -> accepted private unit (all definitions validated; no native consumer)
```

## Responsibilities

`nocter-syntax` owns recognition, exact source/tree identity, declaration-surface invalidation,
and stable locators that exclude body contents. The lowered query uses that one projection; it
does not parse source again or invent a second declaration fingerprint.

`nocter-model` owns distinct access-callable/input identities and the independent permission/data
shape descriptor. Permission is not a borrow capability or a general coercion rank.

`nocter-declarations` owns the source-independent program, namespace, scalar data authority,
parameter ownership and ordered input identities, exact modes, and result-referent upper bounds.
Its builder alone mutates reservations/definitions; freeze rejects incomplete or invalid headers.
Only the admitted built-in data shapes cross this boundary. No borrowed wrapper or released
storable-type heuristic supplies a missing future contract.

`nocter-declaration-lowering` owns canonical header collection, normalization and diagnostic
projection. It reserves functions in namespace order and maps resolved identities to neutral
syntax locators. Its reusable query contains no source identity, syntax ID, range, or body text.
Binding checks the complete header surface before resolving locators to current syntax. The
current context has no public parts constructor; a caller cannot pair unrelated program/syntax
products or relabel released source as future source.

## Contract Facts and Limits

All headers are resolved before body semantics. Body contents, source coordinates, declaration
traversal order, and later Copy specialization cannot change a declared mode. Definition order
cannot renumber parameters or result origins. Reordering source declarations may invalidate a
source-surface cache entry but must preserve canonical semantic identities when recomputed.

An independent I Copy answer retains I. A U result retains access to eligible caller storage,
including for zero-sized unit. Result-referent sets are alternative upper bounds, not simultaneous
active U accesses and not proof of payload/storage independence. An empty bound cannot create an
access or make a local survive exit. Private body checking can establish narrower facts; this
private product cannot invent an omitted public `from` contract.

Declaration acceptance proves only header integrity. A syntax-valid body with a missing transfer,
invalid update, escaping local, or unsafe backedge may rebind that same declaration program and
must later fail checking. Body typing consumes those identities/contracts and performs lexical body resolution,
structural data/access checks, and direct-call selection. It cannot recover parameter/result modes
from body tokens, redo declaration normalization, or treat source projection as dispatch authority.
Named-slot initialization and referent/permission validation share a source-neutral body-flow
graph. Logical resource transitions and the exact first-fragment payload contracts share those
alternative worlds. Only the complete private-unit constructor closes all body/callee assumptions;
intermediate facts cannot be treated as its accepted product.

The released compile-input/target-selection boundary rejects this future root, and no conversion
from this declaration product to the released program exists. Public roots, module joining,
compound type trees, generic/interface/callable evidence, outcomes, async/effects, layouts,
standard-library migration and native output remain outside this implemented boundary.


## Typed-Only Body Boundary

Checking owns one leaf query per callable. The declaration context shares the exact immutable
program authority; the query owns that authority and callable together with source-neutral body
arenas. Place reads, update/transfer operations, boundary Copy/delegation, argument preparations,
RHS-first stores, checked primitive arithmetic and structured control retain separate operations.
Direct calls store the selected callable and one preparation action per ordered argument. A direct
place reservation is not an early read/update/transfer; explicit computed operations execute at
their own operand point. Later validation must consume those plans, not repeat dispatch or infer
intent from source punctuation.

The semantic body has no source/syntax IDs, ranges, names, or symbol IDs. Exact body bytes and
body-local locators form a separate reusable source recipe. One locator projection serves the
initial checking context. Rebinding checks shared declaration-authority identity and exact own
body bytes, then resolves current origins without repeating typing. Equal declarations rebuilt by
an unrelated query cannot substitute for that authority. Changing another body can reuse this
query only after the declaration query binds the current complete syntax.

No public constructor assembles a typed body or its semantic/source pairing from separate parts.
Structural failures expose their typed rule and current diagnostic; no partial typed body escapes.
A structurally valid transfer still requires initialization and conflict proofs; a structurally
valid access return still requires a surviving allowed referent. Seven of the eleven eventual flow rejections fail named-slot initialization; the four others now
fail referent/permission validation. Passing an intermediate query still does not accept a unit. This product
has no adapter to released checked evidence and no executable consumer.


## Flow and Initialization Handoff

Checking shares one immutable typed query into its body-flow query. The latter owns one closed
source-neutral control/evaluation graph and the fixed-point named-slot facts derived from it.
Semantic operations reference the already selected expressions, statements, locals, block exits and
call plans. No source identity, syntax node, rendered name or independently supplied declaration
program can influence this computation. Binding goes through the query's own typed source recipe;
changed authority/body is rejected before diagnostics can use stale coordinates.

The initialization report distinguishes unreachable points from maybe initialized slots and
preserves reaching invalidation evidence and potential cleanup conditions. These are not an
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
retained-I-temporary and staged-result owners without dynamic execution counters. A complete
normal return preserves only its X result obligation, if any. Cleanup does not disappear when
access liveness ends. Temporary roots protect an I call answer until its enclosing evaluation
finishes, without keeping an unmentioned local owner alive.

The accepted private-unit constructor derives its membership from the declaration authority and
checks every definition before publication. A caller cannot omit an unsafe unused callee, combine
different header generations, or construct acceptance from intermediate reports. Rebinding checks
the whole original body set before exposing any exact-current projection. No lookup, typing,
dispatch, or flow computation is repeated by that projection.

This is closed front-end evidence for the exact private fragment. Target layout, error-storage
runtime realization, executable cleanup/code generation, module/package integration, general
dependencies, user destructors, outcomes and async remain separate contracts. There is no released
checked-program conversion or public compatibility mode.
