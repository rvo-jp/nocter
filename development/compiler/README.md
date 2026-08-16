# New Nocter Compiler

This directory is the implementation root for the specification-first Nocter compiler rewrite.
The lexical and syntactic grammar gate is closed; the Phase 1 source/syntax workspace is the next
implementation boundary.

## Authority

The compiler derives public behavior from [`spec/`](../../spec/README.md). Missing language rules
block implementation; they are not inferred from the archived compiler, old tests, released
binaries, or historical implementation documents.

## Isolation

The new compiler must never depend on, copy, execute, or compare itself with the compiler preserved
by commit `f6c08da3`. Existing standard-library implementation details are also not bootstrap
semantics. Public standard-library contracts come from the specification and will receive new
implementations after the required language foundation exists.

## Planned Dependency Direction

```text
source
  -> syntax
  -> semantic core
  -> analysis and checked program
  -> executable program
  -> MIR
  -> machine program and code generation
  -> CLI and editor adapters
```

Later stages cannot import syntax representations to reconstruct earlier decisions. Source ranges
remain outside semantic identity, and runtime linkage is a one-way output projection.

The Cargo workspace must begin with source and syntax responsibilities only. Its parser fixtures
derive from the [grammar conformance plan](../docs/grammar-conformance.md); semantic crates cannot
be introduced to make an unresolved syntax choice.
