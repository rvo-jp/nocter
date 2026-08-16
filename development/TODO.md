# Nocter Development Handoff

## Current Task

Begin v0.14.0 Phase 1 from the closed public grammar. The previous compiler is preserved by commit
`f6c08da3` and removed from the active working tree. No previous source, test, binary behavior, or
implementation document may be used as an implementation input.

## Immediate Work

1. Scaffold the smallest Cargo workspace that owns only source storage and syntax.
2. Implement normalized source identities, spans, token kinds, joint-token facts, and lexical
   diagnostics without semantic names or types.
3. Expand the first rows of the
   [grammar conformance plan](docs/grammar-conformance.md) into lexer and parser fixtures.
4. Add parsing incrementally in grammar dependency order; do not introduce declaration or checked
   semantic crates during Phase 1.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not create the new compiler workspace before the grammar closure gate.
- Do not mark specification closure complete while an observable choice remains implicit.

## Verification Before the Workspace Exists

```sh
node docs/build-docs.js
git diff --check
```
