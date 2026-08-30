# Argument Packs, Literal Definitions, and Sequence Spread

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Purpose

A callable may accept a statically typed, variable number of final arguments without converting
them into a source-level collection:

```nct
func total(seed: i32, ...items: i32): i32 {
    var result = seed
    for item in items { result += item }
    return result
}

let answer = total(10, 20, 12)
```

The same argument-pack contract implements typed sequence literals. One pack model therefore owns
arity, inference, ownership, provenance, cleanup, exact-size spread, and native calling convention
for functions, methods, construction functions, interface methods, and sequence literals.
Argument packs are typed and compiler-owned. They are not C variadics, arrays, slices, `Vec<T>`,
or an erased runtime argument list.

## Declaration Form

`...name: T` declares an argument pack whose element type is `T`.

```nct
pub func log(prefix: &str, ...values: &str): void

instance Logger {
    pub method &self.write(level: Level, ...values: &str): void
}
```

Rules:

- A supported callable has at most one pack parameter.
- The pack is the final declared parameter. Ordinary parameters may precede it.
- Functions, construction functions, methods, interface methods, and their matching
  implementations may declare a pack.
- Primitives, operators, coercions, drop declarations, tests, variants, and string-literal
  definitions cannot declare a pack.
- Contract and implementation declarations must agree on the pack position and element type.
- A pack marker participates in callable-type identity. `func(i32): void` and
  `func(...i32): void` are different structural callable contracts.
- Closure literals do not declare argument packs in the current language, so a structural
  callable-value contract containing a pack has no invocable source value yet. Pack invocation in
  this phase is statically dispatched through named functions, construction functions, methods,
  and literals.

A sequence-literal definition is the restricted construction form with exactly one pack and no
ordinary parameter:

```nct
construct Vec<T> {
    pub literal [](...items: T): Self {
        var result = Self.with_capacity(items.len())
        for item in items { result.push(move item) }
        return move result
    }
}
```

A string-literal definition retains its one ordinary `&str` parameter and cannot use a pack.

## Calls and Spread

Fixed arguments satisfy the ordinary parameters. Every remaining argument contributes one pack
element:

```nct
write(header, first, second, third)
```

A spread contributes the elements produced by an exact-size iterator:

```nct
write(header, 0, ...copyable, ...&borrowed, ...move owned, 4)
let values = Vec [0, ...copyable, 4]
```

- A fixed expression contributes one owned value of the pack element type.
- `...source` iterates readonly and copies each yielded element; the item type must be `Copy`.
- `...&source` iterates readonly and contributes the yielded readonly references.
- `...move source` consumes a collection or direct iterator and contributes owned yielded values.
- `...&+source` is rejected because a retained pack could hold overlapping mutable element loans.
- Spread is invalid when the selected callable has no argument pack.

Each spread uses the expansion and iterator rules in
[Expansion Operators](23-expansion-operators.md). The selected iterator must also satisfy the
exact-size contract because the total pack length is fixed before the callee begins. The compiler
does not fall back to another expansion after selecting a direct iterator that lacks exact size.

An argument-pack parameter is not an ordinary value or a general expansion source. It may appear
in one dedicated tail-forwarding form:

```nct
func forward(prefix: &str, ...items: &str): void {
    write(prefix, ...items)
}
```

The forwarded pack must be the only contribution to the destination pack. Fixed destination
parameters may precede it, but new pack values or sequence spreads cannot be mixed before or after
`...items`. Tail forwarding passes the remaining compiler-owned descriptor through unchanged; it
does not expose the descriptor as a value, copy it, or build an implicit adapter. A body may
tail-forward its pack once and may not also iterate that pack; these rules ensure that the
descriptor's cached total length still describes the complete forwarded stream.

## Body Surface

A pack is non-escaping and supports three dedicated body uses:

- `items.len()` returns the total checked element count cached before body execution.
- `for item in items` consumes owned `T` values once from left to right.
- `target(fixed, ...items)` tail-forwards the remaining descriptor under the rule above.

The pack cannot be returned, stored, borrowed, dropped as an ordinary value, or passed as one
ordinary argument. A returning tail-forwarding call exhausts the stream: the callee either
iterates values or performs its mandatory residual cleanup before returning. The source body may
still read `items.len()` as the immutable original count, but cannot iterate or forward the pack
again. Every unconsumed value and iterator suffix retains its normal destruction obligation until
that cleanup.

An identifier after `from` may name a pack. It denotes storage provenance carried by the pack's
elements, not the lifetime of the ephemeral descriptor:

```nct
func first(...items: &str): &str? from items
```

The ordinary provenance-elision rules may omit this clause when the pack is the only eligible
external origin.

## Evaluation, Ownership, and Failure

Evaluation is left to right:

1. resolve the callable and its complete generic substitution
2. evaluate a receiver and any explicit allocation override
3. evaluate fixed arguments, fixed pack elements, and spread sources in source order; tail
   forwarding evaluates no additional source value
4. construct every selected spread iterator once
5. compute one checked total pack length
6. invoke the body through its ordinary inputs and one hidden pack input

Moving a fixed element or a `...move` source follows normal explicit-move rules. A bare spread
never guesses that a move-only source should be consumed. Readonly loans created for spread remain
active until the call finishes.

Failure propagation, early return, and partial iteration destroy the current element, remaining
iterator suffixes, later prepared segments, and completed temporaries exactly once. The caller
transfers pack ownership to the callee for the duration of the invocation; the pack cannot escape.
Tail forwarding transfers access to the same remaining descriptor for that nested invocation, so
residual cleanup remains owned exactly once by the original descriptor.

## Native ABI

An argument pack occupies one compiler-owned hidden ABI lane independent of ordinary arguments.
The lane carries a descriptor pointer whose closed contract provides:

- the immutable original total length
- a callback that yields the next `T?`
- a callback that destroys the unconsumed suffix

Fixed parameters retain their normal ABI locations. The pack is never lowered as one ordinary
`T`, a slice, a `Vec<T>`, or platform variadic arguments. TargetProgram fixes the element type,
iterator dispatch, destruction plans, ordinary/pack split, and whether a call creates or forwards
the descriptor. MIR transports those facts; MachineProgram and the target backend may not
reconstruct semantic dispatch.

## Literal Definitions

A nominal type can expose construction from a language-defined literal shape without revealing its
representation:

```nct
let values = Vec [1, 2, 3]
let text = String "hello"
```

Literal entries are public construction members. A private implementation of a public bodyless
contract omits visibility. Literal construction always executes the selected body, returns `Self`,
and permits at most one member per shape. Same-module attachment prevents orphan definitions.

The current shapes are sequence `Type [elements...]` and string `Type "text"`. Bare `[1, 2, 3]`
remains a fixed-size array, while bare string syntax remains static `&str`. Numeric, byte, mapping,
tuple-like, and custom delimiter definitions are unsupported.

A generic target may omit all owner arguments when pack elements or the expected result determine
them uniquely. Otherwise it uses the complete owner type, such as `Vec<i32> []`. Partial owner
arguments are invalid.

## Literal Allocation Context

An allocating literal inherits the current aborting allocation context. A call-site override uses:

```nct
let values = Vec [1, 2, 3] using arena
```

The override is evaluated before pack elements and becomes current only for the literal body. The
previous context is restored on success, failure propagation, return, and partial cleanup.
Recoverable allocation uses explicit named `try_*` APIs rather than a second literal-failure form.

## Unsupported `...` Contexts

Argument packs and call spread do not introduce aggregate-initializer spread, mapping spread,
tuple spread, pattern rest capture, struct embedding, mutable spread, untyped variadics, or a
source-level pack value type.

## v0.21.0 Keyed Packs and Mapping Literals

The v0.21.0 working tree extends the same compiler-owned pack model with keyed entries. This
section is implemented in the working tree but not in the latest published release.

A keyed pack has one key type and one value type:

```nct
func load(...entries: &str: i32): void {
    for key: value in entries {
        consume(key, value)
    }
}

load("one": 1, "two": 2)
```

The declaration `...entries: K: V` is one final pack parameter. It is not two packs, an
alternating flat value pack, a tuple, or named-argument syntax. A callable has at most one final
pack, either ordinary or keyed. Its structural callable contract writes `func(...K: V): O`.

Fixed keyed entries are evaluated key first and then value, from left to right. The callee receives
the cached checked entry count before consuming any entry. Its dedicated body operations are:

- `entries.len()` for the original entry count;
- `for key: value in entries` to consume each owned key and value once;
- `target(...entries)` to tail-forward the entire remaining keyed descriptor once.

The same non-escape, exhaustion, residual cleanup, provenance, and early-exit rules as an ordinary
pack apply to both components. Cleanup destroys an initialized key and value exactly once even
when evaluation, invocation, or iteration stops between entries. A keyed pack may appear after
ordinary fixed parameters, but an invocation cannot mix ordinary pack elements and keyed entries.

The initial keyed-pack phase supports fixed entries and exact tail forwarding. It does not define
spread from a collection because Nocter has no general pair value or pair-expansion contract yet.

A mapping literal is the restricted construction form with exactly one keyed pack and no ordinary
parameter:

```nct
construct Map<K, V> {
    pub literal [:](...entries: K: V): Self {
        var result = Self.with_capacity(entries.len())
        for key: value in entries {
            result.insert(move key, move value)
        }
        return move result
    }
}
```

Nonempty use-site syntax is `Type [key: value, ...]`; the empty form is `Type [:]`. A bare mapping
literal is not introduced. Mapping literals share typed-literal generic inference, allocation
override, evaluation, ownership, and construction-member visibility with sequence literals. The
associative collection behavior selected by the standard `Map` declaration is specified in
[Associative Collections](27-associative-collections.md).

The keyed native descriptor is one ABI lane. TargetProgram fixes both component types, entry
evaluation, next-entry initialization, and residual cleanup. MIR, MachineProgram, and the target
backend transport that plan and cannot reconstruct key/value pairing from alternating values or
source punctuation.
