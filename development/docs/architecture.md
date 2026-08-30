# Specification-First Compiler Architecture

This document owns the compiler-wide pipeline, dependency direction, and contracts that cross crate
boundaries. It does not define Nocter language behavior and does not describe a crate's private
module structure. Public behavior belongs to the [language specification](../../spec/README.md);
crate-local design belongs to the relevant colocated `README.md`.

## Program Pipeline

```text
SourceProgram
  -> SyntaxProgram
  -> DiscoverySnapshot / CompileUnitInput
  -> DeclarationProgram
  -> AcceptedDeclarationProgram
  -> CheckedProgram
  -> TargetProgram
  -> ExecutableProgram
  -> MirProgram
  -> MachineProgram
  -> Arm64Program
  -> MachOImage
```

Each arrow is a one-way lowering boundary. The producer decides and validates its own facts once.
The consumer receives identities and closed decisions through the producer's public contract; it
cannot recover a decision by reading source text, traversing an earlier representation, rendering a
name, or depending on insertion order.

The primary owners are:

| Boundary | Owner |
|---|---|
| normalized source and coordinates | [`nocter-source`](../compiler/crates/nocter-source/README.md) |
| lexical and syntactic structure | [`nocter-syntax`](../compiler/crates/nocter-syntax/README.md) |
| package source discovery | [`nocter-discovery`](../compiler/crates/nocter-discovery/README.md) |
| closed compilation input | [`nocter-compile-input`](../compiler/crates/nocter-compile-input/README.md) |
| immutable declaration model | [`nocter-declarations`](../compiler/crates/nocter-declarations/README.md) |
| syntax-to-declaration lowering | [`nocter-declaration-lowering`](../compiler/crates/nocter-declaration-lowering/README.md) |
| typed semantics and ownership | [`nocter-checking`](../compiler/crates/nocter-checking/README.md) |
| target validation and executable closure | [`nocter-target-program`](../compiler/crates/nocter-target-program/README.md) |
| concrete semantic control flow | [`nocter-mir`](../compiler/crates/nocter-mir/README.md) |
| target-independent machine operations and ABI | [`nocter-machine`](../compiler/crates/nocter-machine/README.md) |
| ARM64 selection and encoding | [`nocter-arm64`](../compiler/crates/nocter-arm64/README.md) |
| Mach-O image construction | [`nocter-macho`](../compiler/crates/nocter-macho/README.md) |

The [checked-program](checked-program-design.md),
[target/executable/MIR](target-program-design.md), and
[machine/native](machine-program-design.md) documents define only the contracts spanning adjacent
owners. The crate READMEs own internal responsibility splits and invariants.

## Side Authorities

Some responsibilities accompany the main lowering pipeline without becoming semantic stages:

```text
filesystem overlay ---------> package/discovery/session input
source projection <---------- lowering and checking identities
diagnostics <---------------- source-backed failures from every stage
analysis queries <----------- semantic evidence + source projection
workspace analysis ---------> immutable editor generations
language server ------------> protocol projection only
command/native session -----> pipeline orchestration and artifact publication
```

Their owners are:

| Responsibility | Owner |
|---|---|
| immutable disk/open-document view | [`nocter-filesystem`](../compiler/crates/nocter-filesystem/README.md) |
| accepted editor source revisions | [`nocter-workspace-revision`](../compiler/crates/nocter-workspace-revision/README.md) |
| revisioned dependency evaluation and reuse | [`nocter-computation`](../compiler/crates/nocter-computation/README.md) |
| semantic identity to source projection | [`nocter-source-index`](../compiler/crates/nocter-source-index/README.md) |
| phase-neutral diagnostics | [`nocter-diagnostics`](../compiler/crates/nocter-diagnostics/README.md) |
| compiler session composition | [`nocter-session`](../compiler/crates/nocter-session/README.md) |
| protocol-independent semantic queries | [`nocter-analysis`](../compiler/crates/nocter-analysis/README.md) |
| workspace revisions, topology, and compilation demand | [`nocter-workspace-analysis`](../compiler/crates/nocter-workspace-analysis/README.md) |
| LSP lifecycle and result projection | [`nocter-language-server`](../compiler/crates/nocter-language-server/README.md) |
| protocol data model and codec | [`nocter-lsp`](../compiler/crates/nocter-lsp/README.md) |
| CLI command planning | [`nocter-command`](../compiler/crates/nocter-command/README.md) |
| native backend orchestration | [`nocter-native-session`](../compiler/crates/nocter-native-session/README.md) |

A side authority cannot become a second semantic pipeline. In particular, source projection may
locate an already selected identity but cannot decide type equality, lookup, dispatch, ownership,
reachability, ABI, or code generation. Protocol code cannot inspect compiler storage to recreate a
query, and orchestration code cannot implement a stage's validation rule.

## Identity and Authority Rules

- Semantic identity domains are syntax-independent and owned by
  [`nocter-model`](../compiler/crates/nocter-model/README.md).
- Public language vocabulary and closed language constants are owned by
  [`nocter-language`](../compiler/crates/nocter-language/README.md).
- Runtime primitive and representation identities are owned by
  [`nocter-runtime-contract`](../compiler/crates/nocter-runtime-contract/README.md).
- Toolchain-selected declarations and standard roles are owned by
  [`nocter-toolchain-contract`](../compiler/crates/nocter-toolchain-contract/README.md) and projected
  through [`nocter-frontend-bindings`](../compiler/crates/nocter-frontend-bindings/README.md).
- Persistent semantic storage is an implementation facility owned by
  [`nocter-persistent`](../compiler/crates/nocter-persistent/README.md); only the semantic owner may
  expose domain-specific transactions.
- A raw ID has meaning only with the immutable program or authority that owns its generation.
- Accepted products are complete values. A rejected product exposes only explicit recovery evidence
  justified by its source diagnostic.
- A builder is the sole mutation path for its product. Freezing validates every cross-identity edge.

Supporting boundary crates remain deliberately narrow:

| Responsibility | Owner |
|---|---|
| syntax-owned target-gate decisions | [`nocter-target-selection`](../compiler/crates/nocter-target-selection/README.md) |
| compile-time expression evaluation | [`nocter-constant-evaluation`](../compiler/crates/nocter-constant-evaluation/README.md) |
| source-only formatting and inspection | [`nocter-source-tooling`](../compiler/crates/nocter-source-tooling/README.md) |
| deterministic JSON and hashing | [`nocter-json`](../compiler/crates/nocter-json/README.md) and [`nocter-hash`](../compiler/crates/nocter-hash/README.md) |
| outer process boundary | [`nocter-cli`](../compiler/crates/nocter-cli/README.md) |
| shared test fixture construction | [`nocter-test-support`](../compiler/crates/nocter-test-support/README.md) |
| whole-pipeline conformance tests | [`nocter-conformance`](../compiler/crates/nocter-conformance/README.md) |

## Package and Installation Boundary

Package interpretation and package-state mutation are separate:

| Responsibility | Owner |
|---|---|
| package declarations, roots, exact selections, and resolved graphs | [`nocter-package`](../compiler/crates/nocter-package/README.md) |
| exact-package cache representation and content verification | [`nocter-package-cache`](../compiler/crates/nocter-package-cache/README.md) |
| root dependency-source transitions and exact-package cache publication | [`nocter-package-state`](../compiler/crates/nocter-package-state/README.md) |
| authenticated Git/archive acquisition | [`nocter-package-acquisition`](../compiler/crates/nocter-package-acquisition/README.md) |
| installed toolchain validation | [`nocter-installation`](../compiler/crates/nocter-installation/README.md) |

Resolution consumes an immutable filesystem view. Acquisition and package-state publication cannot
run through an editor overlay. Root dependency-source commit is failure-atomic; each dependency's
source-specific `commit` or `sha256` field is its sole selection authority. Validated exact packages
publish independently into an append-only cache, so a cache entry may remain after a later
root-source rejection without changing the selected graph. Acquisition seals each staged tree with
one deterministic content manifest. Publication and later resolution use the same verification
contract, so a changed cache tree cannot retain its exact identity. An interrupted acquisition
cannot expose a partial exact package, and an interrupted installation cannot expose a partial toolchain. Package
display names never replace canonical package identities. Workspace topology freezes a
revision-local package-root catalog; package loading and discovery extend that catalog without
reopening a root already selected from the same overlay.

## Editor Generation Boundary

One accepted document event is admitted by `nocter-workspace-revision` and produces one immutable
workspace revision. Workspace analysis freezes
topology and compilation demand for that revision, then produces one analysis snapshot per selected
scope. A changed or closed document may invalidate active demand but cannot silently become demand.

One compiler session produces one semantic-evidence value. Successful and recovered evidence are
exclusive variants, not independent optional fields. Analysis joins that evidence with a validated
source projection once and exposes typed query results. Features depend on the capability they need,
not a phase ordinal. Set-valued queries state whether coverage is complete; mutations require a
validated complete candidate before publication.

The [semantic presentation boundary](semantic-presentation-design.md) owns compiler-to-editor
rendering rules. The completed
[semantic tooling reconstruction](../reviews/v0.18.0-semantic-tooling-reconstruction.md) records why
the current authority shape replaced the older sparse recovery model.

The [incremental computation boundary](incremental-computation-design.md) owns the active migration
from eager scope recompilation to revision-pinned dependency queries. It does not change the
semantic authority or editor presentation contracts above.

## Dependency Enforcement

`development/compiler/Cargo.toml` is the sole workspace-membership authority. Crate manifests own
exact dependency edges. Executable architecture tests validate reviewed production dependencies,
while compiler-resolved Clippy restrictions prevent prohibited types or construction methods from
crossing owner boundaries through aliases or re-exports.

The production graph must obey these rules:

- source and syntax do not depend on semantic stages;
- semantic model does not depend on source or syntax;
- checking cannot depend on target, MIR, machine, native encoding, analysis, or protocol crates;
- target and MIR cannot inspect syntax or repeat name/type/dispatch selection;
- machine and native emitters cannot inspect semantic declaration or checking storage;
- language-server code cannot depend directly on compiler semantic storage;
- test support and conformance may compose production contracts but cannot supply production
  behavior.

The architecture review rejects compatibility imports, reverse semantic lookup from presentation,
parallel registries for the same identity, source-order tie-breaking, feature-local recovery joins,
and wrappers whose only purpose is to bypass an owner contract.

## Documentation Boundary

This document changes only when a pipeline edge, cross-crate authority, or dependency rule changes.
A crate-internal refactor changes that crate's README. A public behavior change changes `spec/`. A
temporary implementation plan changes a milestone. Review findings and remediation evidence belong
in `development/reviews/`; release qualification belongs in `development/releases/`.
