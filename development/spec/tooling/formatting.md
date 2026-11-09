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
