# nocter-language

## Responsibility

Own the closed, source-independent Nocter language vocabulary shared by lexing, parsing, semantic
modeling, tooling, and diagnostics.

## Contract

The crate publishes language constants and closed classifications. It does not parse source, own
semantic identities, or decide whether a source program is valid.

## Invariants

- Every consumer uses the same vocabulary instead of maintaining a local spelling table.
- Adding a public language word requires the owning specification change first.
- The crate has no dependency on compiler stages or source storage.

Exact exported values belong to Rust source and rustdoc; public language meaning belongs to `spec/`.
