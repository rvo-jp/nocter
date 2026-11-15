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

Complete occurrence-sensitive value/capture transport under the
[captured callable instance obligations](design/compiler-realization.md#captured-callable-instances).
The [checking](compiler/crates/nocter-checking/README.md) and
[target-program](compiler/crates/nocter-target-program/README.md) owners describe the implemented
canonical metadata, original source-definition queries and ordered lifecycle specialization.

Extend guarded transport through application-specific received outcome contexts using original
supplied/receiving expressions and the callee's own result descriptor. Preserve shared value graphs,
single-owner transitions, evaluator effects and separate physical layouts. Qualify retained-alias
contexts independently of whole-value carrier transport.

Continue producer-less outcome qualification with representation widening and application-specific
empty received contexts, preserving original tag evaluation, required traps and
exact result-leaf, call and code contexts through each consumer. Continue context-changing updates through edit input boundaries,
partial carrier-root selection and context-changing evaluator slots against
the exact per-value keys. The component owners above describe covered ordinary operand/definition,
evaluator slot metadata, shared body-channel metadata, invocation, body-ABI, lifecycle and storage scope.
Keep evaluator slot selection on checking's shared original-value and ordered resource-data
projection; the checking owner records covered non-materializing views. Keep
execution lowering separate from source-value normalization.
Extend reference-root contract transport through resource-step carrier selection. Source-place
address projection uses checking's frozen original root contracts. Extend retained dependent
partial-source roots and matched owning-place before/after contracts to remaining evaluated
destinations and external alias carriers. A frame-wide type default cannot bypass
this qualification boundary. Keep a virtual reference root distinct from its enclosing allocation
and preserve original before/after resource roles; root metadata does not prove initialized storage
or stable geometry.
Extend scoped replacement's [MIR](compiler/crates/nocter-mir/README.md) and
[Machine](compiler/crates/nocter-machine/README.md) geometry qualification to changing
representations, other partial writes and changing evaluator slot shapes. Do not equate source
operation completion with a resource-transition phase. An input's virtual root need not describe
its caller's complete allocation; preserving capacity alone does not stabilize surviving references.
Keep preparation-time snapshots and staged returns; preserve interrupted preparation metadata
without inventing a committed call or materialization permission. Post-update dependent environments
and prepared temporaries must follow actual call-write context rather than reuse an immutable
pre-call bundle. Qualify completed adapter carrier/environment transport through context-changing
source and actual cleanup before widening the qualified context-bound capture scope. Qualify general dependent input carrier
entry/completion transport and remaining dependent post-call channels. Completed-input metadata
now carries structural data and selected callable components; keep the retained original
source-channel correspondence rather than guessing body-local inputs. Materialized edit temporaries now retain separate post-call
sources and use the checked post-operation phase for cleanup metadata; qualify context-changing
carrier transport and storage before widening source admission.
Source-definition boundaries are not resource phases: preserve the existing
transition order, old-value cleanup and new-value receipt without adding provenance to ownership
world identity, a second owner registry or downstream phase inference from step names. Preserve
shared schemas, sparse loops, homogeneous arrays and one canonical normalization authority.

- [ ] Retain scoped replacement's independent retired/supplied obligations and actual alias-root
  selections through evaluator plans and changing partial-carrier geometry. An alias binding's
  immutable snapshot is not a later read of its referent; keep dependent source admission closed.
- [ ] Close carrier representation coherence under the
  [captured callable instance obligations](design/compiler-realization.md#captured-callable-instances)
  before admitting context-changing writes and retained sibling references.
- [x] Qualify same-geometry contextual callable component replacement and restoration with a
  surviving disjoint sibling reference. Carry each partial receipt's original containing roots
  into MIR and physical geometry checks without fusing separate retirement/receipt operations.
- [x] Extend matched containing contracts to evaluated indexed destinations, qualifying unique
  selection and restoration through required/native execution with surviving sibling views.
  Preserve conservative multi-element context joins and original bounds effects.
- [ ] Implement [temporal evaluator slot contexts](design/compiler-realization.md#temporal-evaluator-slot-contexts)
  through ordered plan validation and execution. Preserve one logical storage/owner identity,
  exact retired/received contexts, preparation snapshots, surviving aliases and failure recovery;
  code identity cannot be erased merely because two payloads have equal geometry.
  Qualify ordinary source changes against checking's owning-local registration, retained concrete
  resource phases and evaluator constraints, preserving exact entry contracts. Registration alone
  does not qualify a transition. Then qualify partial carriers and changing external inputs/aliases.
  Preserve separate original retirement and receipt rather than fusing
  them into a replacement solely to accommodate the evaluator.
  Independent closed-plan support does not close ordinary source or native geometry admission.
- [x] Qualify sequential whole-local replacement of one lexical captured callable with distinct
  concrete environment contexts, including growth/shrinkage without surviving references,
  calls before/after replacement, explicit transfer and restoration. Preserve exact retired,
  received and saved instances; this does not qualify changing partial-carrier geometry.
- [x] Extend ordinary tuple formation to valid context-bound callable descriptors. Preserve
  matched per-component code/environment keys through forwarding, taking inputs and read aliases;
  keep Copy obligations, unresolved producers and homogeneous-array admission separate.
- [x] Qualify context-bound callable arrays through ordinary construction, element invocation
  and zero-count seed cleanup. Require one actual homogeneous element context without a
  first-element fallback, invented Copy proof or phantom invocation from an empty array.
- [x] Qualify partial transfer and remaining-container cleanup of context-bound callable
  components. Retain each ordered resource step's actual containing representation rather than
  recovering a local container from its raw formal type; preserve disjoint sibling views.
- [ ] Qualify the canonical frame's received compound-input contexts through ordinary source and
  consumer storage, cleanup and conditional const reuse. The
  [declaration authority](compiler/crates/nocter-declarations/README.md) owns the validated constructor.
  The same code, direct callable bindings and environment can receive compound values with
  different nested callable contexts. Match original input channels to exact canonical data keys;
  do not recover entry metadata from a raw-type default or from the closure's unrelated captures.
  Preserve matched entry/completion selection and distinguish committed reference inputs from
  frozen materialized preparation sources before widening source admission.
- [x] Give each callable leaf of a declared compound input its own declaration-owned contract
  identity without allocating extra physical parameters. Qualify deterministic publication,
  separate same-signature leaves, canonical input metadata and invalid-header rejection before
  opening source admission. Keep nested callback signatures and compound callable results separate.
- [x] Qualify read/take compound inputs through source and consumers: original-source schema matching,
  forwarding across distinct formal component identities, resolved structural runtime identities,
  source safety, conditional const admission and actual native cleanup, including zero-length
  callable-array inputs. Preserve source element metadata without inventing an element value.
- [x] Qualify stable-context edit compound inputs through original completion channels,
  forwarding, caller reads and actual native cleanup. Reconcile receiving/caller descriptors
  through actual component evidence without weakening may-write footprints or weak joins.
- [x] Qualify mutable callback array inputs, evaluated element receivers and forwarded updates
  with independent per-element state, original bounds effects and actual remaining-owner cleanup.
- [x] Qualify compound edit preparations through produced tuples/arrays and explicit owning
  transfers, preserving temporary completion, source retirement and actual post-call cleanup.
- [x] Qualify structural context-changing partial writes through local edit aliases with one
  owning local carrier, preserving sibling views, subsequent reads and actual cleanup contracts.
- [x] Extend local alias replacement receipts to evaluated singleton-array components, preserving
  saved addresses, original bounds effects and actual containing-owner/cleanup metadata.
- [x] Qualify owning callable results extracted from received tuples/arrays, preserving formal
  component identities, actual producer contexts, source retirement and remaining-container cleanup.
- [x] Give declared callable result identities an owning declaration and leaf ordinal, preserving
  the existing root-result domain and rejecting unpublished slots without a producer/value fallback.
- [x] Establish declaration-owned callable result-component contracts and deterministic publication
  without opening compound-result payload admission. Qualify separate same-signature result leaves
  and invalid-header rejection before extending producer/result realization and consumer storage.
- [x] Separate applied callable-result leaf identities from whole-call identities, preserving
  the existing root-result execution domain and rejecting unpublished result slots without
  borrowing a root witness, contract or unproduced-value classification.
- [x] Index the shared producer-equation algebra and selected/unproduced evidence by exact
  result leaf, preserving one whole-call invocation recipe, named/anonymous body identities,
  independent recursive relations and missing-versus-empty producer distinctions.
- [x] Locate published callable-result identities through declaration-owned descriptor paths,
  preserving outcome layers and homogeneous/empty-array metadata without granting values or
  bounds. Qualify nominal-cycle termination and cardinality-independent traversal.
- [x] Qualify declared callable tuple and positive-array results through original return paths,
  per-leaf producer/application relations, direct/function-value calls, taking and recursive
  forwarding, exact result queries, required evaluation, native ABI and actual cleanup. Preserve
  existing anonymous-carrier relations and reject missing/foreign/swapped concrete contexts.
- [x] Qualify uniquely witnessed callable-result components through optional/fallible carriers, preserving absent
  and unproduced alternatives independently of actual returned functions and their cleanup.
- [x] Qualify closed absence-only and failure-only result domains through ordinary source,
  exact declaration/application identities, forwarding, nested optional construction, required
  evaluation, original resource phases and native execution with actual error-graph cleanup.
- [x] Qualify forwarding an excluded callable alternative into an independently closed inhabited
  outcome through original resource, evaluator, ABI and storage channels; matching signatures or
  equal geometry cannot substitute for represented-value transport.
- [x] Define directed guarded representation compatibility alongside exact representation
  equality in the declaration authority. Reject empty roots, narrowing and changed positive
  callable contexts; preserve nominal identities and cardinality-independent array traversal.
- [x] Realize independently closed guarded transport through original source/destination contracts,
  const plans and MIR/native effects for `widened-outcome-results.nct`. Retain both
  representations: widening is not reinterpretation or byte-copy compatibility. Move only active
  payloads under original tags and retire the source obligation once, including actual error graphs.
- [x] Qualify directed symbolic outcome-contract matching and a single-owner reclassification
  record with separate source/destination payload roles, atomic failures and original identity
  checks. Reuse the ownership transition for function adapters and retain fail-closed admission
  for unqualified destination contexts.
- [x] Qualify exact descriptor reprojection from supplied component instances, preserving distinct
  captured environments beside excluded outcome alternatives through forwarding, partial extraction,
  required/native execution and remaining-container cleanup. Keep representation widening separate.
- [x] Give unqualified occurrence-dependent widening an original source-local cause before
  ownership rather than a generic typed-body integrity failure. Keep direct construction supported.
- [x] Distinguish a proven empty representation domain from unavailable evaluator metadata in the
  evaluator's own type vocabulary before guarded transport can use it as a conversion proof.
  Qualify inactive alternatives separately from actual values, slots and input/cleanup obligations.
- [x] Realize guarded represented-value transport in closed evaluator plans with explicit source
  and destination shapes, exact positive callable contexts, inactive-alternative checks, once-only
  retirement and failure/budget recovery, independently of source and native admission proofs.
- [x] Define directed guarded transport over runtime representation metadata without weakening exact
  transfer equality, preserving positive runtime identities and bounded shared/MAX-array traversal.
  This relation grants no executable conversion, source code/context proof, layout or ownership.
- [x] Qualify directed physical outcome transport in runtime representation metadata and MIR/Machine
  effects, preserving separate layouts, original tags and active payload offsets. Keep exact transfer
  equality and checking's positive code/environment proof separate; reject overlapping payload slots.
- [x] Qualify guarded widening from independently closed supplied domains into occurrence-bound
  destination domains selected from actual positive producers and matched consumers. Do not recover
  captured environments from an ambient
  frame, invent code from exclusion or replace the callee's original supplied schema with its target.
- [x] Extend guarded transport from independently closed supplied domains to occurrence-bound
  supplied tuple/array carriers with both positive contexts and excluded alternatives. Preserve exact original
  schemas and actual positive component instances; destination constraints cannot substitute for
  supplied code/environment evidence or authorize context-changing storage and surviving aliases.
- [x] Amortize physical-transport metadata comparison across shared source/destination pairs.
  Share one immutable-environment comparison authority, including exact identities, unknown/cycle
  behavior and bounded array traversal; emission consumes prepared copy/conversion decisions.
- [x] Qualify application-specific empty callable domains supplied through received optional
  inputs. A globally nonempty formal result equation is not a proof that every application
  supplies a callable; preserve the exact committed input context rather than closing its raw type.
- [x] Qualify received excluded/positive outcome metadata through taking inputs, whole-outcome
  return/forwarding, original selection/merge contracts, required/native execution and actual
  error/capture cleanup. Keep missing inputs, unknown producers and positive callable values separate.
- [x] Qualify fresh `none` construction under matched foreign callable-component input constraints,
  including compound primary schemas, exact source-contract checks and native temporary cleanup.
  Missing positive witnesses and unrelated receiving signatures cannot supply this proof.
- [ ] Extend received-domain qualification to instance-specific control/positive-use pruning.
  A source-wide formal contract cannot determine every application's active callable domain;
  retain original tag effects, traps and exact call/resource phases.
  Carry selected predecessor evidence through recovery/result merges and partially active const
  selections; the original source graph must not join an impossible positive payload as a value.
- [x] Qualify a same-witness recovery join from a received callable array with distinct captured
  payloads, both active/absent tags, partial transfers and remaining-owner/error cleanup.
- [ ] Close contextual callable-array construction for mixed fresh absence and positive functions
  under a foreign input contract. `choose([none, live])` currently rejects the positive `live`
  element even when `choose` declares `[(func(i32): i32)?; 2]`; typed existing optionals work.
  Preserve authored context, actual positive witnesses, homogeneous representation and original
  evaluation order. Do not infer from the first element or admit an unqualified carrier layout.
- [x] Qualify the shared checking-owned instance continuation map through optional take/read/edit
  recovery, effectful input production, required/native execution and all affected consumers.
  Preserve dormant contract checks and original commits; unknown domains must retain their branches.
- [x] Qualify mixed independently sourced tuple result leaves through captured environments.
  Preserve one original invocation recipe, each input/result family and actual swapped contexts
  through required/native execution and owned-error cleanup; reject uncaptured/foreign parameters
  and stronger invocation views without using signatures as scope or value evidence.
- [x] Qualify mixed independently sourced results through success-wrapped tuple arrays, including
  an absent second row, scalar offsets, partial extraction and actual owned-error cleanup.
  Preserve each input/result family, homogeneous per-leaf context and exact instance through
  required/native execution without erasing positive domains or inventing a foreign context.
- [x] Share metadata fixed-point dependencies within value/preparation batches using complete
  frame/source/path/type keys. Qualify the combined constructed-environment source under the
  unchanged default work quota, charge solved hits and discard interrupted drafts before retry.
- [x] Qualify constructor-backed exclusion through tuple, positive tuple-array and nested
  success/optional results. Preserve each context at a shared lexical descriptor, array homogeneity,
  scalar offsets, explicit move/restoration, active component extraction/invocation, remaining-owner
  cleanup and actual owned-error environments through required/native execution.
- [x] Qualify positive inactive outcome contexts inside owned environments through original source
  result closure and declaration-owned result-family projection. Preserve distinct nested environments,
  exact owner/leaf/view checks, representation-versus-receipt distinctions, required execution,
  original resource phases and native cleanup without adding provenance to instance identity.
- [x] Carry constructor-backed active-value exclusion through original return/application equations
  separately from canonical uninhabited representation evidence. Qualify forwarding and cleanup,
  preserve unknown-input rejection and original tag/trap effects, and require actual per-instance
  input/result keys before claiming layout or storage support.
- [x] Retain explicit constructed outcome tags in the shared producer fact DAG, separating missing
  facts from described unknowns. Qualify source creation snapshots, conservative joins/weak updates
  and bounded shared-schema traversal without granting ownership, code or represented-domain facts.
- [x] Retain explicit active-callable absence in each original invocation input summary alongside
  its positive producer alternatives. Merge only matching original inputs, distinguish missing/empty
  alternatives from a constructed exclusion, and keep represented-domain selection and ownership separate.
- [x] Preserve actual supplied function identity when lifting into optional/fallible compound-input
  constraints, including nested outcomes and tuple components. Share constraint selection between
  construction and input preparation; qualify actual required/native invocation and wrong-contract rejection.
- [x] Qualify statically pruned and ordinary conditional outcome inputs through original commit
  recipes and supplied/formal domain reprojection. Preserve dormant descriptor checks separately
  from committed empty-domain and original merge evidence; retained source effects remain intact.
- [x] Define canonical uninhabited representation metadata independently of unknown descriptors,
  concrete callable witnesses and actual owner availability. Normalize required product children,
  retain inactive outcome alternatives and reject an uninhabited environment/input as a received value.
  Qualify metadata classification and fail-closed owned-value/storage boundaries.
- [x] Transport independently closed outcomes with uninhabited alternatives through guarded lifecycle,
  evaluator, target storage, ABI and native execution. An empty root has no stored payload;
  retaining an inactive alternative's descriptor must not create code, cleanup or field geometry.
  Preserve original tag guards, rejected positive uses and the explicit source-retirement decision.
- [ ] Qualify remaining instance-specific recovery, propagation and assertion continuations when
  a received primary callable domain is empty.
  Preserve evaluation of the original carrier/tag, required traps and alternate cleanup while
  excluding impossible positive continuations; a guarded empty descriptor is not a callable value.
- [x] Qualify received take/read/edit assertions through original const trap diagnostics and
  native safety traps. Share assertion-source recognition with definition-level flow refinement;
  preserve whole-carrier evaluation and exclude only the impossible normal continuation.
- [x] Qualify received optional propagation through take/read/edit inputs and fallible value
  propagation/recovery with actual owned errors and captured environments. Preserve staged
  absence/error results, caller-owned projection restrictions and exactly-once cleanup.
- [x] Qualify value/read/edit recovery and value propagation of excluded primary callable
  domains through the shared original flow graph, const obligations and native error cleanup.
- [x] Qualify value/read/edit assertions of excluded primary callable domains through original
  const trap diagnostics and actual native safety traps, preserving earlier exits and inhabited
  counterparts. Represent check continuations in the original graph before domain refinement;
  do not truncate actions, move identities or invent a positive payload after a guaranteed trap.
- [x] Qualify excluded-outcome runtime descriptors, canonical target interning and physical layouts
  without a positive child or field geometry. Reject empty roots/slots and construction of excluded tags.
- [x] Extend the declaration-owned data-instance fold to validated callable domains, preserving
  typed leaf keys, empty alternatives, shared schemas and positive witness checks. Reuse that one
  fold for ordinary witness instantiation instead of adding a checking-only structural normalizer.
- [ ] Eliminate tags for singleton represented outcomes after qualifying ABI, addressability,
  guarded transport and context-changing storage. Derive this from the canonical representation
  domain rather than a second source-presence analysis or a geometry-only assumption.
- [x] Qualify compound returns with empty-array callable descriptors, including mixtures with
  inhabited result leaves, ordinal gaps and forwarding between declaration-owned descriptors.
  Uninhabited metadata must neither create an element equation nor require a concrete producer.

Qualify the same lexical closure with different captured callbacks before and after forwarding
it with another instance, including safe/unsafe ordinary source, conditional const admission,
evaluator plans, native storage and actual cleanup. Extend source admission only when all consumers
agree. A parser, metadata construction or emitted-only check cannot close that
boundary. Opaque effects, ambiguous alternatives and unqualified writes must remain unresolved.

Preserve missing/foreign evidence rejection, frozen views, anchored recursion, deterministic work
failure and healthy retry. Keep dormant definition-contract hypotheses, factored symbolic conditions
and condition/plan quotas distinct from actual committed calls and pre-commit preparation. Semantic
TypeId remains the source/checking authority; contextual runtime IDs grant no fabricated semantic
type, source receipt, payload, layout or cleanup proof. Follow the occurrence-sensitive finding below.

Continue symbolic value/resource-state guarded discharge and independent module publication after
that boundary is coherent. General R/D/S, dependent carriers, deeper callable inputs, inferred
captures, module/toolchain integration and retained-selection closure remain open.

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
  canonical witness/data instance authority. Extend the
  [qualified capture scope](compiler/crates/nocter-checking/README.md#context-bound-callable-captures)
  only after these transport and cleanup cases are qualified; see the
  [instance obligations](design/compiler-realization.md#captured-callable-instances).

- [ ] Extend symbolic discharge beyond covered root-closed named/owned-environment witnesses and
  parameter-selected calls without callable receiving dependencies. Free/unresolved captures,
  compound dependent data and value/resource-state guards retain exact-frame checks. Preserve
  matched body/type receipts, receiving operations, actual cleanup, exact supplying-argument causes
  and source safety.
  Work quotas bound uncertainty without proving eligibility; module publication retains its
  independent qualification.
  Keep dormant definition-contract checks separate from source-value receipts. Preserve body
  expressions' original operand and result-definition selections. Qualify general dependent
  result/carrier contexts through actual consumer paths before widening source admission.
  Preserve explicit drops' original pre-retirement selections and dormant definition-contract
  checks. Preserve retained ordered cleanup selections through shared resource-data
  projection when one action retires and receives the same raw type; a diagnostic type/site is not
  value provenance. Qualify changing cleanup contexts through ordinary source and all consumers.
  Preserve exact received-input
  probes and exact compound-context traversal/discovery rather than same-typed capture defaults.
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
