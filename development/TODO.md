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

Extend the qualified [retained-selection stage](compiler/crates/nocter-checking/README.md#array-selection)
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
- [x] Preserve numerical locals and prefix epochs across exact closed named calls without writes
  into existing caller storage. Close recursive/transitive dependencies and cleanup eligibility,
  retain exact typed owners, and qualify loop calls plus early header-count execution. Unknown
  writes, formal/captured cleanup and absent callee evidence remain conservative.
- [x] Extend named call write summaries to selective input targets, including recursive input
  permutations, edited old-value cleanup, transferred temporary edits and interrupted argument
  preparation. Preserve disjoint scalar/prefix facts through required and native source execution.
- [ ] Extend call write summaries to captured/destruction effects and general alias provenance.
  Generalize evaluated index relations beyond ordered prefix bundles while preserving lifetime-
  specific evidence for retained aliases across re-evaluation. Adapted callable-element transfers
  must also compose their selected adapters with the prefix handoff.
- [x] Close cleanup write effects for exact owned closure environments through their declaration
  schema, without executing or attributing the closure body's invocation effects to destruction.
  Preserve unknown formal cleanup and qualify surrounding scalar/prefix inference through
  source checking and native execution, including aggregate environment payloads.
- [x] Resolve lexical edit-alias roots for named caller-write summaries from their existing typed
  binding initializers. Preserve explicit input positions and owned-local separation through
  alias chains; do not claim exact numeric paths or general escaping alias provenance. Qualify
  selective prefix/scalar preservation and rejection of aliased counter invalidation.
- [x] Project caller-side lexical edit aliases to conservative existing source footprints using
  that same binding resolver. Preserve original invocation addresses and never reload the index
  of an older alias; use containing storage when an exact selection cannot be retained. Qualify
  disjoint prefix preservation and aliased counter invalidation in source/native execution.
- [x] Close partial-field extraction below one retained index, including constant nested selectors,
  reverse remaining-field cleanup, snapshot retirement/restoration, call commit/cancellation,
  conditional holes, parent replacement and maximum-count zero-byte execution. Qualify bounded
  evaluator storage and recovery without equating different cleanup-order peaks.
- [x] Close one nested missing path through multiple dynamic selectors. Capture outer-to-inner
  with atomic rollback, preserve each saved value across index-side mutation, and retire all
  backing through restoration or reverse nested cleanup. Qualify distinct payloads and two-
  dimensional maximum-count zero-byte geometry through required and native execution.
- [x] Close multiple dynamically selected, structurally disjoint field holes under one array.
  Preserve exact missing-point coverage, equal/unordered saved indices, native reverse traversal,
  exhaustive cleanup-helper registration, MIR geometry and bounded evaluator failure recovery.
- [x] Preserve mixed prefix/point coverage in sparse remainder cleanup, retaining the existing
  coverage variants and their distinct bounds/roles. Qualify zero-trip and partial-prefix paths,
  equal-valued backing, maximum-count zero-byte geometry and nontrivial generated predicates.
- [ ] Extend saved-index identity and initialization/cleanup to conditional restoration and
  general numeric relations among holes within one array. Keep
  unproved relations local and explicit. Do not read uninitialized alternative backing or choose
  guards from numeric equality alone; preserve actual original-index mutation and empty/MAX spans.
- [x] Restore one retained whole-element hole from a proved equal current index using the
  existing committed-index history and actual sparse hole/snapshot. Validate the retiring source
  site and present parent; never identify lifetimes from recipe equality alone. Qualify index
  mutation, direct calls, restoration on both branches/backedges and maximum-count zero-byte
  source/native execution, while keeping unproved and multi-hole relations conservative.
  Track scalar read inputs under their existing grant without authorizing updates or retaining
  facts across unknown writes; ordinary input use must not require a redundant local copy.
- [ ] Recover a complete numerical array epoch after restoring its sole proved point. Distinguish
  a point taken from a complete epoch from generic unproved coverage; do not reset after restoring
  only the last element of a longer prefix or after unrelated partial holes. Qualify a later
  prefix loop through the same required/native paths without containing replacement.
- [x] Share current numerical constant selection facts across availability, access creation and
  resource paths. Freeze constants when a place/hole is created and elide unnecessary saved-index
  owners; retain snapshots for unproved values. Do not refine old aliases from a recipe's later
  execution or reinterpret a prefix as one point. Preserve original bounds decisions, local
  unknown-use diagnostics and exact cleanup through ordinary source and native execution.
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
  evaluator closure construction/invocation/cleanup, general R/D/S and survival,
  interfaces/generics/witnesses, constant arguments and symbolic conditions,
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

- [x] Close multiple dynamic field holes without an integrity failure. Source initialization
  and permissions admit different fields even when their indices may coincide, but snapshot
  capture and cleanup still assume one retained hole per array. The reproducer is
  `compiler/tests/fixtures/arrays/element-multiple-dynamic-fields.nct`. Project the existing sparse
  missing set through required/native remainder cleanup; preserve reverse element/component
  destruction, equal-index field merging, saved-index lifetimes and local unsafe-use rejection.

- [x] Preserve mixed array-prefix and point exclusions in remainder cleanup. Qualify a prefix
  loop on a concrete nested array followed by a dynamic transfer from another element field,
  preserving coverage roles and all backing through required and native execution.

- [x] Select an expression's actual operator token across admitted continuation newlines.
  Qualify arithmetic, comparison and logical source execution while preserving syntax-owned
  continuation rules, including delimited blank lines.

- [x] Bind flow-effect reuse to actual callee-body and value/count/result dependencies. Typed calls can survive a
  callee-only body edit, but inferred numeric/availability proofs cannot. Preserve reuse after
  unrelated body edits and avoid repeating whole-unit verification for every body.

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
