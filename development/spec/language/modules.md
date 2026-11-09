# Modules, Use Declarations, and Source Visibility

## Complete Implementation Signatures

Keep complete signatures in both the root contract and its corresponding implementation. This
future rule is adopted alongside contract-only `index.nct` roots, operation-bearing inputs, and
unified `impl`; it does not change the published syntax. The implementation remains readable
without opening the root merely to recover receiver/parameter names, types, or permissions.

```text
// index.nct; the Buffer contract is assumed to be in scope.
see ./buffer.nct
impl Buffer {
    method edit self.append(value: u8): void
}
```

```text
// buffer.nct
see ./index.nct
impl Buffer {
    method edit self.append(value: u8): void {
        self.storage.append(value)
    }
}
```

The implementation repeats the full receiver, generic/parameter list, result, bounds, applicable
origin clauses, and retained contract modifiers. There is no abbreviated `method append { ... }`
or `func name { ... }` body form. A new private helper likewise declares its own full signature.
Existing interface mappings select their checked helper; they do not create a short body-head
alias or require a new redundant root declaration beyond the interface contract.

The repeated header asserts agreement with one authoritative contract; it does not choose a
second public API. Retain direct contract/definition pairing and canonical header agreement:
select the candidate by module, owner/interface scope, callable kind, and member identity, then
check its complete header against the contract. Parameter/generic names, modes, types, bounds,
result shape, authored origins, and retained modifiers must match in canonical source notation.
Whitespace/comments are not header differences. The root alone owns publication, visibility,
and public declaration documentation. A body cannot strengthen the API from inferred usage.

A signature mismatch is a diagnostic at the differing components with both declaration locations;
it must not become a new private overload or trigger a search for a better-fitting body. Missing
and duplicate definitions remain errors independent of source traversal order. In particular,
`ReadView<File>?` and `ReadView<File?>` cannot match merely because their leaf File type agrees:
the illustrative capability types have different constructor order under the [future type-scope rules](values-and-types.md#type-constructor-scope).

This chooses source readability over eliminating repeated header text. The compiler checks that
repetition; editor navigation may link the two declarations but is not needed to recover an
omitted signature. Complete v2.0.0 module, lifecycle/default-body, and visibility migration remains
separate work before the full language boundary is implemented.
