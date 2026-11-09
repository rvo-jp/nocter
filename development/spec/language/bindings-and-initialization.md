# Bindings and Initialization

## Assignment and Flow

The future [Value Access Model](value-access.md) uses fixed `let` and mutable `var` value
bindings, with `read`/`edit` for scoped aliases. Type annotations constrain data types; they do
not infer a binding's operation or turn a value binding into an alias.

`let _ = expression` constructs/discards one value under the ordinary Copy-or-move rule. For a
non-Copy owner this removes and destroys that value. For a Copy place this discards a snapshot,
leaving the source available. An unhandled fallible outcome still receives the
[default-enabled discard warning](outcomes.md#failure-decisions-and-resource-completion);
discard is not handling failure. A non-final expression must produce `void` or terminate. A final block
expression is an ordinary value-producing boundary, not an implicit scoped alias.

Tuple bindings use [one pattern operation](patterns.md#one-operation-over-a-pattern) over their
leaves. Value decomposition acquires the aggregate once, stages named components and cleans the
remaining carrier before continuation; read/edit patterns project existing components. Do not
infer a leaf operation from its Copy status or turn a typed all-discard pattern into another
discard spelling. Refutable enum patterns use match/if-is, not let-else or a refutable local binding.

### Assignment Order and Reinitialization

Evaluate/construct the entire replacement first, then evaluate dynamic destination components
once, check the destination's resulting state, clean any initialized old value, and install the
replacement. Protect the replacement and its dependencies through target evaluation and cleanup.
Reject destruction of storage that the replacement still needs. The right-hand side can move
an eligible non-Copy owner without a prefix; Copy snapshots instead. No destination reservation
or old-value read is introduced before the right-hand evaluation.

A writable destination is a mutable owner, locally writable `take` slot, or `edit` referent.
Fixed owners/read aliases are not writable. An edit replacement never retargets the alias and
must preserve complete initialization on every outward recoverable edge. Mutable owning slots
can be reinitialized after move/drop; fixed bindings cannot. Maybe-initialized old owners receive
conditional cleanup, never an invented complete-value read.

If replacement computation fails, do not evaluate the target, clean its old value, or store.
Completed nested moves/calls/effects are not rolled back. In `value = value`, a non-Copy mutable
owner first transfers out and is then reinstalled without old-value destruction; a Copy value
supplies a snapshot. `value = replace(value)` follows the selected callee contract and can leave
an owning slot uninitialized on failure if that callee has consumed its value.

Compound assignment is likewise RHS-first, then target identification/read/computation/write.
`value += consume(value)` cannot read an unavailable target; for a Copy `take` input the source
instead remains available. Numeric/overflow rules do not change. Ordinary update cannot leave an
uninitialized external referent while evaluating a fallible replacement.

### Conditional Values, Aliases and Loops

Every normally producing branch of an ordinary conditional/block result constructs the common
value type. Each branch applies Copy-or-move using evidence available in that branch's checking
context. A terminating branch supplies no value. Type inference cannot invent an alias or choose
Copy from later uses. For example, an integer branch snapshots a named source while a move-only
branch transfers its owner. A computed conditional supplied to a read input has already constructed
its value; it is not a direct alias selection. `read`/`edit` binding initializers require places.

At joins, keep initialization and alternative dependency/access worlds separate. An operation must
be safe on every possible incoming path; distinct exclusive accesses on mutually exclusive paths
are alternatives, not simultaneous accesses. Moving a source on one path can make it unavailable
after the join. A complete `var` assignment can restore definite initialization.

Access duration includes later uses, retained-state effects, prepared calls, and dependent cleanup.
An `edit` declaration activates delegation immediately. A direct edit call operand reserves its
place and activates at invocation under the distinct preparation rules. Report a conflict at the
attempted operation and identify the access creation plus later demand/cleanup.

Loops compute a conservative fixed point over initialization, active/reserved accesses, carried
dependencies and resource responsibility. Include zero iterations, continuing backedges and every
actual exit. A move followed by replacement only on an exiting path does not make another iteration
safe. Exits dispose exactly the initialized resources and keep edited external places valid;
terminal traps without unwinding follow their own contract. No lifetime extension, hidden clone,
or runtime sharing repairs an unproved relationship.
