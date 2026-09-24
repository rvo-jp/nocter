# nocter-language

## Responsibility

Own the closed, source-independent Nocter language vocabulary shared by lexing, parsing, semantic
modeling, tooling, and diagnostics.

## Contract

The crate publishes language constants and closed classifications, including the registered
diagnostic-code vocabulary generated from `diagnostic-codes.txt`, package-directive and intrinsic
argument-pack member names, the ordered callable-modifier vocabulary and grammar categories, and
the canonical physical source layout. It does not parse source, own diagnostic meaning or semantic
identities, or decide whether a complete source program is valid.

## Invariants

- Every consumer uses the same vocabulary instead of maintaining a local spelling table.
- Parser recognition, semantic presentation, and editor completion read one callable-modifier
  order. The vocabulary also owns context-independent modifier conflicts; declaration checking
  owns rules that depend on callable kind or implementation authority.
- The source extension, module-root file name, and recursive editor glob derive from one physical
  source-layout declaration.
- Adding a public vocabulary entry requires the owning specification change first.
- An unregistered string cannot be used as a compiler diagnostic code.
- The crate has no dependency on compiler stages or source storage.

Exact exported values belong to Rust source and rustdoc; public language meaning belongs to `spec/`.
