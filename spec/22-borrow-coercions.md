# Borrow Coercions

**Availability:** Expected-type and explicit selection were published in v0.8.0. One-step method
receiver selection was published in v0.9.0. Instance-owned declarations and structural generic
requirements are implemented for v0.13.0.

A borrow coercion lets a nominal type expose one of its borrowed views at a concrete expected-type
boundary. It is a type-owned, statically selected call. It is not type equality, a representation
cast, or a general conversion graph.

## Declaration

A coercion is declared as a member of an `instance` block for its nominal source type:

```nct
instance String {
    pub coerce &self as &str {
        return view(self)
    }
}
```

The entry source form is defined by [Instances](25-syntactic-grammar.md#instances).

The following rules apply:

- the receiver is exactly `&self` or `&+self`; an owned `self` receiver is invalid
- the target is a borrowed type, `&str`, or a slice view
- the result origin is always the receiver and is normally elided; an explicit clause, when
  written, must be exactly `from self`
- an omitted visibility makes an entry private; `pub(./)`, ancestor scopes, `pub(/)`, and bare
  `pub` expose it through the same boundaries as other declarations
- only the module that defines a nominal type may declare coercions for that type
- the source receiver capability and canonical target type identify an entry, so duplicate entries
  are invalid even when they appear in separate `instance` blocks

A readonly receiver cannot produce a readwrite target. A readwrite receiver may produce either a
readonly or readwrite target, subject to the declared body and provenance contract.

Generic source parameters follow the source type's declaration order:

```nct
instance Vec<T> {
    pub coerce &self as &[T] {
        return view(self)
    }

    pub coerce &+self as &+[T] {
        return view_mut(self)
    }
}
```

The former standalone form `coerce Type { ... }` is invalid. Coercion behavior, methods, and
operators now share one type-owned `instance` surface.

## Generic Requirements

A generic callable can require the same one-step coercion without naming a nominal interface:

```nct
func view<T>(value: &T): &str from value where &T as &str {
    return value
}
```

The source is exactly `&T` or `&+T`, where `T` is a visible generic parameter. The target is a
borrowed type or view and may contain visible generic parameters or associated projections. The
predicate has no parentheses and no trailing result type because the right side already states the
result.

Within the generic body, this predicate is static evidence for contextual conversion, explicit
`as`, receiver-method fallback, comparison, and indexing. At each concrete call, the substituted
source type must expose one accessible coercion to the exact target. The compiler then specializes
the generic evidence to that concrete declaration before lowering. A requirement does not create
a runtime witness, insert a source borrow, permit chaining, or weaken visibility.

## Contextual Selection

The caller must create the source borrow. A coercion never inserts a borrow or a move:

```nct
func measure(text: &str): usize {
    return text.len()
}

let owned = String "Nocter"
let size = measure(&owned)
```

`measure(owned)` is invalid because `owned` is an owned `String`, not a borrowed `String`.

The compiler considers one user-defined coercion only after ordinary exact compatibility fails and
only when both of these facts are known:

- the expression already has a borrowed nominal source type
- the surrounding boundary requires a concrete borrowed target type

The compiler supplies expected types through the common
[Contextual Expected Types](02-values-types.md#contextual-expected-types) boundaries. Grouping and
control-flow branches preserve that expectation. Optional and fallible injection recursively
projects it onto the success or present payload before ordinary compatibility and coercion are
tested. Each resulting leaf owns at most one selected conversion.

For example, every use of `source` below selects the same `&Box<i32> as &i32` entry independently:

```nct
struct Box<T> {
    pub value: T
}

instance Box<T> {
    pub coerce &self as &T {
        return &self.value
    }
}

struct Holder {
    value: &i32
}

func accept(value: &i32): void {
    return
}

func project(source: &Box<i32>): &i32 {
    let binding: &i32 = source
    var assigned: &i32 = binding
    assigned = source
    let holder = Holder { value: source }
    let array: [&i32; 1] = [source]
    accept(source)
    return source
}
```

A readwrite source borrow may use a readonly entry through ordinary capability weakening. A
readwrite target still requires an entry with an `&+self` receiver.

## Explicit Selection

`as` selects the same one-step borrow coercion when the source expression is already a borrow and
the written target exactly matches an accessible entry:

```nct
let text = String "Nocter"
var values: Vec<i32> = Vec [1, 2, 3]

let text_view = &text as &str
let values_view = &values as &[i32]
let values_mut = &+values as &+[i32]
```

The source borrow is mandatory. `text as &str` is invalid when `text` is an owned `String`; write
`&text as &str` instead. Parentheses are not required because prefix borrowing binds before `as`.
Use `&(integer as WiderInteger)` when the intended operation is to borrow a converted numeric
value.

Explicit selection applies either the existing lossless integer rule, built-in borrow capability
weakening, or one accessible exact borrow coercion. It never chains coercions, inserts a borrow,
consumes an owned value, or changes the selected entry based on later generic inference.

## Method Receiver Selection

An ordinary method call first searches the original receiver's inherent methods, explicit
interface conformances, and interface defaults. Only when that lookup has no candidate may the
compiler prepare the normal receiver borrow and apply one declared borrow coercion:

```nct
let text = String "Nocter"
let byte_count = text.len() // selects str.len through &String as &str
```

The selected method keeps its source declaration identity. Hover and completion show
`method &str.len(): usize`; definition and references point to the declaration in `std/str`.

An original method shadows a coerced method even when the original receiver capability is
invalid. Coercions do not chain. Different target method declarations with the same name are
ambiguous and require an explicit `as` conversion. When readonly and readwrite coercions both
reach the same readonly method declaration, the compiler selects the minimum required capability;
a readwrite target method remains available only through a readwrite target.

Receiver preparation, coercion, and method invocation evaluate the receiver expression once. A
borrow-like result preserves the original owning value's loan through optional, fallible,
aggregate, iterator, and generic result shapes.

## Selection Limits

Borrow coercions are deliberately one-step:

- user-defined coercions never chain
- coercion does not choose or infer an otherwise unconstrained generic argument
- method lookup may use one receiver coercion only after original-receiver lookup has no candidate
- equality selection may use one readonly coercion per operand after exact left-owner lookup
- indexing may use one receiver coercion whose exact result has a built-in projection or an
  accessible source-defined index declaration; competing viable targets are ambiguous
- coercion does not convert an owned value and does not consume a value
- coercion cannot return an owned, optional, or fallible value
- coercion cannot declare fresh, static, allocator, parameter, or aggregate result provenance
- `expression as Type` invokes at most one exact borrow-coercion entry or the built-in lossless
  integer/capability rule

These limits keep source evaluation and ownership visible at the call site and prevent imported
packages from creating a transitive or order-dependent conversion graph.

## Execution and Lifetime

Selection records one concrete conversion plan before ownership and lowering. Its stable kind is
lossless integer conversion, capability weakening, or borrow coercion. A borrow-coercion plan also
contains the declaration identity, concrete source and target types, receiver capability, generic
substitution, inferred receiver provenance, and whether the author wrote an explicit clause.
Ownership, regions, analysis, and native lowering consume that same plan; they do not repeat
declaration lookup.

The source expression is evaluated once. Native lowering invokes the selected body as an ordinary
statically resolved borrow-returning call. The resulting value carries the original source loan,
so it cannot outlive the value borrowed by the caller.

## Standard Library Surface

The current standard library provides these public entries:

```nct
instance String {
    pub coerce &self as &str { ... }
}

instance Vec<T> {
    pub coerce &self as &[T] { ... }
    pub coerce &+self as &+[T] { ... }
}
```

`String` intentionally does not expose a readwrite byte view because arbitrary byte mutation could
break its UTF-8 invariant. `String` and `Vec<T>` expose explicit views through `as`; their borrowed
observation methods are owned once by `str` and `[T]` rather than duplicated on the owning types.

## Editor Contract

Editor tooling presents a normalized coercion entry on the declaration's exact `as` token. The
implicit `self` name is a readonly or readwrite parameter according to its receiver. Hovering a
nominal type lists its accessible coercion surface alongside its construction surface. Presentation
preserves whether `from self` was actually written and never synthesizes the elided clause.

On an explicit expression, hover covers only the exact `as` token and describes the selected
conversion kind and concrete source and target. Definition on that token navigates to the selected
coercion entry. Numeric `as` has conversion hover but no declaration target.
