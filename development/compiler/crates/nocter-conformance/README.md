# nocter-conformance

## Responsibility

Verify that production crate contracts compose into deterministic compiler, editor, and native
behavior across the complete pipeline.

## Contract

This is the only test crate allowed to depend on the full production chain. It may construct
end-to-end fixtures and assert architecture manifests, but it cannot export production behavior or
serve as an implementation dependency.

## Invariants

- Native end-to-end tests compose public stage contracts; command and workspace tests exercise the
  production compiler-computation entry.
- Test helpers cannot become a compatibility layer or semantic oracle.
- Architecture tests validate resolved dependencies and types rather than source-text spellings.
