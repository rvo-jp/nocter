# Current Development Work

## Immediate Work

1. Qualify [const source and conditional eligibility](design/compiler-realization.md#conditional-const-admission)
   beyond the [required frozen-data initializer boundary](compiler/crates/nocter-checking/README.md#required-private-initializers)
   with fixed arrays, enums and early structural dependencies. Extend
   [staged source preparation](compiler/crates/nocter-checking/README.md#bounded-future-value-binding-typing)
   through the [structural type request closure](design/compiler-realization.md#structural-type-request-closure),
   connecting admitted usize computation to required lengths through the same dependency
   coordinator. Close semantic code identity and extend shared source evidence to header expressions,
   including record fields, with body-dependent header invalidation before admitting array source. Extend
   symbolic conditions and exports as ordinary generic/witness schemas are admitted. Qualify
   comparison and typed literal/conversion/operator support against their owning contracts.
   Then close remaining expression/control, capability/library,
   package/platform and task/async-iteration subjects in the [source draft](design/source-grammar.md).
   Integrate ordinary reading/writing examples before extending accepted source scope.
2. Extend the [bounded private native path](architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
   with source/runtime error construction and observable nontrivial record/capture destruction.
   Qualify adapter cleanup before protected answer receipt and absence of repeated body-owned
   destruction through actual native effects. Preserve frozen channels, backing and cleanup timing;
   helper-generation evidence alone does not qualify runtime ownership behavior.
3. Extend contextual/multiple/nested captures, general R/D/S schemas, interfaces/generics, outcomes,
   async and public modules only with their owning specification and end-to-end evidence.
4. Run the vertical [optimization experiments](design/optimization.md) after their ordinary source
   and executable paths are admitted. Qualify correctness and generated operations before timing.

Use the [agent instructions](../AGENTS.md) for delegated scope and syntax approvals, and the
[verification guide](verification/README.md) to select checks that consume the changed path.

## Open Findings

- Investigate the unclassified status-1 failure of the released native test
  `network_and_tls::public_tls_and_https_reject_plain_peers_and_https_advertises_http1`.
  It is not consistently reproducible. Native exit assertions identify the sync/async image,
  but the failing Nocter return/error path and cause remain unclassified. Use the focused
  `nocter-native-session` test and its plain-peer fixture to establish a reproducer before
  changing product behavior or expectations. This path does not consume the private v2 Unit.

## Release Boundary

Publication of the retained [candidate](packaging/candidate.md) requires separate authorization.
Preserve its qualified artifact and source identity during v2 work. Linux target work remains
deferred. No external blocker is established for current v2 work.
