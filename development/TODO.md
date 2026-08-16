# Nocter Development Handoff

## Current Task

Begin the v0.14.0 specification-first compiler rewrite. The previous compiler is preserved by
commit `f6c08da3` and removed from the active working tree. No previous source, test, binary
behavior, or implementation document may be used to answer a language-design question.

## Immediate Work

1. Complete the isolation checkpoint and regenerate contributor documentation.
2. Audit `spec/` from lexical input through execution and tooling.
3. Classify each gap as derivable, internal freedom, deferred scope, or user-visible ambiguity.
4. Stop at each user-visible ambiguity and request a decision with a minimal distinguishing program.
5. Record adopted rules in the sole owning specification chapter.
6. Derive new conformance cases only from closed normative rules.

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
