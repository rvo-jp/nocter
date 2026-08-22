# Nocter Development Handoff

## Current Task

Qualify the completed v0.14.0 implementation against the milestone completion definition. Phase 6
is complete. Publication remains a separate action and requires explicit user authorization.

The active compiler is the specification-first rewrite under `development/compiler/`. The previous
compiler was preserved by commit `f6c08da3` and removed from the active tree. Do not inspect or use
the archived implementation, its tests, released binaries, or historical output as implementation
input.

## Current Baseline

- The source, syntax, declaration, checked-program, MIR, machine, ARM64, Mach-O, package, command,
  formatter, standard-library, and editor phases are implemented through the production boundaries
  described in `development/milestones/v0.14.0.md`.
- Editor queries consume one immutable generation. Hover and semantic tokens share one deterministic
  source-binding authority; semantic ranges, cursor containment, containment, and overlap belong to
  `nocter-source`.
- Compiler-owned quick fixes cover imports, required conformance methods, and optional/fallible
  callable result contracts. Every edit is applied to an isolated overlay and must pass ordinary
  full-package compilation before publication.
- Every public single-file example executes as a native process with exact status, stdout, and
  stderr checks. The public `file-summary` package executes with a real file argument.
- ARM64 string-to-pointer copy now applies the authored destination offset. A native primitive
  conformance case and `custom-format.nct` output test protect the fix.
- The latest complete workspace test run passed with one intentional public-HTTPS integration test
  ignored. The latest warnings-denied workspace Clippy run passed before the final public-package
  execution test was added; rerun the complete verification matrix before closing qualification.

## Qualification Work

1. Rerun the complete workspace tests and warnings-denied Clippy after the latest commits.
2. Check each v0.14.0 completion criterion against a named specification section and executable
   test boundary. Add a test or fix only when the audit demonstrates a real gap.
3. Regenerate public documentation and require a clean regeneration diff.
4. Run formatting and repository whitespace checks.
5. Record the final qualification evidence in `development/milestones/v0.14.0.md` and stop before
   packaging, tagging, pushing, or publishing.

## Guardrails

- `spec/` is the sole source of public language behavior.
- Ask the user only when an observable behavior remains ambiguous after reading the specification.
- Do not add compatibility fallbacks, source-text semantic inference, duplicate indexes, or reverse
  lookup from presentation strings.
- A later phase cannot import an earlier phase's private representation to repeat its decisions.
- Source order, declaration order, filesystem enumeration, and arena insertion order must not select
  between otherwise equal semantic candidates.
- Keep public documentation in English. Edit source Markdown and regenerate the website.

## Verification

```sh
cargo fmt --manifest-path development/compiler/Cargo.toml --all --check
cargo clippy --manifest-path development/compiler/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path development/compiler/Cargo.toml --workspace
node docs/build-docs.js
git diff --check
```
