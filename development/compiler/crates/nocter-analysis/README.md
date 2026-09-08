# nocter-analysis

## Responsibility

Own one immutable protocol-independent editor analysis generation and every validated join between
semantic evidence and source projection.

## Contract

The crate consumes one compiler session outcome, reached source/syntax snapshots, overlay identity,
and source projection. It seals their generation integrity once, then publishes typed hover,
completion, navigation, reference, rename, token, signature, inlay, diagnostic, and code-action
results. Protocol crates receive result values only.

## Internal Responsibilities

- immutable analysis snapshot storage
- private semantic-evidence query kernel over session capability views
- complete/partial/unavailable query coverage
- deterministic source edit grouping
- validated semantic mutation transactions

## Invariants

- Feature modules cannot inspect session phase variants or raw `SourceIndex`.
- Presentation and signature queries consume exclusive semantic inputs instead of optional evidence
  combinations.
- Semantic highlighting joins exact semantic bindings with syntax-owned accepted scalar-literal
  tokens; it never decodes literal text or guesses unresolved names.
- Presentation renders authored callable guarantees from declaration or structural-type contracts;
  it never infers source modifiers from checked effects.
- Every semantic/source join uses one sealed generation.
- Diagnostics are read from the sealed discovery or analyzed state and are not cloned into a
  parallel snapshot authority.
- Code actions consume phase-selected diagnostic repair capabilities and completion-owned name
  relations; they do not recover semantic intent from codes, rendered labels, or source text.
- Missing authored evidence is explicit; an integrity failure cannot become an empty feature result.
- Rename and code actions publish only a whole-generation validated candidate. Candidate
  validation compares the source overlay's opaque authority identity rather than rescanning a
  parallel list of bytes and document versions.

The cross-crate presentation contract is documented in
[Semantic Presentation Design](../../../design/semantic-presentation-design.md).
