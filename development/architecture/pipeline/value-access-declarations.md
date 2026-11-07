# Future Private Value Access Declarations

This document owns the currently implemented bounded handoff between syntax, declaration lowering,
and declaration metadata for the future private-function goal. Language meaning remains in the
[future grammar and header rules](../../../spec/language/syntactic-grammar.md#first-access-validation-fragment)
and [Value Access Model](../../../spec/language/value-access.md). This is not the complete
v2.0.0 pipeline, a second user-facing language version, or a native execution contract.

```text
V2CoreUnit syntax + exact source
  -> reusable access-declaration query
       immutable semantic program + neutral header surface + projection recipe
  -> exact-current context
       the same semantic program + current source/header/body binding
  -> body checking (not implemented yet)
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
must later fail checking. Subsequent checking consumes selected identities/contracts, performs
lexical body resolution and access/flow validation, and constructs its own accepted product; it
cannot recover modes from source tokens, redo declaration normalization, or treat the source
projection as dispatch authority.

The released compile-input/target-selection boundary rejects this future root, and no conversion
from this declaration product to the released program exists. Public roots, module joining,
compound type trees, generic/interface/callable evidence, outcomes, async/effects, layouts,
standard-library migration and native output remain outside this implemented boundary.
