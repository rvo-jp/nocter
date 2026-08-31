# nocter-language

## Responsibility

Own the closed, source-independent Nocter language vocabulary shared by lexing, parsing, semantic
modeling, tooling, and diagnostics.

## Contract

The crate publishes language constants and closed classifications, including the registered
diagnostic-code vocabulary generated from `diagnostic-codes.txt`. It does not parse source, own
diagnostic meaning or semantic identities, or decide whether a source program is valid.

## Invariants

- Every consumer uses the same vocabulary instead of maintaining a local spelling table.
- Adding a public vocabulary entry requires the owning specification change first.
- An unregistered string cannot be used as a compiler diagnostic code.
- The crate has no dependency on compiler stages or source storage.

Exact exported values belong to Rust source and rustdoc; public language meaning belongs to `spec/`.
