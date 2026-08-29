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
- Every semantic/source join uses one sealed generation.
- Missing authored evidence is explicit; an integrity failure cannot become an empty feature result.
- Rename and code actions publish only a whole-generation validated candidate.

The cross-crate presentation contract is documented in
[Semantic Presentation Design](../../../docs/semantic-presentation-design.md).
