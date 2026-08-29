# Incremental Computation Boundary

This document owns the cross-crate computation contract that turns immutable source revisions into
compiler and editor products. Compiler stages remain the sole semantic authorities; the computation
layer may schedule, memoize, validate, and reuse their products but cannot implement a language
rule.

## Target Model

```text
workspace source revision
        |
        v
frozen workspace plan
        |
        v
revision-pinned computation snapshot
        |
        +--> parse(source)
        +--> module surface(module)
        +--> declaration graph(scope)
        +--> resolve body(body)
        +--> check body(body)
        `--> validate target(scope)
                    |
                    v
          semantic evidence + source projection
                    |
                    v
              analysis queries
```

The computation snapshot is a lazy view over one accepted input revision, not a requirement to
materialize every compiler product eagerly. A command may demand a target product; an editor query
may demand only the semantic capability it needs. Both paths invoke the same stage providers.

## Authority Boundaries

- `nocter-workspace-revision` owns open documents, monotonic revisions, and immutable overlays.
- workspace analysis owns physical path interpretation, package/module/single-file scope selection,
  and the frozen plan for one revision.
- the computation owner owns query keys, dependency recording, memoization, invalidation, cycle
  reporting, and reuse accounting. [`nocter-computation`](../compiler/crates/nocter-computation/README.md)
  implements this mechanism without depending on a compiler-domain crate.
- declaration lowering, checking, target construction, and later backend stages own semantic rules
  and return immutable products.
- session composition owns the only production and recovery stage order. One analyzed unit owns its
  discovery snapshot and exact session result inseparably.
- analysis owns the validated join between semantic evidence and source projection. It does not
  invoke compiler stages or inspect computation storage.
- protocol layers consume typed analysis results only.

## Identity

Physical paths are resolved before the computation boundary. Queries use opaque source, package,
module, declaration, and body keys. A query key is stable only in the domain stated by its owner;
generation-local arena IDs never become cross-revision cache keys.

Reusable module products may retain their own dense IDs. A dependent product can reuse those IDs
only while it retains the exact immutable owner product. Pointer identity alone is not a semantic
fingerprint.

## Dependency and Reuse

Dependencies are recorded by query evaluation. Feature code and workspace orchestration do not
maintain parallel downstream invalidation lists. A changed input makes its direct query dirty. A
dirty query is reevaluated only when demanded; if its deterministic output fingerprint remains
unchanged, invalidation does not propagate to dependents.

Examples:

- changing a function body invalidates that source parse and the affected body computations;
  unchanged module surfaces and unrelated bodies remain reusable;
- changing a public declaration invalidates the module surface and every computation that actually
  consumed it;
- changing an ordinary comment can update source projection without invalidating semantic
  dependents when their product fingerprints remain equal.

Every reuse decision is observable in tests through computation counters. Correctness tests compare
a warm incremental result with a fresh computation from the same final source revision.

The first integrated vertical slice uses stable canonical-path identities for a two-step
`source_text(path) -> parse(path, goal)` chain. Workspace revision publication separates overlay
membership, the bytes of each open source, and a filesystem-change epoch into typed inputs. An
ordinary edit therefore dirties only that source's text query. A filesystem notification may dirty
disk-backed sources, and opening or closing a document may change source authority, without making
editor text and filesystem state parallel semantic authorities.

`nocter-syntax` owns the reusable parse product and alone knows how to bind it into the current
`SourceMap` identity domain. Package topology, graph loading, and discovery consume the same narrow
contract and validate the bound text before identity rebinding. A package-root probe retains its
parse product for graph loading, eliminating the prior same-revision second parse. Unchanged
package roots and implementation sources avoid lexing and parsing across workspace revisions
without allowing a prior generation's `SourceId` to escape into the current snapshot. Speculative
mutation validation uses an isolated database populated from the candidate overlay; it cannot
pollute or accidentally read the accepted revision.

The next integrated boundary canonicalizes declaration-relevant syntax while pruning every
function or method block. Node kinds, non-body tokens, body presence, missing syntax, and
declaration-local diagnostics remain in that product; source identities, coordinates, trivia, body
contents, and body-local diagnostics do not. A module-surface query composes those source products
in canonical physical-source order. Instrumented tests prove that a body edit reevaluates the
edited source surface but leaves its fingerprint unchanged, so the containing module surface is
reused without execution. Declaration and body-program migration remains incomplete; the current
body-program pipeline still runs after this boundary and is not mislabeled as incremental.

The syntax owner also assigns source-independent locators to nodes and tokens retained by that
surface. An equal surface can resolve the same locator into its own generation-local syntax
identity. Block nodes are locatable, but descendants of a block are never assigned declaration
locators. This is the only permitted bridge for rebinding a reusable declaration product to a
current source projection; semantic products cannot retain `SourceId`, `NodeId`, token ranges, or
arena offsets from the generation that first produced them.

Declaration lowering now emits one source-neutral projection recipe rather than independently
building semantic bindings and editor indexes. The recipe pairs semantic identities with surface
source ordinals and syntax locators. One current-generation interpreter produces both
`FrontendBindings` and `SourceIndex`. It reads documentation from the current syntax tree instead
of retaining Markdown in the recipe, because documentation does not invalidate declaration
semantics. Body-local imports are deliberately supplied as current body projection and never enter
the reusable declaration recipe. The accepted declaration authority can create an owned checking
branch while preserving its semantic IDs and type-authority lineage; query ownership of that
authority is the next integration step. `ReusableDeclarations` is the source-neutral ownership
unit for that query: it contains the accepted authority, primitive bindings, and recipe, while the
ordinary lowering result keeps current `FrontendBindings` and `SourceIndex` outside it.

The recipe also freezes the complete source-neutral source domain and the exact mapping from
physical module identities to declaration-program module IDs. Rebinding a reused declaration
result canonicalizes only the current compile input's source views, proves that their module,
canonical path, and source kind match the recipe, resolves stable syntax locators, and attaches
current body-local imports through that frozen module mapping. It does not call declaration
collection or topology preparation. A mismatched or incomplete current projection is an integrity
error rather than a partially usable editor index.

The session-owned analyzed unit retains the exact immutable discovery snapshot through shared
ownership. A declaration query and its final analysis result can therefore borrow the same object;
neither side clones a graph containing generation-local IDs or reconstructs discovery from paths.

Discovery additionally freezes one source-neutral semantic topology surface. It canonicalizes the
selected target, package identities and dependency aliases, module/source membership, top-level
`see` and `use` resolutions, package targets, and toolchain attachments. Source contents remain
owned by source/module surface queries, while body-local imports remain body inputs. The encoding
uses stable vocabulary supplied by the owning model contracts, validates every retained resolution
kind, and is independent of discovery traversal order. The declaration query composes this
topology product with module surfaces instead of rerunning discovery or interpreting its private
storage.

That composition is now integrated through the dedicated `nocter-semantic-computation` owner.
Workspace orchestration publishes two fingerprints carrying the same shared discovery snapshot:
declaration semantics combine canonical topology with all module surfaces, while exact current
source additionally includes every reached canonical path, normalized byte sequence, and
`SourceId` layout. The declaration query reads only the
semantic input on acceptance. It dynamically reads exact current source on rejection, preventing
generation-local failure evidence from surviving any edit without invalidating successful
declarations. Session checking consumes an owned branch of an accepted query result and its freshly
materialized projection; it does not invoke declaration lowering again.

The declaration program's `SymbolTable` follows the same invalidation boundary. Declaration and
type spellings form a stable prefix fixed by declaration surfaces. Identifiers and decoded string
literals beneath function or method blocks are absent from that reusable prefix. Every checking
request deterministically appends the exact current body's missing spellings to its owned program
branch. Existing symbol IDs therefore remain valid across a body edit, while a newly authored
local, member name, body type name, block import alias, or string value is available to the current
checker without widening declaration invalidation.

The body-query input boundary now exists independently of checking. Syntax pairs every executable
block pruned from a declaration surface with that block's stable declaration locator and exact
normalized bytes. Workspace orchestration reads those body surfaces from the same source-surface
product already demanded for module composition, then atomically publishes inputs keyed by
canonical physical path plus locator. A sibling-body edit therefore leaves an unchanged body's key
and fingerprint equal. Lexical and typed-body queries now consume that same input; publication
alone is never counted as incremental checking.

Program-wide checking preparation now consumes the accepted declaration query before body work.
Its successful product contains only the stable declaration graph and immutable checking
authorities. `ProgramEnvironment` cannot retain `SourceAccessTable`, and its declaration graph
contains no body-only symbol suffix. Current declaration projection computes that suffix once;
session opens a graph branch and pairs current source access immediately before name resolution.
Consequently a body-only edit reuses interface, copyability, drop, construction, instance, and
capability preparation without allowing a prior generation's source identities into the result.
Program-wide authored preparation rejection is an exact-current query value. Checking captures a
closed rule category rather than cloning arbitrary internal errors, pairs it with declaration-only
recovery, and retains optional interface-method repair evidence. Session opens a new owned branch
from that value. Missing inputs and internal preparation inconsistencies remain unavailable, so a
computation-kernel absence cannot masquerade as authored source rejection.

Lexical resolution now consumes the published body inputs through one query per stable
path-plus-declaration locator. An accepted result is converted immediately into a source-neutral
recipe: local and capture IDs remain body-local semantic identities, symbol IDs become spellings,
and every node/token origin becomes an ordinal locator within the exact body. Session resolves
those locators against current syntax and extends `SourceIndex` once for the complete canonical
body arena. A private exact-current context query materializes frontend bindings and the current
symbol branch once per revision. Although its value refreshes with current source, its successful
fingerprint is the source-neutral declaration authority; only body queries whose exact body input
changed execute against the refreshed context. Lexical and typed queries share that context without
sharing either result; a current `NodeId` product cannot claim its source-neutral fingerprint.
Authored lexical rejection is also a query value. The diagnostic remains bound to the exact current
source fingerprint, while the resolved prefix is captured with the same source-neutral locator
recipe as success. The complete body-name set owns both accepted recipes and rejections. One
canonical materialization extends `SourceIndex` and constructs `NameAnalysisRecovery`; session
does not run the resolver again. Internal resolution or projection inconsistency remains
unavailable rather than becoming an authored rejection.

Typed-body reuse cannot retain the existing program-wide `TypeId` and `ClosureId` allocation order.
A preceding body may add an inferred structural type or closure and shift every later identity.
Checking therefore now owns a body type-extension recipe: references into the immutable prepared
program remain program identities, while types and closures introduced by the body use dense local
identities. Replay interns them into the canonical finalization branch and returns the exact
local-to-current map. Body construction now opens every body from that same prepared semantic
prefix and an empty closure domain; it no longer observes semantic additions from earlier siblings.
Successful closure definitions and structural types replay in canonical body order, and one closed
rebinder rewrites the checked body, nested selections, places, generic arguments, iteration plans,
and opaque witnesses. The checked graph is captured before publication with no current source or
symbol identity. References, `BodyNodeId` origins, and associated-type completion sites share one
ordinal body-syntax recipe. Local and capture declarations are not copied into that product:
canonical replay joins their types with the already materialized lexical recipe. Session consumes
the complete query-owned body set and does not run body checking again. Canonical `BodyId` replay
remains the sole allocator of final program type and closure identities; ownership, provenance,
and loans still run once after the complete canonical body set.

Authored typed-body rejection is also a query value. Its diagnostic and interruption capability
remain in the exact current source domain, while independently successful siblings retain their
source-neutral recipes. Session replays those successes and assembles the canonical rejected-body
arena once; it does not rerun checking to reconstruct recovery. An internal checker or projection
failure is unavailable rather than being mislabeled as authored rejection.

Whole-program finalization is an exact-current query above the complete body-name and typed-body
sets. It replays body-local type and closure recipes in canonical order, then computes ownership,
provenance, loans, opaque witnesses, and checked semantic completion once. The query reuses the
private body semantic context, so current declaration projection is not materialized again. Its
success and `BodyCheckFailure` values both open owned consumer branches; session cannot invoke body
replay or finalization. If complete typed coverage exists but finalization is unavailable,
workspace analysis reports an integrity error instead of repeating the compiler stage.

The same query owns the alternative complete lexical-rejection branch. It materializes the full
body-name catalog into one exact-current `NameAnalysisRecovery` and retains the authored diagnostic
beside it. Workspace orchestration demands this final query after accepted preparation and session
only translates the resulting branch. Workspace does not transport the intermediate body-name or
typed-body sets, and there is no session entry point that can replay them through compiler stages.

For source-complete input, `Unavailable` always means the query graph failed to produce a required
authority. Workspace analysis reports that state as an integrity error. It never restarts from
declarations or a prepared program in session, because such a restart would duplicate dependency
selection and conceal the missing query edge.

Rejected declarations retain their complete diagnostic and recovery authority inside that exact
current-source identity domain. Session clones the query-owned recovery branch and continues editor
analysis from it; it never performs a second declaration traversal. Reuse is valid only when the
canonical source identity layout and bytes are identical, so generation-local syntax identities
cannot be joined to a different current source domain.

## Source Projection

A stage product carries its semantic result, diagnostics, deterministic fingerprint, and source
projection contribution together. The semantic model remains source-independent, but stage APIs do
not provide a success path that silently drops the corresponding authored projection. The finished
`SourceIndex` remains an independent immutable relation and retains its whole-generation integrity
seal as defense in depth.

## Recovery

Complete syntax uses the semantic query graph described above. Incomplete syntax still uses one
explicit editor-only session admission while its recovered declaration/body result is migrated into
the query graph; it cannot claim compilation success. Recovery does not create feature-local
fallback order. Rejected name and body domains, typed interruptions, diagnostics, and coverage are
cacheable products only when their source justification is complete. Internal inconsistency is
never cached as authored recovery.

## Deferred Facilities

The first computation phase does not require background execution, request cancellation, persistent
disk caches, remote indexes, or parallel backend work. Those facilities may be added only after the
dependency graph and snapshot contract are proven. They must not introduce another semantic
authority.
