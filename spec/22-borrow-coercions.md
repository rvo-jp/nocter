# Borrow Coercions

**Availability:** Published in v0.8.0.

A borrow coercion lets a nominal type expose one of its borrowed views at a concrete expected-type
boundary. It is a type-owned, statically selected call. It is not type equality, a representation
cast, or a general conversion graph.

## Declaration

A `coerce` declaration belongs to the nominal type named after the keyword:

```nct
coerce String {
    pub &self as &str from self {
        return self.view()
    }
}
```

An entry has this grammar:

```text
visibility? receiver `as` target `from` `self` block
```

The following rules apply:

- the receiver is exactly `&self` or `&+self`; an owned `self` receiver is invalid
- the target is a borrowed type, `&str`, or a slice view
- `from self` is mandatory because the result remains tied to the source loan
- `pub` makes an entry available outside its defining module; an entry without `pub` is private
- `pub(nocter)` is not accepted on an entry
- the enclosing `coerce` declaration has no visibility modifier
- only the module that defines a nominal type may declare coercions for that type
- the source receiver capability and canonical target type identify an entry, so duplicate entries
  are invalid even when they appear in separate `coerce` blocks

A readonly receiver cannot produce a readwrite target. A readwrite receiver may produce either a
readonly or readwrite target, subject to the declared body and provenance contract.

Generic source parameters follow the source type's declaration order:

```nct
coerce Vec<T> {
    pub &self as &[T] from self {
        return self.view()
    }

    pub &+self as &+[T] from self {
        return self.view_mut()
    }
}
```

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

The compiler supplies a concrete expected type at these boundaries:

- an explicitly typed binding initializer
- a simple assignment
- a callable argument
- a struct field initializer
- a fixed-array element initializer
- a typed-sequence literal capture
- an enum payload argument
- an explicit or final-expression return

Grouped expressions propagate that expectation to their inner expression. `if`, `if is`, and
`match` propagate it independently to every value-producing branch. Optional and fallible
projection preserves the expectation on the projected success or present value. Each resulting
leaf owns at most one selected conversion.

For example, every use of `source` below selects the same `&Box<i32> as &i32` entry independently:

```nct
struct Box<T> {
    pub value: T,
}

coerce Box<T> {
    pub &self as &T from self {
        return &self.value
    }
}

struct Holder {
    value: &i32,
}

func accept(value: &i32): void {
    return
}

func project(source: &Box<i32>): &i32 from source {
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

## Selection Limits

Borrow coercions are deliberately one-step in v0.8.0:

- user-defined coercions never chain
- coercion does not choose or infer an otherwise unconstrained generic argument
- coercion does not participate in member lookup, operator typing, overload ranking, construction,
  or literal selection
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
substitution, and `from self` provenance. Ownership, regions, analysis, and native lowering consume
that same plan; they do not repeat declaration lookup.

The source expression is evaluated once. Native lowering invokes the selected body as an ordinary
statically resolved borrow-returning call. The resulting value carries the original source loan,
so it cannot outlive the value borrowed by the caller.

## Standard Library Surface

The v0.8.0 standard library provides these public entries:

```nct
coerce String {
    pub &self as &str from self { ... }
}

coerce Vec<T> {
    pub &self as &[T] from self { ... }
    pub &+self as &+[T] from self { ... }
}
```

`String` intentionally does not expose a readwrite byte view because arbitrary byte mutation could
break its UTF-8 invariant. Existing explicit `view` and `view_mut` methods remain available.

## Editor Contract

Editor tooling presents a normalized coercion entry on the declaration's exact `as` token. The
implicit `self` name is a readonly or readwrite parameter according to its receiver. Hovering a
nominal type lists its accessible coercion surface alongside its construction surface.

On an explicit expression, hover covers only the exact `as` token and describes the selected
conversion kind and concrete source and target. Definition on that token navigates to the selected
coercion entry. Numeric `as` has conversion hover but no declaration target.
