# Bindings and Initialization

This chapter defines local binding mutability, assignment, initialization state, and
reinitialization after ownership transfer or explicit drop.

## Bindings and Mutability

Bindings are immutable by default.

```nct
let count = 0
```

Mutable bindings use `var`.

```nct
var count = 0
count += 1
```

Local bindings must be initialized.

```nct
let path = "input.txt"
var count: i32 = 0

let missing: i32 // error
var later: File  // error
```

Use a discard initializer when an evaluated value is intentionally ignored:

```nct
let _ = String.copy("unused")
let _ = try_operation()
```

Assignment updates a writable place.

```nct
var file = File.open(path)?
file = File.open(other_path)?
```

Borrowing rules are specified in [Ownership, Borrowing, and Drop](ownership.md).

Rules:

- `let` creates an immutable binding.
- `var` creates a mutable binding.
- `let _ = expression` is a discard initializer. It evaluates `expression` but creates no binding.
- A discard initializer accepts an expression of any type. It is the only source form that may
  intentionally discard a non-`void`, non-`never` body value.
- The discarded value is consumed and any owned content is dropped at the end of the discard
  statement. A borrow-like value requires no drop and its borrow ends according to normal
  statement-end liveness.
- Discarding `T?`, `T!`, `T?!`, or `(T!)?` does not unwrap, recover, or propagate it. The complete
  outcome value, including any active success, absence, or failure payload, is intentionally
  discarded and its owned content is dropped.
- Discarding an existing move-only binding still requires `move`, as in `let _ = move value`.
- `_` in a discard initializer cannot have a type annotation, cannot be referenced, and cannot be
  used with `var`.
- Local `let` and `var` bindings require an initializer.
- Uninitialized local declarations are not supported.
- `let` bindings cannot be reassigned.
- `var` bindings may be reassigned.
- After `move name`, the binding enters an uninitialized state.
- After `drop name`, the binding enters an uninitialized state.
- A moved or explicitly dropped `let` binding cannot be reinitialized.
- A moved or explicitly dropped `var` binding may be reinitialized by assigning to the whole binding.
- Reinitializing a moved or explicitly dropped `var` binding does not drop an old value.
- If the right-hand side of a reinitialization fails through postfix `?`, the binding remains uninitialized.
- An uninitialized binding cannot be read, borrowed, dropped, assigned through a field, or used for field access.
- Uninitialized bindings are not dropped at scope end.
- A maybe initialized binding cannot be read, borrowed, moved, explicitly dropped, assigned through a field, or used for field access.
- At scope end, maybe initialized bindings use conditional drop.
- To use a binding after a branch, every reachable path to that use must leave the binding initialized.
- Reinitializing only a field of an uninitialized binding is not supported.
- Statically named fields of a partially initialized struct independently carry initialized,
  uninitialized, or maybe initialized state. A field state becomes maybe initialized when control
  flow merges initialized and uninitialized incoming states.
- An initialized named field may be read or moved. An uninitialized or maybe initialized field may
  not be read, borrowed, moved, explicitly dropped, or used by compound assignment.
- Disjoint definitely initialized fields remain usable while another field is uninitialized or
  maybe initialized. The complete parent may be read, borrowed, moved, or passed only after every
  field is definitely initialized.
- Assignment is a statement, not an expression.
- Assignment target must be a writable place.
- Writable places are `var` bindings, fields reachable through writable
  places, fields reachable through `&+T` borrow bindings or parameters,
  elements of fixed-size arrays reached through writable places, elements of
  `&+[T]` readwrite slices, elements selected by a readwrite index declaration,
  and elements reached through one selected coercion to either kind of
  readwrite index operation.
- `let` bindings are not writable places.
- Fields reached through `&T` are not writable places.
- Elements reached through `&[T]` are not writable places.
- Built-in index assignment applies to fixed-size arrays and `&+[T]` slices. A
  nominal collection becomes a writable index place through an accessible
  `operator (&+self[index: K]): &+V` declaration or one accessible coercion to
  a readwrite index operation.
- Assignment to a place that conflicts with an active borrow is an error. The field-sensitive conflict rules are specified in [Ownership, Borrowing, and Drop](ownership.md#field-sensitive-borrows).
- Field assignment stores a complete field value. It either overwrites an initialized field or
  restores an uninitialized or maybe initialized field; it never creates a new partial state.
- For assignment, the complete right-hand side is evaluated first. After it succeeds, dynamic
  target-place components are evaluated exactly once. An initialized old value is dropped, a maybe
  initialized old value is conditionally dropped, and an uninitialized place performs no old-value
  drop. The new value is then stored and the target place becomes initialized.
- If right-hand-side evaluation propagates or terminates, the target expression is not evaluated
  and no assignment drop or store occurs. Side effects already performed by the right-hand side
  remain. Normal scope-end cleanup still applies to recoverable propagation.
- Whole-binding assignment to a maybe initialized `var` binding is allowed. If the right-hand side succeeds, the compiler conditionally drops the old value if it is initialized, then stores the new value.
- Named-field assignment to an uninitialized or maybe initialized field of a writable partial
  `var` parent is allowed when every proper-prefix field needed to reach it exists. On success that
  field becomes definitely initialized, so the complete parent becomes initialized once all fields
  are initialized.
- Whole-binding assignment over a partial `var` parent drops every remaining initialized field in
  reverse declaration order, conditionally drops each maybe initialized field, and then stores the
  complete replacement. Such a parent cannot own a drop declaration because its earlier partial
  move would have been rejected.
- Assigning an existing non-copy value requires explicit `move`.
- Assigning a copy value copies it.
- Field assignment follows the same ownership and borrow rules as local reassignment.
- Assignment itself produces no value.
- Chained assignment such as `a = b = c` is not supported.
- Compound assignment operators are `+=`, `-=`, `*=`, `/=`, and `%=`. They are allowed only for
  numeric writable places and require a right-hand side of the same numeric type.
- A compound assignment evaluates the complete right-hand side first. If that evaluation
  propagates or terminates, the target expression is not evaluated and no compound write occurs;
  side effects already performed by the right-hand side remain.
- After the right-hand side succeeds, dynamic target-place components such as an index expression
  or source-defined readwrite index operation are evaluated exactly once. The current target value
  is then read, the corresponding checked numeric operation is performed, and the result is stored.
- Compound assignment uses the same overflow, division, remainder, and writable-place rules as the
  corresponding ordinary numeric operation and assignment. It is not a textual desugaring to
  `target = target operator rhs`, because that would duplicate or reorder target evaluation.
- Compound assignment follows the same borrow-conflict rules as assignment at each evaluation
  point.
- Compound assignment requires a definitely initialized target because it reads the old value
  before writing the result. It cannot restore an uninitialized or maybe initialized field.

Examples:

```nct
let count = 0
count = 1 // error: let binding

var total = 0
total = 1 // OK
```

```nct
var a = File.open(path_a)?
var b = File.open(path_b)?

a = b      // error: File is not copy
a = move b // OK; b is no longer valid
```

```nct
var stats = WordStats.empty()
stats.bytes = 10
stats.lines += 1
```

If an owned field is overwritten, the old field value is dropped after the new value has been successfully evaluated.

```nct
var user = move old_user
user.name = move new_name
```

The field assignment above means:

1. Evaluate `move new_name`.
2. If evaluation succeeds, drop the old value in `user.name`.
3. Store the new value into `user.name`.
4. Mark `new_name` invalid.

If step 1 fails because the right-hand side contains postfix `?`, `user.name` is not changed.

### Reinitialization After Move Or Drop

Whole `var` bindings may be reinitialized after a whole-value `move` or explicit `drop`. A writable
named field may be restored after an eligible field move without allowing field-by-field
construction of a never-initialized parent.

```nct
var text = String.new()
consume(move text)

text = String.new() // OK: reinitializes the var binding
consume(move text)
```

Rules:

- Reinitialization is assignment to a whole `var` binding that is currently uninitialized because it was moved or explicitly dropped.
- Reinitialization is not reassignment over a live value, so no old value is dropped.
- If reinitialization succeeds, the binding becomes initialized again.
- If reinitialization fails through postfix `?`, the binding remains uninitialized.
- `let` bindings cannot be reinitialized after move or explicit drop.
- Field reinitialization after moving a whole binding is not supported.
- Assignment may restore an uninitialized or maybe initialized field only when the parent began as
  a fully initialized value and became partial through named-field move. The right-hand side and
  conditional old-value drop follow the common assignment order above.
- Struct construction and whole-binding reinitialization do not accept field-by-field partial
  initialization. A fully initialized struct may enter the compiler-tracked partial state only
  after an eligible named-field move, under the restrictions in
  [Move Expressions](ownership.md#move-expressions).
- At scope end, uninitialized bindings are skipped, initialized bindings use ordinary drop,
  maybe initialized bindings use conditional drop, and partial structs apply those states to each
  named field in reverse declaration order.
- Definite initialization is checked across control flow.

Examples:

```nct
var file = File.open(path)?
close(move file)

file.read() // error: file is uninitialized

file = File.open(other_path)?
file.read()?
```

```nct
var text = String.new()

if condition {
    consume(move text)
    text = String.new()
}

consume(move text) // OK: both paths leave text initialized
```

```nct
var text = String.new()

if condition {
    consume(move text)
}

consume(move text) // error: text may be uninitialized
```

### Initialization State Across Control Flow

The compiler tracks binding initialization state across control flow.

Tracked states:

```text
initialized
uninitialized
maybe initialized
```

Rules:

- New `let` and `var` bindings start initialized because local bindings require initializers.
- `move name` changes that binding to uninitialized on paths that continue after the move.
- `drop name` changes that binding to uninitialized on paths that continue after the drop.
- Successful whole-binding assignment to a `var` binding changes that binding to initialized.
- Reads, borrows, moves, field access, field assignment, method calls through the binding, and explicit `drop name` require initialized state.
- A maybe initialized binding cannot be used directly.
- At a control-flow join, only reachable incoming paths are considered.
- If all incoming paths have the binding initialized, the joined state is initialized.
- If all incoming paths have the binding uninitialized, the joined state is uninitialized.
- If incoming paths disagree, the joined state is maybe initialized.
- Scope end drops initialized bindings.
- Scope end does not drop uninitialized bindings.
- Scope end conditionally drops maybe initialized bindings.
- Conditional drop is generated by the compiler. It is not user-visible state and does not change the source-level type.
- A whole-binding assignment to a maybe initialized `var` binding may be used to restore the state to initialized.
- `if`, `match`, loop exits, `break`, `continue`, `return`, and postfix `?` propagation participate in the same state analysis.
- For loops, the compiler treats the body as running zero or more times and computes a conservative fixed point. If a binding's state may differ after the loop, the result is maybe initialized.

Examples:

```nct
var text = String.new()

if condition {
    consume(move text)
    text = String.new()
}

consume(move text) // OK: initialized on all paths
```

```nct
var text = String.new()

if condition {
    consume(move text)
}

consume(move text) // error: maybe initialized
```

```nct
var file = File.open(path)?

if should_close {
    close(move file)
}

// file is maybe initialized here.
// It cannot be used directly, but scope end will conditionally drop it.
```

```nct
var file = File.open(path)?

if should_close {
    close(move file)
}

file = File.open(other_path)?
file.read()? // OK: whole-binding assignment restored initialized state
```

