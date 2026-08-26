# Generics, Interfaces, and Methods

This file is part of the Nocter language specification. The specification entry point is
[README.md](README.md).

## Generics

Generic declarations use angle brackets:

```nct
struct Buffer<T> {
    value: T
}

func first<T>(items: &[T]): T? {
    if items.len() == 0 {
        return none
    }
    return items[0]
}
```

A generic parameter list declares names and arity only. A `where` clause declares every nominal
interface, structural callable, intrinsic copy, operator, coercion, expansion, and pattern-refinement
requirement. The complete
recognition grammar is centralized under
[Generic Requirements](25-syntactic-grammar.md#generic-requirements).

Names in an explicit generic parameter list are unique. A nested declaration cannot redeclare a
generic name visible from its enclosing declaration; it must use that existing parameter or choose
a new name. Declaration type patterns are the sole exception to spelling repetition: their first
occurrence declares a binder and later occurrences refer to that same binder, as specified below.

Every nominal interface requirement must resolve to an accessible interface with the declared type
arity. Requirement order is formatting information; semantics use specialized interface
declaration identities and associated bindings. Duplicate interface requirements are invalid.

`copy` is an intrinsic requirement, not an interface or a type modifier. A callable may rely on
implicit copies of `T` only when its contract contains `where copy T`. A concrete call satisfies the
requirement only when its substituted type is copyable under the ownership rules.

Callables can further constrain a generic parameter inherited from a surrounding `construct`,
`instance`, or `interface` scope:

```nct
construct Buffer<T> {
    pub func from_view(values: &[T]): Self where copy T {
        ...
    }
}
```

The clause follows result provenance and precedes a callable body. On a struct, enum, or interface,
it follows the generic parameter list and precedes the body. On an `instance`, it follows the
target. On a type alias, it follows the aliased type. A requirement target must be a generic
parameter or associated projection visible to that declaration. Duplicate `copy` requirements and
duplicate interface requirements are invalid. `copy` is unavailable after `impl` and is invalid
inside a type expression such as `&[copy T]`.

An interface requirement uses `impl` and may bind that interface's associated types in braces:

```nct
where T impl Iterator { Item = &str }
```

`impl` is reserved for nominal interfaces. It cannot introduce an intrinsic copy, callable,
operator, coercion, or expansion requirement. Associated bindings belong to the immediately
preceding interface application, are compared after alias expansion, and apply recursively beneath
existing type constructors. A generic body may rely only on implementations and bindings in its
lexical predicate environment. A concrete call or conditional interface implementation must prove
every specialized binding. Cycles that cannot normalize to a finite type, unresolved operands,
duplicate bindings, and names absent from the selected interface are invalid.

An operator requirement encloses the required expression in parentheses and states its result type:

```nct
where (&T == &T): bool
where (&T < &T): bool
where (&C[K]): &V
where (&+C[K]): &+V
```

The equality and strict-order forms require two readonly borrows of the same visible generic
parameter. A concrete type satisfies either through the matching accessible instance-owned
comparison declaration or the same one-step readonly borrow coercions used by a non-generic
comparison expression. Equality and ordering remain independent evidence. The index forms describe a
readonly or readwrite projection from a borrowed generic container. Their result borrow must have
the same capability as the container borrow. A concrete container satisfies an index requirement
through a built-in projection, an accessible instance-owned index declaration, or one accessible
receiver coercion to either operation. Generic specialization uses the same selector as an ordinary
index expression. Operator requirements produce no runtime witness.

`instance` does not have a prefix generic parameter list. Its target header is a declaration type
pattern. Each generic argument slot contains a bare binder name; its first occurrence declares the
binder and later occurrences reuse the same identity:

```nct
instance Pair<L, R> { ... }
```

Concrete and nested types do not appear directly in a pattern slot. A declaration introduces a
binder and applies a directed refinement after the header:

```nct
instance Vec<T> where T = i32 { ... }
```

In this context, `where T = Type` is a binder refinement rather than symmetric projection
equality. The left operand must be a binder declared by the same pattern, the right operand cannot
contain that binder, and one binder cannot have two refinements. Refinements affect method and
instance and interface-implementation applicability. Overlapping patterns are rejected; a more concrete refinement never
wins by ranking or source order.

An independent `drop TypePattern(&+self) { ... }` declaration is uniform across every
specialization of a nominal type. Its pattern must use each target slot through one distinct
binder and cannot have a `where` predicate. A `copy struct` family cannot declare one, including
when one particular specialization is move-only. This keeps generic ownership and ABI behavior
independent of conditional method availability and prevents a conditional declaration from
changing copyability.

```nct
func inspect<T>(value: &T): i32 where T impl Readable<i32> {
    return value.read()
}
```

Generic implementation uses monomorphization. Predicate equality and binder refinement are
compile-time only and create
no witness, metadata, dictionary, or ABI field. Nocter does not provide runtime generic metadata,
interface objects, interface inheritance, higher-kinded types, generic associated types, or general
const generics.

### Callable Type-Argument Inference

A function, method, construction function, interface member, or closure-call contract may declare
its own generic parameters. Call sites never write those callable type arguments explicitly. The
compiler requires one unique substitution inferred bidirectionally from:

- the method receiver, when present
- ordinary call arguments and parameter types
- contextual closure parameter and result checking
- the expected type of the complete call result
- associated bindings and statically witnessed callable annotations that propagate types already learned from those
  sources

Nominal capability, copy, operator, coercion, and expansion requirements validate a candidate
substitution; they do not choose an otherwise unknown type. Declaration order, visible
interface implementations, overload ranking, return provenance, and callable body contents do not supply guesses.
If any callable parameter remains unknown or multiple substitutions remain viable, the call is an
error and the caller must provide an expected type at the surrounding expression boundary.

When a parameter has statically known optional or fallible structure, inference projects through
that structure before contextual outcome injection. A plain payload argument can therefore infer
the projected parameter and is wrapped only after inference succeeds. `none` and an `error` used
as failure select an outcome tag but add no constraint for the projected payload type. Another
argument, the receiver, a contextual closure, or the expected call result must determine it.
A `never` argument expression also contributes no substitution constraint because it produces no
argument value. Once other sources determine the parameter, the `never` expression is compatible
with that expected type and terminates before the call.
A `void` completion expression likewise cannot infer a generic payload as `void`; `void` is not a
valid concrete generic substitution. Only an already concrete expected `void` or `void!` boundary
may consume that completion.

```nct
func decode<T>(bytes: &[u8]): T!

let config: Config = decode(bytes)?
let unknown = decode(bytes)? // error: T cannot be inferred
```

```nct
func inspect<T>(value: T?): void {
    return
}

inspect(42)   // T = i32
inspect(none) // error: T cannot be inferred
```

Forms such as `decode<Config>(bytes)` and `iterator.map<U>(transform)` are not call syntax. Nocter
has no partial type arguments, `_` type-argument placeholders, named type arguments, or default
generic arguments for callables.

## Instances

An `instance` declaration is the type-owned surface for receiver methods, borrow coercions, and the
closed set of source-declarable operators. It does not create a class or introduce inheritance.

```nct
instance WordStats {
    pub method &+self.add_word(): void {
        self.words += 1
    }
}
```

An equality declaration uses the operator expression itself as its signature:

```nct
instance WordStats {
    pub operator (&self == other: &Self): bool {
        return self.words == other.words
    }
}
```

Equality, strict ordering, readonly/readwrite indexing, and readonly/readwrite/owned expansion are
the complete operator families accepted in an `instance`. Their exact declaration shapes and
selection rules are owned by [Operators, Comparison, and Precedence](02-values-types.md#operators-comparison-and-precedence),
[Strict Ordering Operators](24-ordering-operators.md), and
[Expansion Operators](23-expansion-operators.md). Borrow coercion entries are owned by
[Borrow Coercions](22-borrow-coercions.md). These members cannot appear in an interface or
interface implementation, and arbitrary operator spellings are not declarable.

For an ordinary package, the target must be a nominal `struct` or `enum` declared by the same
module; a type alias cannot own an `instance`. The exact active standard-library package may also
provide the compiler-authorized instances for built-in types and views described in
[Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md). Generic instance
parameters are in scope for the target, members, and member bodies.

Functions that directly create the owner belong to its `construct` declaration. A callable without
a receiver that does not construct that owner is an ordinary module function; Nocter has no
qualified top-level function declaration. Construction behavior is specified in
[Construction Surfaces](19-construction-surfaces.md).

## Receivers

Receiver spelling determines call capability:

```nct
method &self.name(...): Return
method &+self.name(...): Return
method self.name(...): Return
```

- `&self` borrows the receiver readonly.
- `&+self` borrows a writable receiver readwrite.
- `self` consumes the receiver or copies it when its type is `Copy`.
- A newly created owned temporary may be a readwrite receiver for its single method call.
- A borrow derived from a temporary receiver cannot escape the statement.

Methods use `value.method(arguments)`. They are not callable through UFCS-like
`Type.method(&value, arguments)` syntax. Associated functions use `Type.function(arguments)` and
cannot be called as value members.

`self` is the fixed receiver binding. `Self` is type-position syntax denoting the current inherent,
interface, or construction owner; it is not resolved as an ordinary identifier.

## Interfaces

An interface is a nominal public capability. Its associated types and methods are explicitly
public. A method without `default` is required and has no body. A method marked `default` is
reusable behavior derived from the same interface contract; its body may be inline or completed by
a private implementation fragment.

```nct
pub interface Counter {
    pub method &+self.next(): i32?

    pub default method self.count(): usize {
        var source = move self
        var total: usize = 0
        loop {
            source.next() otherwise { return total }
            total += 1
        }
    }
}
```

An interface cannot declare fields, stored state, associated data, construction members, or
`drop`. A method without `default` is an interface implementation requirement. A method marked `default` supplies
reusable behavior and may carry its body inline or in a reciprocally seen private source. A
default method does not establish an implementation and cannot access members absent from its
declaring interface contract.

## Associated Types

An interface may declare a required type selected by each explicit implementation:

```nct
pub interface Source {
    pub type Item

    pub method &+self.next(): Self.Item?
}

instance BufferSource<T> {
    impl Source { Item = T }

    method &+self.next(): T? {
        ...
    }
}
```

The associated declaration, binding, and projected-type source forms are defined by
[Interfaces](25-syntactic-grammar.md#interfaces),
[Instance Interface Implementations](25-syntactic-grammar.md#instance-interface-implementations), and
[Types](25-syntactic-grammar.md#types).

Every associated type is required and public. A declaration may require nominal interfaces from
its selected type. One `impl Interface` member binds each declaration
exactly once, cannot bind an undeclared name, and must satisfy every declared requirement. Bindings
omit `pub` because their visibility and identity come from the interface declaration. Associated
type names use a namespace separate from interface method names.

`Self.Name` selects a declaration on the current interface. `T.Name` requires one unambiguous
interface requirement on `T` that declares `Name`. A concrete projection follows one applicable
interface implementation and substitutes its binding. The normalized result participates in method-signature
compatibility, ownership, copyability, sizing, provenance, generic specialization, ABI checking,
and lowering exactly like the bound type written by the implementation.

Associated type declarations cannot currently have defaults or generic parameters. An associated
binding may relate projections selected by independent parameters without introducing a second
associated-type declaration:

```nct
func chain<L, R>(left: L, right: R): ChainIter<L, R>
where L impl Iterator, R impl Iterator { Item = L.Item } {
    ...
}
```

## Static Opaque Results

`some Interface` lets a body-bearing callable expose one interface while keeping its concrete
result type out of the API:

```nct
pub func lines(text: &str): some Iterator { Item = &str } from text {
    return LinesIter.new(text)
}
```

This is static abstraction. It does not create an interface object, vtable, box, runtime type
record, or allocation. The compiler selects one concrete witness from the callable body and uses
that witness for layout, ABI, destruction, and statically dispatched interface calls. Callers see
only the declared interface and its named associated-type bindings.

The `some` result source form is defined by
[Types](25-syntactic-grammar.md#types).

Ordinary interface type arguments precede associated bindings when both are present:

```nct
func values<T>(): some Source<T> { Item = &T } { ... }
```

Rules:

- `some` is contextual at the start of a callable result. It commits that result to the opaque
  form and remains an ordinary value identifier in value position.
- The advertised type must be one accessible nominal interface.
- Every named binding must name one associated type declared by that interface. Duplicate and
  unknown bindings are errors.
- Every reachable value return and the callable body result must select the same concrete witness
  after alias and projection normalization.
- At least one reachable success-producing return or callable body result must select that witness.
  A failure-only, absence-only, or diverging implementation does not provide a layout witness and
  is rejected.
- The witness must explicitly implement the advertised interface, and every named associated
  binding must equal the implementation's selected type.
- The initial form is accepted only as the success payload of a body-bearing function, associated
  function, inherent method, or body-bearing interface default method.
- Parameters, fields, aliases, callable value types, primitives, construction literals, bodyless
  interface requirements, interface implementation methods, coercions, and drop declarations
  cannot introduce opaque types.
- `some Interface?` and `some Interface!` use the ordinary optional and fallible outer layers.
- A `from` clause remains an independent storage-lifetime contract. `some` neither adds nor removes
  an origin.
- Each declaring callable creates a distinct opaque identity. Results from two declarations do not
  become assignment-compatible merely because they use the same interface and witness.
- Generic specialization substitutes the callable's type arguments into the interface bindings and
  witness while preserving the declaration identity.
- An opaque result is move-only at its public boundary. Hidden witness copyability is not an
  advertised capability.
- Member lookup, completion, and source navigation expose only interface members. Witness fields,
  inherent methods, constructors, and concrete type names remain unavailable.

The concrete witness is an implementation fact rather than an inferred spelling of the public
contract. Changing it does not require caller source changes as long as the interface, associated
bindings, provenance, and observable behavior remain valid. Dynamic interface values are not part
of this feature.

## Explicit Interface Implementation

An `impl` member inside an `instance` explicitly implements one nominal interface for that
instance target:

```nct
instance User {
    impl Printable

    method &self.print(): i32 {
        return 0
    }
}
```

The `impl` member owns only the interface application and its associated bindings. Required method
bodies are ordinary inherent methods and may be declared in the same instance fragment or another
applicable fragment:

```nct
instance ValuesIter<T> {
    method &+self.next(): T? {
        ...
    }
}

instance ValuesIter<T> {
    impl Iterator { Item = T }
}
```

Declaration order and physical fragment order do not affect implementation. At checked-program
construction, every required interface method is matched to exactly one applicable inherent method
by normalized name and signature. The resulting interface-method-to-callable mapping is frozen once
and every later generic call and executable specialization consumes that mapping without searching
instance declarations again.

Interface implementation rules are:

- the interface and instance target resolve to exact nominal identities
- the target is a nominal `struct` or `enum`
- every bodyless interface method has exactly one matching inherent method, unless its interface
  supplies a default
- every associated type declaration has exactly one binding and no undeclared binding is present
- receiver capability, generic parameters, parameter and result types, outcome layers, packs, and
  external result provenance participate in signature compatibility
- parameter names do not participate in compatibility
- associated projections are compared after substituting the implementation's bindings
- coercion and overload ranking never make a near match satisfy an interface
- a result provenance implementation may promise a narrower, longer-lived origin set; a concrete
  storage-independent result may omit an interface origin that cannot apply to that result, while
  a storage-carrying result cannot introduce an undeclared origin
- matching inherent members without an explicit `impl Interface` member do not implement it

One inherent method may satisfy compatible requirements from several interfaces. If two applicable
interfaces require incompatible methods with the same name, the type cannot implement both; the
language does not synthesize or rank interface-specific adapters.

An interface implementation inherits the containing instance's target pattern and `where`
requirements. A method used by it must be available for every specialization admitted by that
instance. Overlapping normalized target/interface patterns are rejected rather than ranked. Nocter
does not perform overlap specialization.

Because an interface implementation changes program-wide dispatch and has no private visibility
form, a directory module declares each `impl Interface` fact in `index.nct`. Private implementation
sources may provide the selected inherent method bodies but cannot introduce a new interface
implementation. Single-file mode may declare the complete instance inline.

## Method Lookup

Concrete receiver lookup collects accessible inherent methods, explicitly implemented interface methods, and
applicable interface defaults. The call is valid only when one candidate remains. Declaration and
import order never resolve ambiguity.

For a bounded generic receiver, lookup searches only the parameter's declared capability set. The
call is checked against the specialized interface signature, and every reachable concrete
instantiation must provide an explicit implementation. Lowering statically selects the frozen implementation member
or specialized default body; there is no vtable or runtime interface lookup.

If two bounds declare the requested name, lookup is ambiguous even when their rendered signatures
match. The compiler never falls back to an inherent method merely because it shares the missing
bound method's spelling.

## Callable Annotations

Built-in callable contracts appear as statically witnessed parameter and local annotations and are
invoked with ordinary call syntax. They are not nominal interface requirements. Callable ownership
and repeated-call capability are specified in [Callable Values and
Interface Default Methods](18-callables-default-methods.md).

## Unsupported Composition Syntax

Embedding syntax such as `...Type` and `pub ...Type` is not part of the current language. Nocter
does not provide inheritance, mixins, trait implementation reuse, automatic delegation, or implicit
interface implementation. A future composition proposal must define ownership, initialization,
visibility, collision, partial-move, and interface-implementation interaction before adoption.
