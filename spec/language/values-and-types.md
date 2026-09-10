# Values and Types

This chapter defines built-in and structural type forms, expected-type propagation, `Self`,
associated projections, built-in literals, and aliases.

## Built-in and Structural Types

Nocter is value-centered. Data is represented with explicit value types.

Named built-in types:

```text
bool
f32 f64
i8 i16 i32 i64
u8 u16 u32 u64
usize isize
char
str
error
void
never
```

Each name above is declared exactly once by the compiler-selected standard package with a
`primitive type` declaration. The declaration is the source authority for the type's canonical
name, documentation, navigation target, and inherent-surface ownership; the compiler supplies its
semantic identity and representation. Named built-in types are available in every source type
context without an import and cannot be shadowed or redeclared by ordinary source.

The active standard package declares floating-point, integer, and boolean types in `std/num`, `char` in `std/char`,
`str` in `std/str`, `error` in `std/error`, and the completion types `void` and `never` in `std/core`.
These declarations do not make a built-in nominal or structural type: they have no fields,
variants, body, generic parameters, source construction form, or source-defined layout.

Structural and contextual type syntax:

```text
*T
&T
&+T
[T]
&[T]
&+[T]
future T
T?
T!
T?!
[T; N]
(T)
Self
```

`future T` is a move-only deferred computation whose eventual output is `T`. Its execution and
ownership rules are defined in [Asynchronous Computations](asynchronous-computations.md). `T!`
means a fallible value whose success payload is `T` and whose failure payload is the built-in
`error` type. `T?!` means a fallible value whose success payload is optional.

Supported optional and fallible compositions are ordinary sized
values. They may be stored in bindings and sized aggregates, moved, assigned, passed as arguments,
returned, and consumed later. An optional with no fallible layer is copyable exactly when its
recursively contained payload is copyable. Every fallible value and every mixed outcome containing
a fallible layer is move-only because its failure branch owns an `error`. Only the selected tag
branch is initialized. Absence never initializes a success payload, and failure initializes the
`error` payload instead of the success payload. The complete rules are defined in
[outcome copyability](ownership.md#copy-and-move).

Outcome construction at a callable return boundary is contextual, not a subtype conversion. Each
presence or success injection follows the complete declared result type; an expression that already
has that complete type keeps its existing tags unchanged. The normative
algorithm is [Recursive Outcome Injection](errors-and-optionals.md#recursive-outcome-injection).

Nocter supports one optional layer, one fallible layer, or one of each in either order. Repeated
equal layers and deeper recursive outcome types are not supported.

`void!` is valid and represents a recoverable operation with no success value. An optional layer
must not ultimately wrap `void`; `void?`, `void?!`, `(void?)!`, and `(void!)?` are invalid. This
restriction is checked after alias expansion and generic substitution, so an otherwise valid
generic `T?` cannot be instantiated with `T = void`. Use an enum when absence and successful
completion are observably different states.

`never` is a control-flow termination type, not an outcome payload. `never?`, `never!`,
`never?!`, and `(never!)?` are invalid after alias expansion and generic substitution. Use `void!`
for a recoverable operation that has no success value, and use an enum for a value-level state.

After alias expansion, `never` may appear as the complete result of a function, method, closure,
or structural callable type. A type alias may name `never`, but using that alias remains subject to
the same position rule. `never` is invalid as a binding or parameter type, borrow or pointer
pointee, aggregate field or enum payload, array element, outcome payload, generic argument,
associated-type binding, or any other data-bearing type position. Use `*void` for an opaque raw
pointer.

`void` is a completion type, not a zero-sized value type. After alias expansion, it may appear as
the complete result of a function, method, closure, or structural callable type; as the direct
success completion of `void!`; and as the pointee spelling of opaque `*void`. It is invalid as a
binding or parameter type, borrow pointee, aggregate field or enum payload, array element, optional
payload, generic argument, associated-type binding, or any other data-bearing type position. A
type alias may name `void`, but does not bypass these use-site rules. Use an empty struct when a
storable zero-sized unit or marker value is required.

Pointer and borrow type operators bind more tightly than postfix outcome operators. `future`
instead consumes a complete type operand. Therefore `&T?` is an
optional readonly borrow, while `&(T?)` is a readonly borrow of an optional value. Parentheses in
type syntax group a type without creating a new type. `future T!` means `future (T!)`, while
`(future T)!` is an immediately produced fallible value whose success payload is a computation.

### Contextual Expected Types

An authoritative expected type flows from a destination into its expression at these boundaries:

- an explicitly typed binding initializer
- a simple assignment
- a callable argument
- a struct field initializer
- a fixed-array element initializer
- a typed-sequence literal capture
- an enum payload argument
- a `catch` or `otherwise` fallback result
- an explicit `return` or callable body result
- a contextually typed closure result

Grouping preserves the same expectation. `if`, `if is`, and `match` propagate an enclosing
expectation independently to every value-producing branch. The expected payload type of `catch`
and `otherwise` comes from the operated-on outcome; it does not need a further enclosing
destination.

An expected `void` result is a completion consumer rather than a value destination. An expression
of type `void` may be evaluated there and then complete normally, as in `return log_message()`.
When the expected type is `void!`, recursive outcome injection evaluates a `void` expression and
constructs payloadless success only after that expression completes. This does not make `void` a
storable value or a valid generic substitution.

Optional and fallible values use
[Recursive Outcome Injection](errors-and-optionals.md#recursive-outcome-injection) at these
boundaries. Outcome injection is directional: it consumes an expected type already supplied by
the program context. It does not infer an outcome wrapper from an unannotated initializer or from
a sibling control-flow branch.

```nct
let present: i32? = 42
let absent: i32? = none
let failed: i32! = error.new("app.failed", "operation failed")

let missing = none // error: no expected optional type
```

For a generic expected type with statically known outcome structure, inference may project through
those outcome layers and collect constraints for the payload. Injection occurs only after the
substitution is unique. `none` and a failure `error` select tags but contribute no payload-type
constraint, so they cannot determine an otherwise unknown generic parameter. A `never` expression
terminates before producing an argument or result and likewise contributes no type constraint; it
is checked only after another source determines the expected type. A `void` completion expression
also contributes no generic payload constraint.

```nct
func inspect<T>(value: T?): void {
    return
}

inspect(42)   // T = i32; inject presence after inference
inspect(none) // error: T cannot be inferred from absence
```

### Self Type Syntax

`Self` is type-position syntax, not an ordinary user-defined name.

`Self` is valid only in type positions owned by a type or interface declaration: an `instance`, an
interface member signature or default body, or a `construct` entry.

Meaning:

- In `instance File { ... }`, `Self` means `File`.
- In `interface Source { ... }`, `Self` means the eventual implementing type.
- In `instance File { impl Source ... }`, `Self` means `File`.
- In `construct File { ... }`, `Self` means `File`.

Rules:

- `Self` cannot be used as a value expression.
- `Self` cannot be used as a binding name, parameter name, function name, method name, field name, enum variant name, module name, type declaration name, type parameter name, or import alias.
- `Self` is not resolved through normal name lookup.
- `Self` is not imported or exported.
- `Self` has no meaning outside a type- or interface-owned type position.
- Lowercase `self` is not special. It is an ordinary identifier if it is otherwise valid in that syntactic position.

This preserves Nocter's rule that ordinary names do not define special behavior. The special behavior belongs to type syntax, not to a value or declaration name.

### Associated Type Projections

Type selections are resolved from left to right. When the prefix names an imported module
namespace, `.Name` selects one exported type declaration, as in `parser.Parser<T>`. Once the prefix
denotes a type, `.Name` is an associated type projection selected by an interface implementation.

```nct
func next<S>(source: &+S): S.Item? where S impl Source {
    return source.next()
}
```

`Self.Item` is valid when the current interface declares `Item`. `S.Item` requires exactly one
interface requirement on `S` to declare `Item`. A concrete projection such as `FileSource.Item`
requires exactly one applicable interface implementation that binds `Item`. Projection normalization also
applies beneath existing type constructors, so `Vec<S.Item>`, `S.Item?`, and `&S.Item` retain their
ordinary outer type rules.

An unknown or ambiguous selection is an error. Type arguments may follow a module-selected nominal
type, but not an associated projection because generic associated types are not supported. Nocter
does not select a declaration by import order, interface spelling, or the name `Item`.
Associated-type declarations, bindings, and constraints are specified in
[Generics, Interfaces, and Methods](generics-and-interfaces.md#associated-types).

Built-in literal values:

```text
true
false
none
```

`true` and `false` have type `bool`. `none` is a contextual optional absence literal and requires an expected `T?` type.

User-defined typed literal construction, such as `Vec [1, 2, 3]` or `Path "README.md"`, is specified
in [Argument Packs, Literal Definitions, and Sequence Spread](literals-and-packs.md). It does
not change the meaning of built-in literals.

Built-in core type forms include `str`, `error`, `[T]`, `&str`, `&[T]`, `&+[T]`, and `[T; N]`. These forms are type-position syntax, not ordinary names imported from a module. In particular, `error` may still be used as a value binding name, such as the conventional binding in `catch error { ... }`.

Primitive scalar and view storage sizes are part of the target
[ABI and Layout](../platform/abi-and-layout.md#struct-layout) contract. Register transport does not widen their
stored aggregate fields.

`str` is unsized UTF-8 string data. `[T]` is unsized contiguous array data. These unsized data forms cannot be used by value as parameters, return values, fields, local annotations, optional payloads, fallible success payloads, or generic arguments unless they are behind an indirection. Use `&str` for a string slice, `&[T]` for a readonly array slice, `&+[T]` for a readwrite array slice, `String` for owned variable-length text, and `Vec<T>` for owned variable-length arrays.

Nominal types, interfaces, functions, constants, and statics supplied by the standard library are
ordinary resolved declarations, not compiler built-ins.

The compiler does not treat `Int` specially, and the standard-library prelude does not export it. User code should write `i32` or define a project-local alias when a domain-specific name is useful.

### Type Aliases

`type` declares a pure type alias. A type alias introduces another name for the exact same type. It does not create a distinct nominal type.

```nct
pub type Count = i32
pub type Bytes = [u8]
pub type Items<T> = Vec<T>
```

Rules:

- Type aliases are top-level declarations.
- Type aliases are private by default.
- A non-private `pub(...) type` makes the alias importable inside its declared visibility boundary.
- Bare `pub type` makes the alias importable and re-exportable across packages.
- Generic type aliases are allowed.
- A type alias has no separate identity from its target type.
- A direct or indirect alias-expansion cycle is invalid because it has no finite exact target type.
- A type alias does not change ownership, copyability, drop behavior, layout, or ABI.
- Implementations cannot target a type alias.
- A type alias cannot be used to create a type-safe wrapper around an existing type.
- There is no dedicated `newtype` syntax.
- Use a `struct` when a distinct type is required.

Examples:

```nct
let x: Count = 10
let y: i32 = x  // OK: Count is i32
```

```nct
type UserId = u64
type OrderId = u64

let user: UserId = 10
let order: OrderId = user  // OK: both aliases are u64
```

```nct
pub copy struct UserId {
    pub value: u64
}
```

```nct
instance Count {
    ...
}
// error: instance target must be a nominal type, not a type alias
```
