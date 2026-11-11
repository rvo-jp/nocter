# Nocter v2 Migration

## Goal and Completion

Complete the migration to a coherent Nocter v2 product, using the [adopted contracts](spec/README.md)
and [design goals](design/README.md) as the authorities. A private grammar, internal kernel or passing
isolated test does not close a product feature. The compiler, ordinary executable path, standard
library, tools, examples and documentation must agree on the qualified v2 scope.

This file owns unfinished work and findings. Component READMEs own implemented scope; specifications
own language meaning. Mark an item complete only after implementation, two reviews, appropriate
verification and documentation reconciliation. Recheck the whole product after every item closes;
record newly discovered work here before declaring the goal achieved.

Internal models, analysis, implementation and ordinary work order remain delegated. Obtain user
approval before a major syntax-design direction change. Publication and replacement of qualified
artifacts remain separately authorized actions, outside automatic completion of this goal.

## Current Unit

Extend the qualified [retained whole-element stage](compiler/crates/nocter-checking/README.md#array-selection)
while preserving one typed projection traversal, conservative access facts, actual saved-number
ownership, sparse cleanup and the original source bounds disposition. A recipe key alone never
proves equal numbers across a backedge. Do not add a parallel ownership registry, N element owners,
layout fields, generated operations, an arbitrary iteration cutoff or a constant-index-only policy.

- [x] Close unit-stride repeated immediate extraction without containing replacement using a compact
  proven prefix/frontier. Consume the checking-owned numerical prefix-extension facts through
  availability, guarded region transitions and execution. Preserve each normal bounds check,
  cancellation and exact cleanup when execution leaves before the user counter update. Do not
  overwrite a live selected-index owner or invent unbounded generations from repeated execution
  of the same source recipe.
- [x] Extend proven prefix extraction through direct marked take/read/edit argument handoffs,
  preserving frozen arguments and atomic commit/cancellation. Reuse the same endpoint transition,
  prepared argument owners and ordinary invocation/temporary channels.
- [x] Admit ordered multi-element prefix bundles from their closed extension proofs. Validate the
  whole bundle before exempting its neutral frozen-place nodes; preserve actual grants and outer
  reservations. Qualify distinct payloads, late counter mutation, two-reservation cancellation and
  maximum-count zero-byte execution in required and native paths.
- [ ] Generalize evaluated index relations beyond ordered prefix bundles and consume closed call
  write summaries before carrying numerical prefixes across invocation. Preserve lifetime-specific
  evidence for retained aliases across re-evaluation. Adapted callable-element transfers must also
  compose their selected adapters with the prefix handoff.
- [ ] Extend saved-index identity and initialization/cleanup to nested selectors, partial fields
  below a retained index, conditional restoration and multiple holes within one array. Keep
  unproved relations local and explicit. Do not read uninitialized alternative backing or choose
  guards from numeric equality alone; preserve actual original-index mutation and empty/MAX spans.
- [ ] Qualify actual nontrivial array cleanup effects through the
  [native array consumer](compiler/crates/nocter-machine/README.md#private-array-execution), including
  source error construction. Emitted/image-only cleanup is not execution of an owned error graph.

Keep required execution, native execution and independent logical tests aligned with the shared
source corpus. Extend budget-failure recovery alongside new working state and preserve homogeneous
geometry, payload-free empty arrays and the existing lifecycle ownership. General retained-frontier
admission follows the [evaluated-selection closure](design/compiler-realization.md#evaluated-array-selection-closure),
not a language restriction derived from the current realization boundary.

## Migration Checklist

- [ ] Complete array construction/repetition, evaluated indexing, sparse partial initialization,
  restoration and cleanup, evaluator data/publication, and native storage/ABI through ordinary
  source. Extend the [typed construction and selection](compiler/crates/nocter-checking/README.md#array-construction)
  and [concrete ledger](compiler/crates/nocter-checking/README.md#concrete-array-resource-frontiers)
  through the [structural request closure](design/compiler-realization.md#structural-type-request-closure).
  Preserve count supplier guards, one type authority, shared index relations and homogeneous/sparse
  representations. No constant-index-only policy or source-level cardinality ceiling substitutes
  for the required proofs.
- [ ] Close remaining value/access/type and callable behavior: contextual/multiple/nested captures,
  general R/D/S and survival, interfaces/generics/witnesses, constant arguments and symbolic conditions,
  exports and conditional const admission. Follow the [realization obligations](design/compiler-realization.md)
  and owning specifications, with ordinary source evidence for safe and unsafe cases.
- [ ] Close outcomes, runtime error construction, source-level nontrivial record/capture destruction,
  normal adapter cleanup before answer receipt, and absence of repeated body-owned destruction.
  Extend the [common native path](architecture/pipeline/value-access-declarations.md#common-machine-body-construction)
  with actual effects while preserving frozen channels, backing and cleanup timing.
- [ ] Close remaining expression/control, comparison, typed literal/conversion/operator, capability,
  memory/region, task/async and iteration subjects. Resolve the [remaining source grammar](design/source-grammar.md)
  against the adopted owners before acceptance; do not inherit an unspecified released production.
- [ ] Complete directory modules, contract-only index files, visibility, see hints and package/platform
  integration. Close the intended v2 platform and ABI scope with executable and negative source cases.
- [ ] Integrate v2 into the ordinary compiler/CLI and editor workflow with one semantic pipeline,
  current diagnostics, formatting, presentation and revision/source reuse. Remove transitional private
  handoffs when their production replacement is qualified; retain no competing semantic authority.
- [ ] Migrate standard-library implementations/contracts, installation and packaging inputs, runnable
  examples and user/contributor documentation coherently. Prepare specification promotion by moving
  authoritative contracts at the authorized release transition, without keeping editable duplicates
  or silently replacing released behavior and qualified artifacts.
- [ ] Implement and qualify the [optimization experiments](design/optimization.md) through ordinary
  source and executable paths. Establish correctness and generated operations before repeatable timing;
  preserve the language goals without an unsupported universal performance claim.
- [ ] Complete product-wide conformance and release qualification for the selected v2 scope under the
  [verification contract](verification/README.md). Perform the final architecture/code/documentation
  review, record and resolve new findings, and verify that no migration work remains. External
  publication is not implied by qualification.

## Open Findings

- [x] Keep prefix-lifecycle selection within co-executed array epochs. A later prefix loop on
  a replaced array must not reclassify an earlier point extraction/restoration on the same local.
  Qualify the mixed source path through checking, required execution and native execution.

- [ ] Investigate the unclassified status-1 failure of the released native test
  `network_and_tls::public_tls_and_https_reject_plain_peers_and_https_advertises_http1`.
  It is not consistently reproducible. Native exit assertions identify the sync/async image,
  but the failing Nocter return/error path and cause remain unclassified. Use the focused
  `nocter-native-session` test and its plain-peer fixture to establish a reproducer before
  changing product behavior or expectations. This path does not consume the private v2 Unit.

## Release Boundary

Publication of the retained [candidate](packaging/candidate.md) requires separate authorization.
Preserve its qualified artifact and source identity during v2 work. Linux target work remains
outside the selected current scope. No external blocker is established for current v2 work.
