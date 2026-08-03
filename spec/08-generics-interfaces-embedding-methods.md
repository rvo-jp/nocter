# Generics, Interfaces, Embedding, and Methods

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## v0.2.0 Scope

Nocter v0.2.0 includes generic type parameters, associated functions, inherent
`impl` blocks, receiver methods, contract-only `interface` declarations,
explicit interface conformance declarations, and `Self` type syntax inside
inherent member and interface method contexts.

Nocter v0.2.0 does not include traits.
Embedding is an adopted future composition feature, but it is not part of the
v0.2.0 implementation contract. Typed literal construction, literal rest capture, implemented
sequence spread, and future variadic capture are specified separately in
[Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md).

Not part of v0.2.0:

- `trait` declarations
- embedding declarations such as `...Type` and `pub ...Type`
- typed literal construction such as `Vec [1, 2, 3]`
- generalized `...` spread, rest capture, and variadic capture forms
- generic bounds such as `T: Interface`
- interface-bound method lookup
- interface objects such as `dyn Printable`
- interface inheritance, associated types, default methods, blanket impls,
  specialization, and `where` clauses
- code reuse through interfaces

`trait` is not a reserved keyword in v0.2.0. It is lexed as an identifier. A source
form that starts a top-level item with `trait` is diagnosed as removed syntax,
but the spelling remains available as an ordinary identifier in
positions such as a function name.

`interface` is a reserved keyword.

## Impl Blocks

Adopted: `impl` associates receiver methods and destructor members with a
nominal type. It is not a class declaration and does not introduce inheritance.

Associated functions are declared at top level with a qualified function name.
They have no receiver and are called through the type.

```nct
pub func WordStats.empty(): WordStats {
    return WordStats {
        bytes: 0,
        lines: 0,
        words: 0,
    }
}
```

```nct
let stats = WordStats.empty()
```

`method` inside an `impl` defines a receiver method. The receiver is explicit
and appears before the method name.

```nct
impl WordStats {
    pub method &+self.add_byte(byte: u8): void {
        self.bytes += 1
    }

    pub method &+self.add_word(): void {
        self.words += 1
    }
}
```

Generic impl blocks declare their own type parameters before the target type.
Those parameters are in scope for the impl target, method signatures, drop
members, and member bodies.

```nct
impl<T> Box<T> {
    pub method self.value(): T {
        return self.value
    }
}
```

```nct
stats.add_word()
```

`Self` is type-position syntax inside an inherent `impl` block and inside a
qualified associated function declaration such as `func WordStats.empty`.
It is not an ordinary identifier and is not resolved through normal name lookup.
In `impl WordStats` or `func WordStats.empty`, `Self` means `WordStats`.

`self` is the fixed receiver name in method and drop member declarations.
`Self` remains type-position syntax. The restrictions on `Self` are specified
in [Values and Types](02-values-types.md#self-type-syntax).

The target of an `impl` block must be a nominal type declaration, such as a
`struct` or `enum`. An `impl` block cannot target a type alias because aliases
do not create distinct types.

```nct
type Count = i32

impl Count {
    ...
}
// error: Count is a type alias, not a nominal type
```

`impl Interface for Type` declares explicit conformance to an interface. It is
not an inherent impl block and cannot contain members.

```nct
impl Printable for User
impl Printable for User {}
```

Generic conformance declarations use the same impl generic parameter list:

```nct
impl<T> Source<T> for Box<T>
```

The implementing methods are ordinary public inherent methods on the target
type.

```nct
impl User {
    pub method &self.print(): i32 {
        return 0
    }
}
```

Initial receiver forms:

```nct
method &self.name(...): Return
method &+self.name(...): Return
method self.name(...): Return
```

Meaning:

- `&Self` is a readonly receiver.
- `&+Self` is a readwrite receiver.
- `Self` is a consuming receiver. It requires copy or explicit move according to
  the normal ownership rules.
- Calling a `&Self` method borrows the receiver readonly.
- Calling a `&+Self` method borrows the receiver readwrite and requires a
  writable receiver place.
- A newly created owned temporary may be used as a `&+Self` receiver for that
  single method call because it has no existing aliases.
- Calling a `Self` method consumes or copies the receiver according to the
  receiver type.
- Borrow-like values derived from a temporary receiver cannot escape the current
  statement.

Call rules:

- `Type.function(args)` calls an associated `func`.
- `value.method(args)` calls a `method`.
- Associated function and method arguments follow the positional argument rules
  in [Control Flow](03-control-flow.md#function-calls-and-arguments).
- `Type.method(&value, args)` and `Type.method(&+value, args)` are invalid in
  v0.2.0.
- `value.function(args)` is invalid when `function` is only an associated
  `func`.
- `func Type.name` and `method` share the same member namespace for a type.
  Defining both with the same member name for the same type is an error in v0.2.0.
- Enum variants also occupy the type member namespace. An associated `func` or
  `method` cannot reuse an enum variant member name in v0.2.0.
- If method lookup finds multiple valid inherent candidates, the call is
  ambiguous and is a compile error.
- v0.2.0 has no qualified method-call escape hatch for ambiguity resolution.

```nct
file.write_text("hello")?          // OK: method call
File.write_text(&+file, "hello")?  // error: methods are not UFCS functions
```

## Method Lookup

Adopted: method lookup is deliberately small and deterministic in v0.2.0.

For `value.method(args)`, the compiler first determines the static type of
`value`.

If the receiver has a concrete nominal type, the compiler looks only for
inherent methods declared in `impl Type` blocks for that nominal type.

If the receiver is a generic type parameter, v0.2.0 has no interface-bound method lookup.
A method call through an unconstrained generic receiver is invalid unless a
future feature supplies a bound and lookup rule.

Nocter v0.3.0 Phase 4 supplies that rule for an explicit interface bound:

```nct
func read<S: Source<i32>>(source: &S): i32 {
    return source.read()
}
```

Lookup on `S` searches only the canonical `Source<i32>` contract. Each concrete specialization
requires explicit conformance and statically resolves to the matching public inherent method.
Phase 9 extends this to a finite capability set while retaining static dispatch:

```nct
func inspect<I: Iterator<T> + ExactSizeIterator<T>, T>(iterator: &I): usize {
    return iterator.remaining_len()
}
```

The set is resolved by specialized interface declaration identity. If two distinct bounds declare
the requested method name, the call is ambiguous even when their displayed signatures match.
Runtime interface objects and `where` clauses remain unavailable.

Concrete-receiver lookup order remains:

1. inherent method on a concrete nominal receiver type
2. no candidate, producing a compile error

Bounded generic-receiver lookup in v0.3.0 Phase 9 is separate:

1. resolve the generic parameter's canonical interface-bound set
2. search only accessible methods declared by those interfaces
3. typecheck against the specialized interface signature
4. require explicit conformance for every reachable concrete specialization
5. lower directly to the matching public inherent method

The compiler never falls back to an inherent method merely because it has the same name as a
missing bound method.

The compiler does not search visible interface conformance declarations to resolve
`value.method(args)` in v0.2.0. This avoids import-dependent method lookup and keeps
calls readable from the receiver type.

Initial implementation order:

1. `impl Type { ... }` receiver methods
2. `Self` inside `impl` and `func Type.name`
3. associated function declarations such as `func Type.function(...)`
4. associated function calls such as `Type.function(...)`
5. method declarations
6. method calls such as `value.method(...)`

## v0.2.0 Interface Contracts

An interface is a contract-only nominal declaration. It may be private,
`pub`, or `pub(nocter)` like other top-level definitions. Every member inside
an interface must be explicitly marked `pub`.

```nct
pub interface Printable {
    pub method &self.print(): i32
}
```

Rules:

- Interface members are method signatures only.
- Interface method signatures cannot have bodies.
- Interface members cannot be private or `pub(nocter)`.
- Interfaces cannot declare fields, associated functions, `drop` members,
  default methods, associated types, or reusable code.
- `Self` inside an interface method signature means the eventual conformance
  target type for structural checking.
- An interface conformance declaration is written `impl Interface for Type`.
- A conformance declaration may omit `{}` or use an empty `{}` body.
- A conformance declaration body cannot contain members.
- The conformance target must be a nominal `struct` or `enum`.
- Typechecking verifies that the target has a public inherent method with the
  same name and signature for every interface method.
- Method parameter names do not participate in conformance. Receiver type,
  parameter types, and return type do.
- A Phase 4 result provenance clause participates in conformance. An implementation may promise a
  narrower, longer-lived origin set, but it cannot omit the explicit relationship required by the
  interface contract.
- Conformance is explicit. A type with matching methods does not satisfy an
  interface unless source contains `impl Interface for Type`.

Example:

```nct
interface Reader {
    pub method &+self.read_byte(): i32!
}

struct File {
    fd: i32
}

impl File {
    pub method &+self.read_byte(): i32! {
        ...
    }
}

impl Reader for File
```

This model prevents accidental conformance while keeping the contract check
structural. It also keeps code reuse out of v0.2.0: interface declarations describe
requirements only.

## v0.3.0 Phase 10 Interface Defaults

Phase 10 permits a public interface method to carry a default body. A bodyless method remains a
conformance requirement. A body-bearing method is reusable behavior derived from the interface
contract and does not add a conformance requirement.

```nct
pub interface Counter {
    pub method &+self.next(): i32?

    pub method self.count(): usize {
        var source = move self
        var total: usize = 0
        loop {
            source.next() otherwise { return total }
            total += 1
        }
    }
}
```

An explicit `impl Interface for Type` must satisfy every bodyless method. Default bodies are checked
with `Self` constrained by their declaring interface and are statically specialized at use sites.
An applicable inherent method takes precedence and acts as an explicit override. If two proven
interfaces supply an otherwise applicable default with the same name, the call is ambiguous.
Import or declaration order never resolves that ambiguity.

See [Callable Values and Interface Default Methods](18-callables-default-methods.md) for the Phase
10 callable and lookup contract.

## Interface And Embedding Separation

Adopted: Nocter separates stateless interface reuse from stored composition.

An `interface` describes a public capability. Phase 10 permits default method bodies derived from
that capability. An interface still does not store data, inject fields, or establish conformance.

Embedding owns another value inside a struct and promotes only that value's
public contract through the embedding owner. It is Nocter's planned
composition-based reuse feature. It is not inheritance, not a trait, not a
mixin, and not implicit interface conformance.

This separation is part of Nocter's core direction:

- `interface` answers "what public capability does this type promise, and what stateless behavior
  follows from it?"
- `embedding` answers "what contained value does this type own and expose?"

The two features may work together, but neither feature includes the other.
A type that embeds a value does not automatically conform to an interface, and
an interface default does not own or expose embedded state.

## Generics

Adopted: generic type parameters use angle brackets.

```nct
struct Buffer<T> {
    ...
}

func first<T>(items: &[T]): T? {
    ...
}
```

Generic parameter grammar in v0.2.0:

```text
GenericParameters = "<" GenericParameter ("," GenericParameter)* ">"
GenericParameter  = Name
```

Generic bounds are not part of v0.2.0. Nocter v0.3.0 Phase 4 adds one interface bound; Phase 9 adds
a finite `+`-separated set:

```text
GenericParameters = "<" GenericParameter ("," GenericParameter)* ">"
GenericParameter  = Name [":" InterfaceBound ("+" InterfaceBound)*]
InterfaceBound    = Type
```

```nct
func inspect<T: Readable<i32>>(value: &T): i32 {
    return value.read()
}
```

Every bound must resolve to an interface with the declared type arity and visibility. Bound order
is formatting information; semantics use a set of specialized interface declaration identities.
Duplicate identities are invalid. `where` clauses, interface inheritance, and runtime interface
values remain unavailable.

Phase 9 also permits a conformance declaration's generic parameters to carry bounds. Such a
conditional conformance exists for a concrete target only when all specialized bounds hold:

```nct
impl<T, I: Iterator<T>> Iterator<T> for TakeIter<T, I>
```

Nocter rejects identical normalized target/interface patterns rather than selecting between
overlapping conditional conformances.

Generic implementation uses monomorphization. Each concrete instantiation is
compiled as concrete code.

```nct
Buffer<i32>
Buffer<String>
```

This keeps generic dispatch static, avoids runtime type metadata for basic
generics, and fits the no-runtime direction.

Initial generic scope:

- type parameters on structs
- type parameters on functions
- type parameters on `impl` blocks where needed
- compile-time monomorphization

Deferred generic features after v0.3.0 Phase 9:

- full `where` clauses
- higher-kinded types
- generic associated types
- const generics beyond the minimum needed for fixed-size arrays

## Embedding

Adopted future design: embedding is Nocter's privacy-preserving composition
feature.

Embedding declares that a struct stores an unnamed value of another struct type.
The embedding owner may use only the embedded type's public surface. Private
fields, private methods, private associated functions, and other implementation
details of the embedded type are not promoted, even when the embedding owner is
declared in the same module.

Initial syntax:

```nct
struct Profile {
    ...User
    pub ...Article

    visits: i32
}
```

In the embedding subset, leading `...` is recognized only in struct bodies and
struct literals. Sequence spread and other contextual uses of `...` are defined separately in
[Literal Definitions and Sequence Spread](17-literal-definitions-sequence-spread.md)
so embedding does not become a hidden import, macro, or general delegation
mechanism.

Meaning:

- `...User` stores a `User` value and promotes `User`'s public instance members
  to the `Profile` implementation scope only.
- `pub ...Article` stores an `Article` value and promotes `Article`'s public
  instance members as public `Profile` members.
- The embedded values do not receive source-level field names such as `user`,
  `article`, `User`, or `Article`.
- The embedded types do not know that they are embedded.

Embedding is directional. If `Profile` embeds `User`, `Profile` owns a `User`
value. `User` does not gain access to `Profile`, does not see `Profile`'s
private or public members, and does not dispatch to `Profile` overrides.
`self` inside a promoted `User` method is still the embedded `User`, not the
outer `Profile`.

Example:

```nct
struct User {
    id: u64
    name: String
    pub age: i32
}

impl User {
    pub method &self.print(): void {
        print(self.name)
    }
}

struct Article {
    title: String
    pub text: String
}

struct Profile {
    ...User
    pub ...Article

    visits: i32
}

impl Profile {
    method &self.run(): void {
        self.print() // OK: User.print is public, promoted inside Profile
        self.id      // error: User.id is not public
        self.age     // OK: User.age is public, promoted inside Profile
        self.title   // error: Article.title is not public
        self.text    // OK: Article.text is public and promoted
        self.visits  // OK: Profile's own private field
    }
}
```

Outside `Profile`'s implementation scope:

```nct
let profile = make_profile()

profile.print()  // error: User was embedded privately
profile.id       // error: User.id is private to User
profile.age      // error: User was embedded privately
profile.title    // error: Article.title is private to Article
profile.text     // OK: Article was embedded publicly and text is public
profile.visits   // error: visits is private to Profile
```

### Embedded Initialization

Struct literals initialize embedded values with embedded initializers:

```nct
func Profile.new(
    name: String,
    age: i32,
    title: String,
    text: String,
): Profile {
    return Profile {
        ...User.new(move name, age),
        ...Article.new(move title, move text),
        visits: 0,
    }
}
```

Rules:

- An embedded initializer has the form `...expr`.
- `expr` must have the embedded target type after generic substitution.
- Every embedding declaration must be initialized exactly once.
- Unknown embedded initializers are compile errors.
- Duplicate embedded initializers are compile errors.
- Field initializers and embedded initializers are evaluated left to right in
  the order written in the struct literal.
- If initialization fails through postfix `?`, already initialized ordinary
  fields and embedded values are dropped in reverse initialization order.
- A struct may not embed the same concrete target type more than once in the
  initial design.

### Promotion And Visibility

Embedding promotion is not ordinary import, not ordinary module-private access,
and not a named field.

Only the embedded type's public instance members are promotable:

- public fields
- public `&self` receiver methods
- public `&+self` receiver methods

Not promoted in the initial design:

- private fields
- private methods
- associated functions
- `drop` members
- enum variants
- type aliases
- nested declarations
- consuming `self` receiver methods
- `pub(nocter)` members

Consuming receiver methods are not promoted initially because calling one would
partially move the embedding owner. A future design may allow them only after
partial-move and drop-state rules for embedded values are fully specified.

For `...T`, promoted members are visible only inside the embedding owner's
implementation scope:

- inherent methods in `impl Owner`
- `drop &+self` in `impl Owner`
- qualified associated functions declared as `func Owner.name`

They are not visible to unrelated top-level functions in the same module.
This is stricter than ordinary module-private visibility because embedding is a
type-contract boundary.

For `pub ...T`, promoted members become public members of the embedding owner.
They are available anywhere the owner type is visible and the promoted member's
use satisfies normal borrow, assignment, and ownership rules. Reading or writing
a promoted public field reads or writes the field inside the embedded value.

### Name Collisions

Embedding does not provide renaming, aliasing, override order, or explicit
disambiguation.

A promoted member name must not collide with:

- an ordinary field declared directly by the embedding owner
- an inherent method declared directly by the embedding owner
- a member promoted by another embedding
- another member promoted by the same embedding target

Collisions are compile errors regardless of whether the colliding members would
be private or public after promotion.

Rationale: a collision means the composed API has stopped being self-evident.
Nocter requires the author to choose clearer names at the source types instead
of adding local rename syntax.

### Interface Interaction

Embedding does not create implicit interface conformance.

```nct
pub interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: u64
}

impl User {
    pub method &self.print(): i32 {
        return 0
    }
}

struct Profile {
    pub ...User
}

impl Printable for Profile // explicit conformance is still required
```

When checking an explicit `impl Interface for Owner`, public methods promoted by
`pub ...T` may satisfy interface requirements as public `Owner` methods.
Methods promoted by private `...T` cannot satisfy a public interface because
they are not public on `Owner`.

### Non-Goals

Embedding is not:

- class inheritance
- subclassing
- trait implementation reuse
- mixins
- extension methods
- implicit conversion
- automatic delegation to private implementation details

Class inheritance, mixins, extension declarations, and implicit conformance are not part of the
core language direction.
