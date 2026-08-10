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

A generic parameter list declares names and arity only. A `where` clause declares every capability,
intrinsic copy requirement, and associated-type equality required by the declaration:

```text
GenericParameters = "<" Name ("," Name)* [","] ">"
WhereClause       = "where" Predicate ("," Predicate)*
Predicate         = Name ":" Capability ("+" Capability)*
                  | "copy" Name
                  | Type "=" Type
Capability        = InterfaceBound | CallableContract
InterfaceBound    = Type
CallableContract  = ["&" ["+"]] "func" "(" CallableParameters ")" ":" Type
```

Every nominal capability must resolve to an accessible interface with the declared type arity. Bound
order is formatting information; semantics use specialized interface declaration identities plus
at most one structural callable contract. Duplicate interface identities and multiple callable
contracts are invalid.

`copy` is an intrinsic requirement, not an interface or a type modifier. A callable may rely on
implicit copies of `T` only when its contract contains `where copy T`. A concrete call satisfies the
requirement only when its substituted type is copyable under the ownership rules.

Callables can further constrain a generic parameter inherited from a surrounding `construct`, `instance`,
`conform`, or `interface` scope:

```nct
construct Buffer<T> {
    pub func from_view(values: &[T]): Self where copy T {
        ...
    }
}
```

The clause follows result provenance and precedes a callable body. On a struct, enum, or interface,
it follows the generic parameter list and precedes the body. On an `instance`, it follows the
target. On a `conform`, it follows the conformance target. On a type alias, it follows the aliased
type. A requirement target must be a generic parameter visible to
that declaration. Duplicate `copy` requirements, duplicate interface bounds, and multiple callable
contracts are invalid. `copy` is unavailable after `:` and is invalid inside a type expression such
as `&[copy T]`.

A general type equality requires at least one associated projection. Equality is symmetric and transitive,
expands aliases, and applies recursively beneath existing type constructors. A generic body may
rely only on equalities in its lexical predicate environment. A concrete call or conditional
conformance must prove every specialized equality. Cycles that cannot normalize to a finite type,
unresolved operands, duplicate predicates, and equalities without a projection are invalid. An
`conform Interface for Type` clause uses the same predicate model and places `where` after the target.

`instance` and `conform` do not have a prefix generic parameter list. Their interface and target
headers are declaration type patterns. Each generic argument slot contains a bare binder name; its
first occurrence declares the binder and later occurrences reuse the same identity:

```nct
instance Pair<L, R> { ... }
conform Comparable<T> for Pair<T, T> { ... }
```

Concrete and nested types do not appear directly in a pattern slot. A declaration introduces a
binder and applies a directed refinement after the header:

```nct
instance Vec<T> where T = i32 { ... }
conform Printable for Pair<L, R> where L = String, R = Vec<String> { ... }
```

In this context, `where T = Type` is a binder refinement rather than symmetric projection
equality. The left operand must be a binder declared by the same pattern, the right operand cannot
contain that binder, and one binder cannot have two refinements. Refinements affect method and
conformance applicability. Overlapping patterns are rejected; a more concrete refinement never
wins by ranking or source order.

`drop` is uniform across every specialization of a nominal type. An instance containing `drop`
must use each target slot through one distinct binder and cannot have a `where` predicate. This
keeps generic ownership and ABI behavior independent of conditional method availability.

```nct
func inspect<T>(value: &T): i32 where T: Readable<i32> {
    return value.read()
}
```

Generic implementation uses monomorphization. Predicate equality and binder refinement are
compile-time only and create
no witness, metadata, dictionary, or ABI field. Nocter does not provide runtime generic metadata,
interface objects, interface inheritance, higher-kinded types, generic associated types, or general
const generics.

## Instances

An `instance` declaration associates receiver methods and `drop` with a nominal type. It does not create a
class or introduce inheritance.

```nct
instance WordStats {
    pub method &+self.add_word(): void {
        self.words += 1
    }
}
```

The target must be a nominal `struct` or `enum`; a type alias cannot own an `instance`. Generic
instance parameters are in scope for the target, members, and member bodies.

Functions that directly create the nominal owner belong to its `construct` declaration. Other
associated functions are qualified top-level declarations. Construction behavior is specified in
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
interface, conformance, or construction owner; it is not resolved as an ordinary identifier.

## Interfaces

An interface is a nominal public capability. Its associated types and methods are explicitly
public. A method without a body is required; a method with a body is reusable default behavior
derived from the same interface contract.

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

An interface cannot declare fields, stored state, associated data, construction members, or
`drop`. A default method does not establish conformance and cannot access members absent from its
declaring interface contract.

## Associated Types

An interface may declare a required type selected by each conformance:

```nct
pub interface Source {
    pub type Item

    pub method &+self.next(): Self.Item?
}

conform Source for BufferSource<T> {
    type Item = T

    method &+self.next(): T? {
        ...
    }
}
```

```text
AssociatedTypeDeclaration = "pub" "type" Name [":" Bound ("+" Bound)*]
AssociatedTypeBinding     = "type" Name "=" Type
ProjectedType             = TypeAtom "." Name
```

Every associated type is required and public. A declaration may require ordinary interface or
callable capabilities from its selected type. A conformance binds each declaration exactly once,
cannot bind an undeclared name, and must satisfy every declared capability. Bindings omit `pub`
because their visibility and identity come from the interface declaration. `instance`
declarations cannot contain associated type bindings. Associated type names use a namespace
separate from interface method names.

`Self.Name` selects a declaration on the current interface. `T.Name` requires one unambiguous
interface requirement on `T` that declares `Name`. A concrete projection follows one applicable
conformance and substitutes its binding. The normalized result participates in method-signature
compatibility, ownership, copyability, sizing, provenance, generic specialization, ABI checking,
and lowering exactly like the bound type written by the implementation.

Associated type declarations cannot currently have defaults or generic parameters. Equality
predicates relate projections selected by independent parameters without introducing a second
associated-type declaration:

```nct
func chain<L, R>(left: L, right: R): ChainIter<L, R>
where L: Iterator, R: Iterator, R.Item = L.Item {
    ...
}
```

## Explicit Conformance

Conformance is declared with a mandatory body-bearing `conform` declaration:

```nct
conform Printable for User {
    method &self.print(): i32 {
        return 0
    }
}
```

The conformance body owns every required member implementation. Members omit `pub` because the
interface declaration owns visibility. A default may be omitted or overridden by a same-name
member. An inherent method never establishes or overrides interface conformance.

Conformance rules are:

- the interface and target resolve to exact nominal identities
- the target is a nominal `struct` or `enum`
- every bodyless interface method has exactly one matching implementation member
- every associated type declaration has exactly one binding and no undeclared binding is present
- extra methods, associated functions, literals, `drop`, and construction members are invalid
- receiver capability, generic parameters, parameter and result types, outcome layers, and external
  result provenance participate in signature compatibility
- parameter names do not participate in compatibility
- associated projections in method signatures are compared after substituting the conformance's
  bindings
- a result provenance implementation may promise a narrower, longer-lived origin set; a concrete
  storage-independent result may omit an interface origin that cannot apply to that result, while
  a storage-carrying result cannot introduce an undeclared origin
- matching members without an explicit conformance declaration do not conform

Generic conformance parameters may carry bounds. A conditional conformance exists for a concrete
target only when every specialized bound is satisfied:

```nct
conform Iterator for TakeIter<I> where I: Iterator {
    type Item = I.Item

    method &+self.next(): I.Item? {
        if self.remaining == 0 {
            return none
        }
        self.remaining -= 1
        return self.source.next()?
    }
}
```

Overlapping normalized target/interface patterns are rejected rather than ranked. Nocter does not
perform overlap specialization.

## Method Lookup

Concrete receiver lookup collects accessible inherent methods, explicit conformance members, and
applicable interface defaults. The call is valid only when one candidate remains. Declaration and
import order never resolve ambiguity.

For a bounded generic receiver, lookup searches only the parameter's declared capability set. The
call is checked against the specialized interface signature, and every reachable concrete
instantiation must provide explicit conformance. Lowering statically selects the conformance member
or specialized default body; there is no vtable or runtime interface lookup.

If two bounds declare the requested name, lookup is ambiguous even when their rendered signatures
match. The compiler never falls back to an inherent method merely because it shares the missing
bound method's spelling.

## Callable Bounds

Built-in callable contracts may appear in a capability set and are invoked with ordinary call
syntax. Callable ownership and repeated-call capability are specified in [Callable Values and
Interface Default Methods](18-callables-default-methods.md).

## Unsupported Composition Syntax

Embedding syntax such as `...Type` and `pub ...Type` is not part of the current language. Nocter
does not provide inheritance, mixins, trait implementation reuse, automatic delegation, or implicit
interface conformance. A future composition proposal must define ownership, initialization,
visibility, collision, partial-move, and conformance interaction before adoption.
