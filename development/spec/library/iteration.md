# Traversal Protocols

## Declaration Surface

The adopted v2.0.0 std/iteration surface contains the following ordinary interfaces. They are
unreleased contracts; this chapter does not modify published std declarations or provide a
runtime implementation. [Iteration and expansion](../language/iteration.md) own language
scheduling and built-in carrier behavior. [Interfaces](../language/interfaces.md) own conformance.

```text
interface ReadCursor {
    type Item
    method edit self.advance(): bool
    operator (self[]): Self.Item from self
}

interface EditCursor where Self impl ReadCursor {
    operator (edit self[]): Self.Item from self
}

interface ValueIterator {
    type Item
    method edit self.next(): Self.Item?
}

interface ReadSource {
    type Cursor impl ReadCursor
    operator (... self): Self.Cursor from self
}

interface EditSource {
    type Cursor impl EditCursor
    operator (... edit self): Self.Cursor from self
}

interface TakeSource {
    type Cursor impl ValueIterator
    operator (... take self): Self.Cursor from self
}
```

Associated Cursor bindings select one concrete data identity; they do not store a dictionary or
erase its code/layout. An Item binding is valid storable data, including void, not never.
ReadSource, EditSource and TakeSource are independent capabilities. EditCursor inherits the
one ReadCursor.Item declaration; it does not bind a second Item. Source-interface Cursor
declarations are distinct, so an actual collision uses ordinary associated-type selection.

The source interfaces bundle an explicit expansion-family requirement with its cursor bound.
A concrete declared expansion operator can supply the same recorded operation directly when
its result has the required cursor conformance. A generic body needs that exact operation/result
evidence, either from its source-interface bound or its declared operator/cursor requirements.
These facts resolve to one selected operator; they are not ranked fallback paths. Matching only
method names is insufficient, and the cursor conformance remains nominal and explicit.

The selected toolchain binds traversal roles to these exact declaration identities once.
An unrelated interface/member with an equal spelling/shape gains no language privilege.
Re-exporting the same identity preserves it. No ambient name lookup, method-name duck typing,
runtime probing or source-order search selects a traversal. The released prelude/export set
and compiler role registry remain unchanged until qualified migration.

## Operational Contracts

A source operator receives its declared read/edit/take input and constructs one cursor value.
Origins distinguish that receiver's direct storage from already carried outside dependencies;
a taking source cannot return access into storage its own cleanup destroys. Public source
contracts can expose an opaque nominal cursor with private fields. This surface does not add
an implementation-specific opaque result position to operators or required interface members.

A read/edit consumer starts before its first step. It calls advance; false stops
that traversal without projecting. True selects one current-entry projection for that iteration.
The projection returns an initialized existing place on normal completion under its read/edit
contract. It is not an ordinary method returning an alias. Advancement may invalidate or replace
cursor-backed storage only when existing access/reservations and cleanup no longer require it.
The complete checked schema distinguishes cursor storage, revoking guards and surviving backing.

ValueIterator.next returns a value: absence stops, presence delivers one Item under the ordinary
optional/value rules. A delivered Item can still have outside dependencies. Repeated calls may
require edited access to the cursor's state. They cannot leave external referents incompletely
initialized on a recoverable exit or hand out non-Copy ownership through a read projection.
Implementations needing removal/exchange use an independently checked operation with that
contract, not a reinterpretation of edit access as ownership.

Only the outer optional is the end marker. If Item is T?, next returns T??: a present absent
item is still an item. If Item is T!, next returns `(T!)?`; a present failure is delivered as
ordinary fallible item data, without implicit loop propagation. An Item of error is data too.
The ordinary failure-loss warning still applies when a delivered fallible item is discarded.

The consumer stops at the first false/absence. There is no requirement that a later manual call
stay exhausted, no finite-length/purity promise, no snapshot of the source, and no automatic
lookahead. Every selected operation and cleanup keeps its ordinary effects and trap/divergence
behavior. Structural conformance alone cannot remove a validity check or prove a semantic law
about ordering, progress or stable backing; optimization needs the corresponding checked proof.

The protocols neither prescribe contiguous memory nor allocate one object per item. Cursor and
item placement follow the ordinary storage proof. Actual constructor, projection, next/advance,
drop and backend effects remain part of checking, especially within an async body's drive.
These are immediate synchronous operations; no executor, async iterator, separate fallible-step channel or
automatic error propagation is introduced. Such an API needs its own advertised contract.

## Implementations and Authority

Named methods can map to private helpers under ordinary conformance rules. Operators bind by
their fixed operation family/shape, not helper-name aliases. Read/edit/take source operators
are three distinct families, selected before authority checking. Equal component types do not
make one family satisfy another. EditCursor requires both read and edit current projections.

Fixed arrays receive language-owned structural source/cursor evidence. Packs have a checked
argument-sequence binding rather than an ordinary data type, and use their intrinsic entry map.
The language chapter owns their acquisition, projection, transfer and cleanup behavior. Their
concrete identities/evidence must be registered under the same declaration/type authority during
implementation; backends cannot recover them from names or copy another standard-library body.

Generic source/interface solving, capability/write schemas, source cases, executable lifecycles,
target realization and actual std declaration migration remain qualification gates. An interface
listing, synthetic evidence plan or documentation build proves none of those implementations.
