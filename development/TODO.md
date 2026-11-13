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

Complete [captured callable instance identity](design/compiler-realization.md#captured-callable-instances)
through the declaration-owned witness/binding model before admitting formal callable values into
owned environments. The same lexical closure with different captured callback substitutions must
remain distinct when both values are forwarded together. Derive dependencies from the canonical
environment schema; retain nested context, frozen invocation views and one normalization authority.
Integrate the resulting frame and shared `DataInstanceKey` projection with ordinary ownership
checking, conditional const admission, evaluator plans and native storage. Preserve captured
context across return/application producers without reconstructing it from a signature or code name. Qualify safe/unsafe source cases,
exact source/type reuse and deterministic failure recovery. A parser or metadata-only extension
alone does not complete capture support. Preserve current rejection until executable consumers agree.

Resolve value-occurrence creation context through the retained source-flow producer equations
and ordinary value transport, then close source closure creation before admitting formal callable
captures. Complete value/component transport and representation from retained closure-creation
snapshots and structural producer facts. Replace per-frame leaf-TypeId assumptions where distinct
values share a source type but retain different captured substitutions. Component consumers now use
declaration-validated fields rather than bare child TypeIds; the canonical data key now retains
component-specific contexts and accepts exactly typed carrier assembly. Complete occurrence-specific
source transport and callable-environment/frame identity behind that boundary, rather than treating
metadata carrier assembly as their proof. Keep one declaration-owned
normalization authority and qualify evaluator/native receiving, storage and cleanup together.
Use occurrence-resolved callable construction to retain actual contexts instead of rebinding their
free parameter names in the receiving frame; it does not itself grant a matched source receipt.
Do not substitute outer bindings when occurrences or creation snapshots are missing. Preserve
dormant definition-contract checks, factored symbolic conditions, condition/plan-work failures,
pre-commit preparation behavior, exact creation contexts,
anchored recursion, alternative handling and failure/retry behavior; source formal-capture
creation and per-value/component representation still require integration.
Follow the occurrence-sensitive finding below and the canonical instance obligations. Keep semantic
`TypeId` in source checking and declarations; contextual runtime IDs must never become fabricated
semantic types. A representation key alone grants no payload, source admission or cleanup proof.

Continue symbolic value/resource-state guarded discharge and independent module publication after
that instance boundary is coherent. General R/D/S, dependent carriers, deeper callable inputs,
inferred captures, module/toolchain integration and retained-selection closure remain open.

## Retained Selection Closure

Extend the qualified [retained-selection stage](compiler/crates/nocter-checking/README.md#array-selection)
while preserving one typed projection traversal, conservative access facts, actual saved-number
ownership, sparse cleanup and the original source bounds disposition. A recipe key alone never
proves equal numbers across a backedge. Do not add a parallel ownership registry, N element owners,
layout fields, generated operations, an arbitrary iteration cutoff or a constant-index-only policy.

- [ ] Extend call write summaries to externally dependent captures, user destruction and general
  escaping alias provenance. Generalize evaluated index relations beyond ordered prefix bundles
  while preserving lifetime-specific evidence for retained aliases across re-evaluation.
- [ ] Extend saved-index identity and initialization/cleanup to conditional restoration and
  general numeric relations among holes within one array. Keep
  unproved relations local and explicit. Do not read uninitialized alternative backing or choose
  guards from numeric equality alone; preserve actual original-index mutation and empty/MAX spans.
- [ ] Extend actual owned-error array cleanup to externally dependent captures and their
  surviving callback results through the
  [native array consumer](compiler/crates/nocter-machine/README.md#owned-error-effects).
  Emitted/image-only cleanup is not execution of an owned error graph.

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
- [ ] Close remaining value/access/type and callable behavior: standalone anonymous inference and
  inferred/general capture dependencies,
  general evaluator capture/input dependencies and formal substitution, general R/D/S and survival,
  interfaces/generics/witnesses, constant arguments and symbolic conditions,
  exports and conditional const admission. Follow the [realization obligations](design/compiler-realization.md)
  and owning specifications, with ordinary source evidence for safe and unsafe cases.
- [ ] Close required outcome construction within the adopted const domain, general source-level record/capture destruction,
  cleanup for remaining adapter forms, and absence of repeated body-owned destruction.
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

- [ ] Preserve value-occurrence producer and creation-frame context when one caller receives the
  same lexical closure with different captured callback substitutions. A per-frame `TypeId` map
  cannot distinguish those results after code-only result projection. Extend the existing matched
  producer/application evidence through local, argument, component and return transport, using the
  canonical witness/data instance authority. Keep formal-capture source admission closed until
  this case and its cleanup are qualified; see the [instance obligations](design/compiler-realization.md#captured-callable-instances).

- [ ] Extend symbolic discharge beyond covered root-closed named/owned-environment witnesses and
  parameter-selected calls without callable receiving dependencies. Free/unresolved captures,
  compound dependent data and value/resource-state guards retain exact-frame checks. Preserve
  matched body/type receipts, receiving operations, actual cleanup, exact supplying-argument causes
  and source safety.
  Work quotas bound uncertainty without proving eligibility; module publication retains its
  independent qualification.
- [ ] Complete nested callable data/witness support through header/data positions and the owning
  type authority. Keep valid formal structural descriptors separate from owned-payload admission.
  Qualify concrete representation, substitution and callable input/result evidence;
  one recognized type tree and a matching signature do not erase code/environment identity.
  Any final callable-value extension must satisfy the separate constant/static serialization
  contract; callable environments cannot inherit final-value eligibility from grammar recognition.
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
