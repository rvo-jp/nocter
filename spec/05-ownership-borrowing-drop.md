# Ownership, Borrowing, and Drop

This file is part of the Nocter language specification.
The specification entry point is [README.md](README.md).

## Borrowing

Borrows distinguish readonly access from readwrite access.

```nct
func inspect(file: &File): void {
    ...
}

func write(file: &+File, data: &str): void! {
    ...
}
```

Rules:

- `&T` is a readonly borrow type.
- `&+T` is a readwrite borrow type.
- `&value` creates a readonly borrow.
- `&+value` creates a readwrite borrow.
- `&+value` may be created only from a writable place, such as a `var` binding, a writable field, or an existing `&+T` reborrow.
- Readonly borrows may coexist with other readonly borrows.
- A readwrite borrow is exclusive and cannot coexist with other readonly or readwrite borrows of the same value.
- A value cannot be moved while it is borrowed.
- A value cannot be explicitly dropped while it is borrowed.
- `let _ = expression` consumes and drops the resulting owned value without creating a binding.
  Existing move-only bindings require `let _ = move name`; newly produced owned temporaries do not
  require `move`.
- A borrow cannot outlive the value it refers to.
- A borrow of a stack value cannot escape the stack value's scope.
- A borrow of region-allocated memory cannot escape that region.
- Ordinary function calls require explicit borrow syntax at the call site.
- Method receivers may create the required borrow automatically.
- Lifetime annotations are not supported.
- `&+` is a single lexical token.
- Unary `+x` is not part of the language. This avoids ambiguity with `&+x`.

Examples:

```nct
var file = File.open(path)?

let a = &file
let b = &file       // OK: multiple readonly borrows
let c = &+file      // error: a and b are used below

inspect(a)
inspect(b)
```

```nct
var file = File.open(path)?

let w = &+file
drop file           // error: w is used below

write_more(w)
```

Ordinary function calls are explicit:

```nct
func inspect(file: &File): void {
    ...
}

inspect(&file)
```

Method receiver borrows are automatic:

```nct
instance File {
    pub method &+self.write_text(text: &str): void! {
        ...
    }
}

file.write_text("hello")?
```

The method call above creates a temporary readwrite borrow of `file` for the call. This does not enable UFCS-style calls:

```nct
File.write_text(&+file, "hello") // error
```

A newly created owned temporary may be used as a readwrite receiver for one method call:

```nct
(File.open(path)?).write_text("hello")?
```

The temporary receiver is dropped according to the statement-end temporary rules in [Control Flow](03-control-flow.md#evaluation-order-and-temporaries).

## Borrow Checker

Nocter uses source-level non-lexical borrow ranges.

A borrow is active from the expression that creates it through the last source-level use of the borrow-like value derived from it. The borrow may end before the lexical scope of the borrow binding ends if the binding is not used again.

```nct
let read = &file
inspect(read)

let write = &+file // OK: read is not used after inspect(read)
```

```nct
let read = &file
let write = &+file // error: read is used below

inspect(read)
```

Rules:

- Source-level lifetime annotations are not supported.
- The compiler determines borrow live ranges from actual source uses, not only from lexical scopes.
- A use includes passing the borrow-like value to a call, method call, field access, index access, return, assignment, initialization, storing it inside an aggregate, or deriving another borrow-like value from it.
- Scope end of a plain borrow-like value does not by itself extend the borrow, because borrow-like values are non-owning and do not run `drop`.
- A readonly borrow may overlap with other readonly borrows of the same place.
- A readwrite borrow must not overlap with any other readonly or readwrite borrow of the same place.
- Borrow overlap is determined from semantic places and projections, not from runtime numeric
  addresses. Distinct zero-sized places remain distinct for ownership and exclusivity even when
  their borrows use the same machine address.
- `move place`, `drop name`, whole-place assignment, field assignment, and reinitialization are invalid when they conflict with an active borrow.
- A borrow cannot outlive the storage it refers to.
- A borrow cannot outlive a region from which its storage was allocated.
- A borrow or borrow-like value derived from a temporary owned value cannot escape the statement that owns the temporary.
- A borrow-like result discarded by `let _ = expression` does not extend the source borrow beyond
  the discard statement.
- A borrow held only by a temporary in an ordinary `if` or `while` boolean condition ends when the
  condition temporary is dropped, before the selected body begins. Retaining the borrow through
  the body requires an explicit longer-lived binding.
- Method receiver borrows last only for the call unless the method returns a borrow-like value derived from the receiver.
- If a method returns a borrow-like value derived from the receiver, the receiver borrow remains active for the returned value's live range.
- `&str`, `&[T]`, `&+[T]`, `ViewIter<T>`, and aggregates containing borrow-like values participate in the same live-range and provenance checks.
- Payload borrows introduced by `match` and `if expr is Pattern` are projections of the pattern
  target borrow. They retain its readonly or readwrite capability and provenance through the last
  use of any derived payload borrow.
- Borrow-like return values are governed by [Borrow-like Return Values](#borrow-like-return-values).

### Field-Sensitive Borrows

Nocter tracks disjoint named struct fields for simple places.

Field-sensitive tracking applies only to direct named field paths whose base is a local binding, parameter binding, or borrow binding that the compiler can resolve statically.

```nct
var user = User {
    name: String.copy("alice"),
    count: 0,
}

let name = &user.name
user.count += 1 // OK: count is disjoint from name
inspect(name)
```

Rules:

- A borrow of a whole value conflicts with borrows and mutations of any field of that value.
- A borrow of one named field does not conflict with mutation or borrowing of a disjoint named field.
- Moving, dropping, reinitializing, or assigning the whole parent value conflicts with any active field borrow.
- Assigning a field conflicts with active borrows of that same field and active borrows of the whole parent value.
- Field-sensitive tracking does not apply to array indexes, collection indexes, `&[T]` or `&+[T]` elements, pointer dereferences, method-call results, enum payloads, or computed projections.
- If the compiler cannot prove two places are disjoint, it treats them as conflicting.

## Function Parameters

Parameters are immutable bindings inside the function body.

```nct
func create(name: String, count: i32, out: &+File): User! {
    out.write_text(&name as &str)?

    return User {
        name: move name,
        count: count,
    }
}
```

Rules:

- Parameters are immutable bindings.
- Parameter bindings cannot be reassigned.
- Mutable parameter bindings are not supported.
- Parameter names must be unique within the parameter list.
- Parameter shadowing is not allowed, following the normal name-resolution rules.
- An owned parameter is owned by the function body.
- A move-only owned parameter is dropped at function scope end unless it is moved.
- Moving a move-only parameter requires `move parameter`.
- After a move-only parameter is moved, that parameter binding is no longer valid.
- A copy parameter may be copied by ordinary use.
- `&T` parameters are readonly borrow bindings.
- `&+T` parameters are readwrite borrow bindings.
- A borrow parameter does not own the referenced value and does not drop it at function scope end.
- The `&+T` parameter binding itself cannot be reassigned, but the referenced value may be mutated through it.
- Method receivers are explicit parameters and follow the same binding, ownership, and borrow rules.
- Default parameters and named parameters are not supported.

Examples:

```nct
func rename(user: &+User, name: String): void {
    user.name = move name
}
```

```nct
func invalid_reassign(name: String): void {
    name = String.empty() // error: parameters are immutable bindings
}
```

```nct
func normalize(value: i32): i32 {
    var current = value

    if current < 0 {
        current = -current
    }

    return current
}
```

```nct
func increment(value: &+Counter): void {
    value.count += 1 // OK: mutates the referenced Counter
}

func invalid_rebind(value: &+Counter, other: &+Counter): void {
    value = other // error: parameter binding is immutable
}
```

## Drop

Resource destruction uses an independent top-level `drop` declaration, not an `instance` method or
a `Drop` interface.

```nct
drop File(&+self) {
    close(self)
    return
}
```

`drop` is an identifier token. The parser recognizes it contextually at the start of a top-level
drop declaration and at the start of an explicit `drop value` statement. Outside those two source
forms, `drop` is an ordinary identifier.

The declaration and statement source forms are defined by
[Drop and Test Declarations](25-syntactic-grammar.md#drop-and-test-declarations) and the common
[statement grammar](25-syntactic-grammar.md#statements).

A declaration such as `func drop(...)` or `method &self.drop(...)` declares an ordinary function or method named `drop`. It does not define destruction behavior.

Rules:

- A nominal type family may define at most one drop declaration.
- A drop declaration has the source form `drop TypePattern(&+self) { ... }`.
- Its target must be an ordinary `struct` or a payload-bearing `enum` declared in the same module.
  Type aliases, `copy struct` families, payloadless enums, and non-nominal types cannot own a drop
  declaration.
- `self` is the fixed drop receiver name and is scoped to the drop body.
- The drop receiver type is always exactly `&+Self`.
- A drop declaration is top-level. `instance` contains inherent methods, and `conform` contains
  interface members. A drop declaration cannot appear inside either body.
- A drop declaration has no visibility, target directive, generic-prefix, `where`, or return-type
  annotation.
- Its target pattern must cover every generic slot exactly once with a distinct binder.
- The declaration applies uniformly to every specialization of its nominal type family.
- Every eligible target family is move-only by declaration. Copyability is determined from the
  type declaration and substituted fields, never changed by the presence or absence of a drop
  declaration.
- A drop body always returns no value and cannot be fallible.
- A drop declaration cannot be called as a normal construction function or method.
- `file.drop()` is an ordinary method call if an ordinary method named `drop` exists; it does not
  invoke the drop declaration.
- `File.drop(&+file)` is an ordinary construction-function call only if `construct File` declares a
  construction function named `drop`; it does not invoke the drop declaration.
- A drop body cannot report cleanup failure through a fallible return.
- If an operation inside `drop` can fail, the `drop` body must ignore that failure, record it in already-owned state before destruction, or terminate with `trap` / `abort`.
- Terminating with `trap` or `abort` from inside `drop` does not unwind remaining caller scopes.
- Owned values are automatically dropped at scope end.
- Initialized owned values are dropped in reverse declaration order.
- Struct drop glue invokes the struct's own drop declaration first when present, then drops owned
  fields in reverse declaration order. Payload-bearing enum drop glue invokes the enum's own drop
  declaration first when present, then drops only the active variant payload in reverse payload
  declaration order. Field and payload drop glue follows the same rule recursively.
- Consuming enum patterns preserve this ordering through their dedicated rule in
  [Enums and Variant Construction](02-values-types.md#enums-and-variant-construction): a drop body
  that would otherwise receive partial storage runs once before a named move-only payload leaves,
  while later residual cleanup drops only the still-initialized payload fields.
- Maybe initialized owned values use compiler-generated conditional drop.
- Uninitialized bindings are not dropped.
- `return` and postfix `?` propagation run the same scope-end drop behavior, including conditional drop.
- A moved value is not dropped through the original binding.

Valid move-only type:

```nct
struct File {
    handle: usize
}

drop File(&+self) {
    close(self.handle)
    return
}
```

A generic drop declaration covers the complete family and cannot be refined:

```nct
struct Buffer<T> {
    value: T
}

drop Buffer<T>(&+self) {
    release(self)
    return
}
```

Copyable targets are invalid, including an enum whose variants all carry no payload:

```nct
copy struct Point {
    x: i32
    y: i32
}

drop Point(&+self) { // error: Point is copyable
    return
}

enum Token {
    only
}

drop Token(&+self) { // error: Token is copyable
    return
}
```

A generic `copy struct` cannot own a drop declaration. The compiler does not search substitutions
or turn only its non-copy specializations into separately destructible types:

```nct
copy struct Box<T> {
    value: T
}

drop Box<T>(&+self) { // error: copy struct families cannot own drop declarations
    return
}
```

### Explicit Drop Statements

Explicit early destruction uses a `drop` statement.

```nct
var file = File.open(path)?
drop file
```

After `drop file`, the binding enters an uninitialized state.

```nct
file.read() // error
```

Rules:

- `drop name` is a statement.
- The operand of `drop` must be a local binding name or parameter binding name.
- `drop` is not a reserved keyword and is not an ordinary function call in this statement form.
- The operand must be initialized.
- The operand must be a move-only owned binding.
- Copy types cannot be explicitly dropped.
- Borrow bindings such as `&T` and `&+T` cannot be explicitly dropped because they do not own the referenced value.
- Maybe initialized bindings cannot be explicitly dropped.
- Uninitialized bindings cannot be explicitly dropped.
- A binding cannot be explicitly dropped while it is borrowed.
- `drop name` runs the same drop glue as scope-end automatic drop.
- `drop` is not fallible.
- `drop` produces no value.
- After `drop name`, the binding is uninitialized on all later reachable paths.
- A dropped `var` binding may be reinitialized by assigning to the whole binding. The detailed rules are specified in [Values and Types](02-values-types.md#reinitialization-after-move-or-drop).
- A dropped `let` binding cannot be reinitialized.
- `drop object.field`, `drop array[index]`, and `drop make_value()` are invalid.

Examples:

```nct
var file = File.open(path)?
drop file

file = File.open(other)?
file.read()?
```

Invalid:

```nct
drop count
drop ref
drop object.field
drop array[index]
drop make_value()
```

## Copy and Move

Types are move-only by default. Only copy types may be copied implicitly.

Copyable structs are declared with `copy struct`.

```nct
copy struct Point {
    pub x: i32
    pub y: i32
}
```

Rules:

- Types are move-only by default.
- `copy struct` opts a nominal family into structural copyability. An ordinary `struct` remains
  move-only even when all its fields happen to be copyable.
- A non-generic `copy struct` is copyable only when every field is copyable; a declaration with an
  unconditionally move-only field is invalid.
- A generic `copy struct` derives one copy condition from its fields. A field whose copyability
  depends on a generic parameter contributes that dependency. A field that remains move-only for
  every substitution makes the declaration invalid rather than defining a misleading never-copy
  `copy struct` family.
- After concrete substitution, a valid generic `copy struct` specialization is copyable exactly
  when every substituted field type is copyable. For example, `copy struct Box<T> { value: T }` is
  copyable as `Box<i32>` but move-only as `Box<String>`, while a field such as `&T` remains copyable
  for every `T`.
- No copyable type can own or acquire a drop declaration. This includes primitive numeric types,
  `bool`, raw pointers, payloadless enums, copyable fixed arrays,
  copyable `copy struct` specializations, copyable borrows, and aliases to any of these types.
- Because one drop declaration covers a complete nominal type family, every `copy struct` family is
  ineligible even when a particular specialization is move-only. A drop declaration never changes
  a type from copyable to move-only.
- A copyable `copy struct` specialization cannot own a field that requires destruction. A
  conditional specialization such as `Box<String>` is move-only and runs ordinary structural field
  cleanup; the `copy struct` family itself still cannot declare a type-owned drop body.
- Primitive numeric types, `bool`, and raw pointers are copyable.
- The built-in `error` type is move-only and has compiler-defined destruction.
- Payloadless enum values are copyable.
- Fixed-size arrays `[T; N]` are copyable when `T` is copyable.
- An optional `T?` is copyable exactly when `T` is copyable.
- Every fallible `T!` is move-only, regardless of its success payload. The special
  payloadless-success type `void!` is also move-only.
- Supported mixed outcomes containing a fallible layer are move-only. An optional `T?` remains
  copyable exactly when `T` is copyable.
- Copyability is a property of the complete outcome type, not its currently active tag. An absent
  `String?` and a failed `String!` remain move-only because another value of the same type may own a
  `String` payload.
- A closure's anonymous environment is copyable exactly when every stored capture is copyable. A
  capture-free closure and a closure containing only readonly-borrow captures are copyable. A
  closure containing a readwrite-borrow capture or move-only owned capture is move-only.
- Callable capability and closure copyability are independent. `&func`, `&+func`, and `func`
  describe how a closure may be invoked; they never change whether its environment may be copied.
- Type aliases to copy types are copyable. For example, a project-local alias to `i32` is copyable.
- `&T` is copyable.
- `&+T` is not copyable.
- A generic parameter is treated as potentially move-only unless its declaration or enclosing
  callable contract requires `copy`.
- `copy T` permits ordinary copy operations in the generic body and is checked again against every
  concrete substitution. It adds no runtime metadata and cannot be implemented by user code.
- Non-copy values are not implicitly moved by assignment, argument passing, or return.
- Moving a non-copy value requires explicit `move`.
- Direct collection iteration follows the same rule. An existing move-only iterator binding must
  be written as `for item in move iterator`; only a newly produced iterator temporary or a copyable
  iterator may appear as a bare direct-iterator source.

Examples:

```nct
let p1 = Point { x: 1, y: 2 }
let p2 = p1 // OK: Point is copy

let text1 = String.new()
let text2 = text1      // error: String is not copy
let text3 = move text1 // OK
```

Generic-dependent copyability is valid:

```nct
copy struct Box<T> {
    value: T
}

let number_box: Box<i32> = Box { value: 1 }       // copyable
let text_box: Box<String> = Box { value: text }   // move-only
```

A field that is unconditionally move-only makes the declaration invalid:

```nct
copy struct Invalid<T> {
    text: String // error: move-only for every T
    marker: &T
}
```

Copyable optionals copy like other copy types:

```nct
let maybe: i32? = find_count()
let copied = maybe
inspect(maybe) // valid: maybe remains initialized

```

An optional that can contain a move-only payload is move-only even when absent. Every fallible
outcome is move-only because failure owns an `error`:

```nct
let maybe_name: String? = find_name()
let copied_name = maybe_name // error: String? is move-only
let moved_name = move maybe_name

let read_name: String! = load_name()
let copied_read = read_name // error: String! is move-only
let moved_read = move read_name

let completion: void! = flush()
let copied_completion = completion // error: void! is move-only
let moved_completion = move completion
```

Function calls follow the same rule.

```nct
func consume(text: String): void {
    ...
}

let text = String.new()
consume(text)      // error
consume(move text) // OK
```

Static opaque results written as `some Interface` are move-only at the public boundary. A caller
cannot infer copyability from the hidden witness. Explicit `move` transfers an existing opaque
binding, and normal scope, return, optional, fallible, and path-sensitive cleanup destroys the
hidden value exactly once.

## Move Expressions

`move` is a unary expression that explicitly transfers ownership from a binding or named struct
field.

```nct
let b = move a
consume(move text)
consume(move pair.second)
return move value
let item = move maybe?
```

Move syntax uses the place-specific production under
[Expression Precedence](25-syntactic-grammar.md#expression-precedence).

An outcome suffix is applied to the completed move expression. Therefore `move maybe?` is
equivalent to `(move maybe)?`, not `move (maybe?)`.

Rules:

- `move` is a reserved keyword, not an ordinary function.
- `move place` is an expression.
- The operand of `move` must be a local binding, parameter binding, or a named struct field rooted
  in one of those bindings.
- The root must own the selected storage: an owned local, owned parameter, or owned closure capture.
  A readonly or readwrite borrow binding, borrowed closure capture, or field path that crosses a
  borrow cannot be a move root. Readwrite permission permits mutation, not extraction of the
  caller's ownership.
- The operand binding may be immutable or mutable.
- The selected place must have a move-only type.
- Using `move` on a copy type is a compile error.
- `move place` has the same type as the selected place.
- Evaluating `move place` transfers ownership out of that place.
- `move place?` and `move place!` first transfer the complete optional or fallible value and then
  apply their one outcome suffix. `move place catch ...` and
  `move place otherwise ...` likewise move the place before the lower-precedence eliminator.
- A second outcome layer requires a new expression boundary, as in `(move result?)?`. The
  parentheses are semantically relevant grammar and are not removed as redundant grouping.
- After a whole-binding move, the binding is uninitialized on all later reachable paths. After a
  named-field move, that field is uninitialized and the parent is partially initialized; disjoint
  initialized fields remain usable and retain their own cleanup obligations.
- A moved `var` binding may be reinitialized by assigning to the whole binding. The detailed rules are specified in [Values and Types](02-values-types.md#reinitialization-after-move-or-drop).
- A moved `let` binding cannot be reinitialized.
- Moved storage is not dropped through its original place.
- A place cannot be moved while it conflicts with an active borrow.
- `move` describes ownership transfer. It does not specify whether generated code copies bytes, passes a pointer, or elides a copy.
- Moving a newly constructed value is unnecessary and invalid.
- This restriction also applies under collection iteration and sequence spread. `move` in
  `for item in move source` and `...move source` is the ordinary place-only move expression, not a
  contextual ownership selector for a newly produced value.
- Moving from an index, dereference, call result, postfix `?` expression, conditional expression,
  or parenthesized complex expression is invalid.
- The valid `move place?` spelling does not contradict that restriction: the move operand is the
  place, while `?` applies afterward. `move (place?)` still attempts to move a computed result and
  is invalid.
- Partial moves are field-sensitive only for statically named struct fields. Index moves from
  arrays or collections, enum-payload moves, and computed projections are not supported.
- A named-field move is invalid when any proper-prefix struct made partially initialized by that
  move has its own drop declaration. Drop bodies always receive one complete initialized `Self`;
  the compiler neither calls one on a partial value nor silently omits it.
- `match move place` and `if move place is Pattern` move the complete enum place before selecting a
  branch. This is not a partial enum-payload move; branch payload bindings receive ownership from
  the already consumed enum according to the enum pattern rules.

Valid:

```nct
let b = move a
consume(move text)
consume(move pair.second)
return move value
user.name = move name
let item = move maybe?
```

Invalid:

```nct
move make_value()
move (make_value()?)
move (maybe?)
move (condition ? a : b)
move array[index]
move copy_value
```

Assignment may replace a live move-only field when the right-hand side produces
a complete replacement value. The replacement is evaluated first, then the old
field is dropped, and ownership of the replacement transfers into the field.
Moving a named field records that field as separately dead. Automatic cleanup destroys only the
remaining initialized fields, in their ordinary reverse declaration order. The whole parent
cannot be used or moved while it remains partially initialized. A mutable parent may restore a
moved field through field assignment before a later whole-value use.

Control-flow joins merge named-field state independently. A field initialized on every incoming
path is initialized, a field uninitialized on every incoming path is uninitialized, and a mixture
is maybe initialized. Assignment to an uninitialized or maybe initialized field of a `var` parent
restores that field through the common no-drop or conditional-drop assignment rule. Scope exit and
whole-parent replacement likewise drop only live fields, using conditional drop for maybe
initialized fields.

This partial state exists only for structs without their own drop declaration. A field whose type
has a drop declaration may still be moved as one complete field; its drop obligation transfers to
the destination. The prohibition concerns each enclosing struct that would otherwise require a
drop body to observe an incomplete `Self`.

```nct
func rename(user: User, name: String): User {
    var next = move user
    next.name = move name
    return move next
}
```

Invalid partial move through a type-owned drop contract:

```nct
struct Session {
    socket: Socket
    label: String
}

drop Session(&+self) {
    record_session_end(self)
    return
}

let session = open_session()
let socket = move session.socket // error: Session owns a drop declaration
```

Conditional field restoration:

```nct
var user = make_user()

if condition {
    consume(move user.name)
}

user.name = String.copy("replacement")
use(user) // valid: every field is initialized again
```

## Return Values

Returning an existing move-only place requires explicit `move`.

Rules:

- `return value` may return a copy value by copying it.
- `return place` is invalid when `place` is an existing move-only binding or named field.
- `return move place` returns an existing move-only binding or eligible named struct field by
  moving the common `MovePlace` defined above.
- `return move place` transfers the selected place before return cleanup. A whole binding becomes
  uninitialized; a named field leaves its eligible parent partial, and return cleanup drops only
  the remaining initialized fields.
- A newly constructed owned value may be returned with `return expr` without `move`.
- Newly constructed owned values include struct literals, enum variant constructors, array literals, and function or method call results.
- `return` evaluates the returned expression first.
- When control leaves through `return`, the returned value is not dropped by the callee.
- Other live local owned values are dropped in reverse declaration order.
- Moved bindings are not dropped.
- Copy parameters may be returned with `return parameter`.
- Move-only owned parameters require `return move parameter`.
- Optional and fallible return layers are constructed by
  [recursive outcome injection](04-errors-optionals.md#recursive-outcome-injection).
- Injection transfers or copies the recursively accepted value into the active payload exactly
  once. It does not copy a move-only value or relax the explicit-`move` requirement.
- Cleanup owns only the active payload of the constructed outcome. A returned active payload is not
  also dropped as a callee local.
- Bare `return` is valid for `void` and for successful completion of `void!`.

Examples:

```nct
func make_text(): String {
    let text = String.new()
    return move text
}
```

```nct
func make_user(name: String): User {
    return User {
        name: move name,
    }
}
```

```nct
func take_user(user: User): User {
    return move user
}
```

```nct
func invalid(user: User): User {
    return user // error: User is move-only
}
```

### Borrow-like Return Values

Borrow-like return values are allowed only when the compiler can prove the referenced storage lives after the function returns.

Borrow-like return values include:

- `&T`
- `&+T`
- `&str`
- `&[T]`
- `&+[T]`
- `ViewIter<T>`
- structs, enums, optionals, fallible values, and arrays containing borrow-like values

The detailed provenance source kinds are specified in [Strings, Arrays, Views, and Pointers](07-strings-arrays-views-pointers.md#borrow-like-provenance).

Rules:

- Borrow-like return values must carry provenance to storage that outlives the function call.
- Borrow-like values derived from static storage, such as string literals, may be returned.
- Borrow-like values derived from input borrow-like parameters may be returned when the return value's provenance is still tied to that input borrow-like value.
- A readonly borrow-like value may be returned from a readonly or readwrite input borrow-like source.
- A readwrite borrow-like value may be returned only from a readwrite input borrow-like source, such as `&+T` or `&+[T]`.
- Borrow-like values derived from local owned values cannot be returned.
- Borrow-like values derived from temporary owned values cannot be returned.
- Borrow-like values derived from owned parameters cannot be returned, because owned parameters are dropped at function scope end unless moved.
- Borrow-like values derived from region-allocated storage cannot escape the region.
- Nocter has no source-level lifetime parameters or lifetime annotations.
- If provenance cannot be proven by the compiler, returning the borrow-like value is a compile error.

Examples:

```nct
func greeting(): &str {
    return "hello" // OK: string literal storage is static
}
```

```nct
func first_byte(bytes: &[u8]): u8? {
    if bytes.len() == 0 {
        return none
    }

    return bytes[0] // OK: u8 is copy
}
```

```nct
func bad(): &str {
    var text = String.copy("hello")
    return &text as &str // error: view points to local owned value
}
```

```nct
func also_bad(text: String): &str {
    return &text as &str // error: view points to an owned parameter dropped at return
}
```
