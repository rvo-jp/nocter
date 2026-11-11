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

Extend the [selection path](compiler/crates/nocter-checking/README.md#array-selection) beyond proven
concrete element holes to retained numeric dynamic indices. Preserve the shared typed path,
container/value availability queries, sparse ledger and interval cleanup consumers. A logical
path with an unknown index is not a concrete ownership key. Do not add a second ownership
registry or a separate bounds evaluator.

- [ ] Connect the [logical retained-index backing](compiler/crates/nocter-checking/README.md#concrete-array-resource-frontiers)
  to source dynamic transfer after normal bounds completion. Commit with an
  immediate transfer or the complete call bundle. Cancellation creates no retained hole. A live
  snapshot owner cannot be overwritten by another execution of the same recipe.
- [ ] Extend the shared path, availability and cleanup representation to retained whole-element
  indices under a concrete containing-array path. Source recipe IDs alone never prove equal
  values. Keep snapshot ownership in branch conditions: equal runtime numbers from different
  paths must not erase their distinct backing lifetimes. Preserve the original required bounds
  check, captured numeric data, empty dynamic remainder intervals and checked successor at MAX.
- [ ] Qualify source transfer, read/edit lending, original-index mutation, normal replacement and
  cleanup through required execution and native execution. Check joins and backedges, snapshot
  retirement with its array, cancellation, zero-sized elements and storage-failure recovery.
  Keep unsupported nested/multiple dynamic frontiers local and explicit while closing finite loop
  progression; do not turn a staged realization boundary into a language restriction.

The [closed evaluator address mechanism](compiler/crates/nocter-constant-evaluation/README.md#evaluated-array-addresses)
is an execution consumer, not proof of precise element-state safety. Preserve the evaluated numeric
index through reservations, aliases and holes; equal recipe keys across a backedge do not establish equal values.
General retained-index transfer and partial-state admission remain gated until the
[evaluated-selection closure](design/compiler-realization.md#evaluated-array-selection-closure)
closes joins, loops and cleanup alongside the ordinary whole-value construction path. A finite
concrete-path stage must not become a constant-index-only source policy or substitute for retained
dynamic holes and finite progression summaries.

Qualify actual nontrivial array cleanup effects through the
[native array consumer](compiler/crates/nocter-machine/README.md#private-array-execution), including
source error construction. Respect its stated qualification boundary: emitted/image-only cleanup
is not execution of an owned error graph.
Preserve homogeneous geometry, payload-free empty arrays and the existing lifecycle ownership;
do not add N compiler owners, layout fields or generated operations. Actual native nontrivial effects
and general dynamic-hole cleanup remain separate consumers to close before product admission.

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
