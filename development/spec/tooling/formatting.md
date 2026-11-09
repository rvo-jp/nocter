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
