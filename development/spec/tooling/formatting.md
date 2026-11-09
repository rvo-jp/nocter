# Source Style and Formatting

## Value Binding Formatting

Future formatting follows the [value/alias grammar](../language/syntactic-grammar.md#access-syntax).
Use `let value = expression`, `var value = expression`, `read alias = place` and
`edit alias = place`, with one space after the operation word and around `=`. An annotation is
`name: Type`; a parameter operation precedes the name: `edit buffer: Buffer`, `take job: Job`.
Receiver forms are `self`, `edit self` and `take self`; retain parentheses for a receiver origin
clause, as in `(edit self from self | buffer)`. Ordinary calls have no access assertions.

Callable input operations precede an anonymous type (`func(edit Buffer): void`). A mutating
environment clause is separated from its parameter list by one space:
`mut from input (input: Buffer): void`. Result origins stay after the complete result type.
Do not move a clause between these subjects or add parentheses that change its attachment.
Preserve `from a | b` order in canonical source; it is not name-based semantic identity.

Value captures use `(file; path: Path) { ... }`, with no space before `;` and one after it when
parameters follow. Do not insert retired symbolic captures or ordinary type qualifiers.
Type/outcome grouping preserves constructor order. A formatter cannot convert an owning `let`
into `read`, introduce Copy evidence, or repair lifetime/authority by changing source operations.
Published formatting remains unchanged until a qualified release transition.

## Contract and Operation Formatting

Keep full matched headers in contract and implementation sources. Formatting cannot synthesize
a body in `index.nct`, move a private helper into the root, shorten a header or publish an import.
Format each item directive on its own line immediately before its subject, preserving comments.
Canonical independent directive order is `#visibility`, then `#target`. A blank line is not a
visibility reset. Reject duplicate/unsupported directives rather than discarding them.

Use `impl Type { ... }` for type behavior and inner `impl Interface` for conformance. A conformance
with bindings has one comma-delimited block; `.Item = T` is an associated binding and
`read = read_bytes` is a named operation mapping. Format a multi-entry block one entry per line,
with a trailing comma. Do not split type bindings and operation mappings into two blocks or
generate redundant same-name mappings. `literal { x, y }` is a structural grant, not a body to
expand into another initializer. Interface-selection spelling remains
`value.(Interface.member)(arguments)` where selection is required; formatting does not resolve
ambiguity by adding or removing a selector. [Modules](../language/modules.md) and
[interfaces](../language/interfaces.md) own validity, not formatting.

## Generic and Constant Formatting

Keep generic call arguments joint with the selected callee and owner arguments attached to their
owner. Do not rewrite an explicit list into inferred arguments or insert `_` placeholders. Retain
grouping that separates a constant comparison/right shift from closing angle delimiters. A
type-position collision selector is `T.(Interface.Item)`; do not add it for unique lookup or
remove it as cosmetic qualification. Normalize neither argument values nor aliases by rewriting
authored source: equal semantic type arguments can retain different source expressions.

Keep `const` and `static` distinct. Formatting cannot inline a required initializer into a public
root, turn a runtime function into const, expand a generated table, replace a constant recipe
with shared static storage or alter an opaque result to its hidden witness. Staging and final-data
eligibility belong to the [generic/constant contract](../language/generics-and-constants.md).

## Outcome Formatting

Use `throw expression` without a transfer marker. Preserve outcome constructor order/grouping
and the single expression-level extraction suffix. Format explicit `some(...)` as an ordinary
call; do not insert it at every optional injection or replace a shadowing declaration by the
prelude operation. Preserve `catch name`, explicit `catch _` and `otherwise` rather than adding
an implicit handler. Keep the declared `let`/`var`/`read`/`edit` operation: rewriting a projection
as value extraction changes source availability and cleanup. Formatting cannot silence the
failure-loss warning by inserting a discard or inventing resource completion.

## Async Staging

Keep async on its selected declaration head and future on the result/type it actually wraps.
An interface default puts default before that ordinary head: `default async method ...` or
`default const method ...`. Do not introduce another modifier order.
Preserve parentheses in `await (make_future()?)` versus `await operation()?` and constructor
grouping in `future R!` versus `(future R)!`. Do not add transfer prefixes or an async closure/
callable-type flag. Formatting cannot turn a deferred body into immediate work, add cancellation
qualifiers or convert inferred effect reports into authored source guarantees.

## Enum and Pattern Formatting

Format enum alternatives one per line and payload positions as comma-delimited lists. A
payloadless variant has no parentheses. Keep copy enum when authored; hidden representation or
a known active tag cannot add/remove its ownership contract. Patterns use qualified owners and
the same tuple syntax as binding/input patterns, without per-leaf operation prefixes.

Print one space after a match/if-is edit/take prefix. Joint `take(value)` in a header is instead
an identifier call; whitespace cannot be changed so it becomes the prefix reading. Preserve
grouping required to keep an identifier expression outside that contextual prefix. Do not add
an explicit-read branch mode, infer take from body use, remove required payload underscores or
generate a catch-all that changes structural coverage. Discard cleanup and branch results remain
semantic operations, not formatter repairs.

## Tuple and Fixed-Array Formatting

Format tuple values as `(left, right)` and fixed-array lists as `[first, second]`; repetition is
`[value; count]`, with no space before its semicolon and one after it. Unit and grouping retain
their distinct `()` and `(expression)` roles. A formatter cannot invent a single-element tuple,
expand repetition into repeated producer calls, omit zero-count evaluation or insert a spread.

Keep primitive selection joint with its base, as in `pair.0` and `items[index]`. A spaced typed
literal is a separate family; whitespace normalization cannot turn selection into construction.
Preserve authored count expressions and component order rather than sorting, evaluating or
rewriting types from a hidden representation. Formatting cannot restore a missing element,
convert a computed selection into a place, or replace an owning move by a projected access.

## Variadic Formatting

Keep ellipsis before an input operation: `... take items: T` or `... edit entries: K: V`.
Unmarked read is `... items: T`, without an explicit-read alternative. A structural ordinary
callable pack may omit its name; a keyed pack retains its name and both colons. A keyed closure
annotation writes both types. Formatting cannot infer a pack kind or turn its binder into a
tuple/discard pattern.

At a call, use `...items` with no intervening space and `key: value` for a direct keyed entry.
Do not insert/delete expansion, labels or tuple construction, reorder keys or deduplicate them.
Preserve the complete fixed prefix and contribution order. A pack value capture remains the
existing `(items; ...)` form; it cannot become a bare-name array binding or postpone ownership
until invocation. Metadata queries and indexed entries retain ordinary `items.len()` and
`items[index]` spelling without synthesized access markers.

Preserve the complete type grouping in `edit action: (mut(): void) from source`, including for
pack element types. Removing it attaches from to an inner callable result; adding a consecutive
from is not a substitute. Existing closed array/generic delimiters do not need redundant grouping.

## Traversal Formatting

Use `for pattern in source`, `for edit pattern in source` or `for take pattern in source`.
Keep tuple patterns for keyed entries and preserve complete-pattern contextual names. Do not
generate an explicit-read mode, colon-key binder or range-only loop branch. Range helper names
remain ordinary declarations rather than formatting privileges.

Keep current projection joint as `cursor[]`, distinct from `cursor[()]` and spaced typed literal
construction. Declarations are `operator (self[]): T` / `operator (edit self[]): T`, while source
operators use `operator (... self): Cursor` and its edit/take receiver forms. Formatting cannot
change a fixed operator family, turn a method result into a place, add yield, reorder pulls or
merge source/body cleanup schedules.
