# Literal Definitions and Sequence Spread

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Purpose

A nominal type can expose construction from a language-defined literal shape without revealing its
fields, allocation strategy, or private helpers. The target type remains explicit:

```nct
let values = Vec [1, 2, 3]
let text = String "hello"
```

Bare `[1, 2, 3]` remains a fixed-size array. Literal definitions do not introduce implicit
conversion from an untyped literal to an arbitrary nominal type.

## Definitions

Literal definitions are public members of the target's same-module `construct` declaration:

```nct
construct Vec<T> {
    pub default literal [](...items: T): Self {
        var result = Self.with_capacity(items.len())
        for item in items {
            result.push(move item)
        }
        return move result
    }
}

construct String {
    pub default literal ""(text: &str): Self {
        return Self.copy(text)
    }
}
```

The empty delimiters in a definition select a shape; they are not values passed to the body.

Rules:

- The construct target is a nominal struct or enum declared in the same module.
- Every literal member is explicitly `pub`.
- The body returns `Self` after substituting the construct target's generic parameters.
- Literal construction always executes the declared body.
- A nominal type has at most one definition for each shape. Parameters, bounds, and result types do
  not form an overload set.
- Different supported shapes may coexist on one type.
- A result-provenance clause follows the ordinary callable elision rules. A sequence pack is the
  inferred origin when it is the only eligible input. It may be named explicitly as `from items`
  when another eligible input makes the retained set ambiguous. This names the element origins,
  not the ephemeral pack container.
- Fresh result storage and execution allocation are inferred from the literal body and trusted
  operations; they add no literal modifier.
- Same-module attachment prevents orphan literal definitions for another module's type.

If a type needs multiple construction modes for one shape, it exposes named construction functions
instead of literal overloads.

## Supported Shapes

The current literal shapes are:

- sequence: `Type [elements...]`
- string: `Type "text"` and `Type """text"""`

String definitions receive the decoded `&str` value. Numeric, byte, mapping, tuple-like, and custom
delimiter definitions are not supported.

Typed sequence syntax contains whitespace between the target and delimiter. `Vec [1]` is typed
construction; `values[1]` is indexing.

A generic target may omit all owner arguments when the elements or expected result type determine
them uniquely, as in `Vec [1, 2, 3]`. Otherwise it uses the complete owner type, as in
`Vec<i32> []`. Partial owner arguments are invalid. The uniform rule is specified in
[Generic Owner Arguments](19-construction-surfaces.md#generic-owner-arguments).

## Element Packs

A sequence definition accepts one final `...items: T` capture. Required parameters before the
capture are not supported.

The capture is a compiler-owned, non-escaping element pack. It is not `[T]`, a slice, `Vec<T>`, an
allocated collection, or an ordinary variadic ABI parameter. Its body surface is limited to:

- `items.len()`, which returns the checked total element count cached before body execution
- consuming `for item in items`, which yields owned `T` values once from left to right

The pack cannot be returned, stored, passed to an ordinary callable, or borrowed beyond the literal
body. It can appear after `from` because that contract refers to storage carried by its elements,
not to the pack as a runtime value. Every unconsumed element and iterator suffix retains its normal
drop obligation.

## Sequence Elements and Spread

A typed sequence contains fixed elements and spread segments:

```nct
let joined = Vec [
    0,
    ...copyable,
    ...&borrowed,
    ...move owned,
    4,
]
```

Fixed elements and spread segments form one comma-delimited list and may use one trailing comma on
any layout.

- A fixed expression contributes one owned element.
- `...source` iterates readonly and copies each yielded element; the item type must be `Copy`.
- `...&source` iterates readonly and contributes the yielded readonly references themselves.
- `...move source` consumes a collection or direct iterator and contributes owned yielded values.

A bare spread never guesses that a move-only source should be consumed. Ownership transfer requires
`...move`.

The `move` operand must satisfy the ordinary move-place rules. It cannot be a call result, literal,
or other newly produced temporary. Bind a new collection or iterator before spreading it:

```nct
let source = make_values()
let joined = Vec [...move source]
```

Each spread source uses the same direct-iterator-first selection used by collection iteration. For
`...move source`, direct iterator conformance has fixed priority over an owned expansion. The
selected iterator must provide an exact remaining count; the compiler does not retry an owned
expansion when a directly selected iterator lacks that contract. Unknown-size iterators are
rejected because `items.len()` is fixed before the body starts. The exact-size contract grants no
unchecked memory access; the body still consumes ordinary `next()` results. `...&+source` is
rejected because the pack could retain multiple mutable element loans. See
[Expansion Operators](23-expansion-operators.md).

## Evaluation Order

Evaluation is left to right:

1. resolve the target and literal definition
2. evaluate an explicit `using` allocation context, when present
3. evaluate fixed expressions and spread sources in source order
4. construct each spread iterator once
5. compute one checked total element count
6. enter the literal body and consume the pack

Failure or early exit drops the current element, remaining iterator suffixes, later prepared
segments, and already completed temporaries exactly once according to ordinary ownership order.
Readonly source loans remain active until the literal call finishes.

## Allocation Context

An allocating literal inherits the current aborting allocation context. A call-site override uses:

```nct
let values = Vec [1, 2, 3] using arena
```

The selected context is evaluated before elements and becomes current only for the literal body.
The previous context is restored on success, failure propagation, return, trap-free early exit, and
partial cleanup. Recoverable allocation remains available through explicit named `try_*`
construction APIs rather than a second literal-failure spelling.

## Unsupported `...` Contexts

The `...` token does not currently define variadic function parameters, aggregate initialization
spread, struct embedding, mapping spread, tuple spread, or pattern rest capture. Tooling must reject
or recover those forms without presenting them as current language behavior.
