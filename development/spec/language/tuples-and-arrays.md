# Tuples and Fixed Arrays

## Scope and Identity

This chapter owns structural tuple/fixed-array construction, primitive component selection and
array partial initialization for v2.0.0. [Value access](value-access.md) owns value supply,
authority, reservations and cleanup; [patterns](patterns.md) own tuple decomposition. These
adopted contracts do not extend the bounded private compiler. Dynamic sequences, `[T]`, typed
sequence/string literal entries, general user expansion, iteration and target representation require
their own contracts. [Variadic inputs](variadic-inputs.md) separately own built-in fixed-array
argument expansion; it is not an array literal spread or a definition of `[T]`. A recognized
draft production does not import any of the other behaviors.

A tuple has at least two ordered components, whose types can differ. Its identity includes the
ordered component types. A fixed array `[T; N]` has N ordered elements of one storable data type T.
N is a required compile-time target usize under the [constant rules](generics-and-constants.md);
the evaluated length is part of type identity. Equal total size does not identify an array with
a tuple, or arrays with different element types/lengths. Transparent aliases preserve identity.
`never` is not a stored component type, even in an empty array. `void` is ordinary storable data.

Neither structural family has an authored destructor or implicit allocation/indirection API.
Its components retain their own cleanup and dependency schemas. Zero-sized components remain
distinct initialized logical places; layout cannot merge their access/resource identities.
A proven zero-length array contributes no by-value element-containment edge. It can terminate
a nominal containment path, while a positive or unproved length cannot hide an infinite payload
cycle. T must still be a valid declared storable type; an empty count does not excuse an
independently invalid declaration or a cycle needed to compute the count itself.

## Construction and Context

```text
let pair = (1, true)
let values: [i32; 3] = [1, 2, 3]
let empty: [File; 0] = []
let repeated = [7; 3]
```

The File example assumes a declared storable type. Parentheses around one expression group it;
they do not construct a one-element tuple. `()` constructs void. A tuple needs at least two
expressions; a trailing comma is permitted there, but `(value,)` is not another grouping or
tuple form. Empty `[]` constructs a zero-length array only when its element type is uniquely
established; the zero count does not choose an arbitrary element type.

A selected expected tuple/array type supplies component data constraints before checking the
initializer. Without that context, infer each component by its own ordinary rules; an array's
resulting component data types must have exact identity. Do not use the first element to choose
an implicit conversion for subsequent ones, or select a common supertype/optional depth by
search. For example, `[true, none]` lacks a uniquely typed none, whereas an expected `[bool?; 2]`
supplies the optional payload type and permits ordinary injection of true.

An annotation cannot turn construction into read/edit access, choose a pack or search for a user
literal entry. Contextual component construction can use its admitted injections/adapters;
there is no general implicit numeric/user coercion. A missing unique type needs local context
rather than a guess from later uses. The number of listed array elements must equal the expected
length. Repetition uses the same expected/inferred element-data rule for its one producer.

Evaluate listed components once, left to right, supplying each by ordinary Copy-or-move before
evaluating the next. This is value construction, not a bundle of delayed call reservations.
For example, `(file, fail()?)` can already have moved file when the second operand exits. Clean
successfully constructed components in reverse actual construction order on recoverable exit or
cancellation; do not roll back earlier moves or destroy an unfinished aggregate as a whole.
An interrupted component's known shape remains structurally checked without fabricating a value.

On successful construction, components occupy their declaration/index order. Complete and
partially moved aggregate cleanup visits remaining initialized components in reverse order.
Value results are staged before enclosing evaluation-temporary cleanup. A component depending
on a root cleaned at that handoff is invalid; construction cannot retain a hidden owner, clone a
resource or retarget the dependency. Read calls may observe a fresh aggregate within its valid
evaluation extent; scoped local read/edit bindings still require an existing place.

Array and tuple constructors have no spread, record-field shorthand, named components, implicit
padding or container conversion. Joint `value[index]` is selection; spaced `Owner [elements]`
belongs to the separate typed-literal family. An invalid selection cannot be reparsed as a
literal after type lookup. Neither an expected user type nor a missing array type authorizes
that draft family implicitly.

## Repetition and Copy

In `[element; count]`, count is required compile-time data, not a runtime loop operand. Its value
determines the array type under the same evaluator/normalization authority as type lengths.
Evaluate element exactly once, including when count is zero. Zero then cleans that produced value
before delivering the empty array; one transfers it into the only element. More than one requires
source-context Copy evidence for the element and constructs that many structural snapshots of
the evaluated datum. No user cloning hook runs and the producer is not called repeatedly.

A body with a symbolic count needs Copy evidence unless its own declared assumptions and checked
path facts prove the count is zero or one. A later zero/one specialization cannot repair a
generic body lacking that proof. Conversely the optimizer may remove physical repetition when
snapshot and effect semantics remain intact. Very large counts use ordinary type/evaluator/target
resource diagnostics, not a fixed source-language element ceiling or integer wraparound.

A tuple has Copy evidence exactly when every component has it. A fixed array has Copy evidence
when its length is proven zero or its element type has Copy evidence in the checking context.
An exact zero-length type contains no element resources or carried element dependencies; this
does not grant Copy to T or to an unproved symbolic-length array. In particular `[File; 0]` may
be Copy while File is not. `[file; 0]` still evaluates, moves and destroys file under the preceding
rule. A non-Copy read input still cannot construct that seed, even with a zero repetition count.
The empty array's Copy evidence cannot undo those effects.

This length-dependent structural proof differs from an optional/enum's runtime tag: different
fixed lengths are different types, whereas learning an active tag does not change whole-type
Copy policy. A zero-byte element with nonzero count is not an empty array. Its resources,
dependencies and lack of Copy still apply once per initialized logical element.

## Primitive Component Selection

Tuple `.0`, `.1`, and subsequent nonnegative integer selectors resolve fixed positions. A
selector must be in range; it is a static component selection, not a user member lookup or an
implicit conversion from a field name. It uses the same place/value distinction as a record
field. This chapter does not add numeric selectors to records or dynamic indexing to tuples.

Fixed-array selection evaluates the base and the index once, in that order. For an existing
place, identifying the container does not first copy/move or read the complete array; validate
the selected component's initialization separately. A computed base constructs its value under
ordinary rules. Check the index with expected usize data; no signed-to-unsigned cast or user
coercion is inserted. Validate index < N before
access. An out-of-range selection traps without furnishing a place or payload. Required const
evaluation instead reports its evaluated invalid operation as a compilation error. A proof can
remove the physical check, never manufacture access to an invalid/missing element. There is no
unchecked indexing form or implicit recoverable outcome in this selected contract.

Selection from existing tuple/array storage identifies a component place and inherits its
authority and initialization state. Read/edit access never creates an owner. A value-producing
use snapshots a Copy component or moves an eligible owning component under ordinary supply.
An owning fixed-array element is a structural subplace, like an owning tuple/record field; it
is not a user-defined index projection whose receiver only lends an element. Taking a non-Copy
element through a read/edit alias or a user index projection is still invalid. The same applies
when an outside read/edit parameter exposes an array: the descriptor does not own its elements.

Selection from a computed aggregate value instead delivers a component value under the existing
computed-field rule. It cannot initialize a scoped alias or edit input. Its remainder belongs
to the evaluation frame and is cleaned at that frame's defined boundary, skipping the selected
component if transferred. Grouping preserves the category rather than converting an expression
into writable storage. Bind an intentional owner first when later scoped access is needed.

An existing-place array element supplied directly to a read/edit/take call uses the common operand-preparation
protocol: freeze the evaluated selection once, reserve the needed place/transfer, validate the
complete bundle, then activate/commit. Do not move a merely reserved element if a later operand
interrupts preparation. An already executed nested aggregate/value construction still has its
own earlier transfers. Indexing is not permission to change that evaluation order.

## Partial Initialization and Restoration

Moving a non-Copy element from an owning array makes that position unavailable and the whole
array incomplete. Initialized disjoint elements remain usable. A complete array read, snapshot,
transfer or alias requires every element initialized; giving a type annotation cannot hide a
hole. Dropping a present owning array instead cleans only its remaining initialized elements,
in reverse index order, and ends the container even if every element already moved.

```text
var files: [File; 2] = [open(first)?, open(second)?]
let saved = files[0]
files[0] = open(replacement)?
```

These assume an independently declared File/open contract. The replacement constructs its RHS
first. If that computation fails, files still has the previously missing element; cleanup visits
only its initialized remainder. On success, identify/check the destination once, clean any old
initialized value, and install the replacement. A fixed let array may supply elements but cannot
restore them. Moving the entire array removes its element containers too: assigning an element
cannot recreate an absent parent. Ancestor complete-self destructor obligations still apply on
every relevant exit, including failure/cancellation.

A dynamic selected position has semantic identity tied to that one evaluated index and containing
array, not to the index expression's text or its later variable value. Initialization/conflict
proofs may retain that index value and path conditions. A later selection for reading/access
needs proof that its element is initialized; replacement can identify a missing element without
reading it. Distinct expression spellings do not establish disjointness. If a later
index may equal an already moved position, reject that use unless the checked relation rules it
out or a complete replacement restores it. Do not silently clone, turn extraction into a read,
insert an unrequested fallible lookup or rescan an earlier expression.

The checker records conditional initialization and exact cleanup responsibility once. Lowering
may carry the frozen indices/path choices needed to implement remaining-element cleanup, but
cannot maintain a second, independently inferred ownership ledger. Do not impose a blanket
constant-index restriction or an arbitrary array-length cutoff to substitute for that proof.
Loops, reservations, suspension and joins must retain the same element identities and actual
initialized remainder; admitting syntax alone does not establish those analyses.

Unknown overlap conservatively conflicts. Invalidating or transferring a container is forbidden
while a component access/dependent cleanup still demands it. After the last such demand, ordinary
writable authority may replace the parent; physical relocation/reuse remains governed by the
storage proof. Native layout, zero-sized addressing, ABI, evaluator realization and source
conformance for these contracts remain explicit implementation gates.
