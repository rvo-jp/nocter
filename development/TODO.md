# Nocter Development Handoff

## Current Task

Close the v0.14.0 public grammar before scaffolding the new compiler. The previous compiler is
preserved by commit `f6c08da3` and removed from the active working tree. No previous source, test,
binary behavior, or implementation document may be used to answer a language-design question.

## Immediate Work

1. Consolidate blocks, bindings, control flow, patterns, and body-result positions into
   `spec/25-syntactic-grammar.md`.
2. Consolidate the complete precedence expression grammar, construction, literals, closures, and
   outcome elimination into the same chapter.
3. Replace remaining duplicate formal productions with links to that sole grammar owner.
4. Audit contextual keywords and add valid, boundary, and invalid conformance cases derived only
   from the closed grammar.
5. Scaffold the source/syntax compiler workspace only after the grammar gate is complete.

## Guardrails

- Do not restore or inspect the archived compiler.
- Do not migrate archived tests or diagnostics.
- Do not run a released compiler to discover unspecified behavior.
- Do not treat the existing standard-library implementation as language semantics.
- Do not create the new compiler workspace before the grammar closure gate.
- Do not mark specification closure complete while an observable choice remains implicit.

## Verification Available During Specification Closure

```sh
node docs/build-docs.js
git diff --check
```
