# Nocter Development Documents

This directory contains compiler and distributed-standard-library implementation design. The public
language [specification](../../spec/README.md) is the sole authority for source behavior and public
APIs. Implementation documents explain responsibility, data flow, invariants, and verification;
they must link to a specification rule instead of restating it.

Candidate scope and qualification live in `development/milestones/`. Frozen release evidence lives
in `development/releases/`. Short-lived handoff state lives only in `development/TODO.md`.

## Documents

- [Packages, Dependencies, and Locks](packages.md): package files, semantic identities,
  dependency graphs, exact locks, stores, and compiler boundaries
- [Immutable LSP Snapshots](lsp-snapshots.md): editor generations, package contexts, source
  overlays, invalidation, and request consistency
- [Body-Bearing Interface Implementations](interface-conformances.md): canonical conformance
  member identity, lookup, validation, migration, and editor boundaries
- [Destruction Declarations](destruction-declarations.md): independent destructor identity,
  type-family uniqueness, cleanup integration, and editor traversal
- [Path-Sensitive Aggregate Cleanup](control-flow-drop-state.md): promoted runtime live state,
  ownership transitions, control-flow joins, and common cleanup lowering
- [Construction Surfaces](construction-surfaces.md): compiler-owned construction entries,
  default selection, lowering reuse, and editor boundaries
- [Borrow Coercions](borrow-coercions.md): type-owned view declarations, contextual plans,
  ownership, lowering, and editor boundaries
- [Equality Operators](equality-operators.md): fixed instance-owned equality, structural generic
  requirements, coercion selection, plans, lowering, and editor identity
- [Strict Ordering Operators](ordering-operators.md): shared fixed-comparison selection, derived
  token orientation, evaluation order, standard lexical ordering, and editor identity
- [Index Selection and Lowering](indexing.md): source-defined index declarations, structural
  requirements, coercion selection, immutable typecheck plans, and native evaluation order
- [Built-in Type Source Surfaces](builtin-type-surfaces.md): installed construction, instance,
  and conformance authority, receiver plans, trusted representation primitives, and editor identity
- [Architecture](architecture.md): compiler phase responsibilities and boundaries
- [Region, Provenance, and Allocation Context](region-provenance.md): shared storage-origin,
  allocation-effect, lexical-region, and lowering design
- [Typed Literals and Composable Element Packs](typed-literals.md): literal shapes, definitions,
  spread segments, context selection, and lowering boundaries
- [Explicit Iteration and Collection Access](iteration.md): readonly and owned iterator invariants,
  optional access, shifting, cleanup, and LSP boundaries
- [Owned String Interpolation and Formatting](interpolation.md): runtime declarations, semantic
  plans, formatting policy, lowering, cleanup, and LSP boundaries
- [Public Provenance Contracts and Generic Interface Bounds](provenance-contracts.md): explicit
  result origins, generic capability lookup, static specialization, and editor boundaries
- [Nested Outcomes and Executable Process Context](outcomes-process-context.md): recursive callable
  result channels, native ABI, process storage, and ambient/recoverable process APIs
- [First-Class Outcome Values](outcome-values.md): recursive stored outcome layout, callable
  bridging, active-payload ownership, consumers, and tooling
- [Catch Recovery Lowering](value-producing-catch.md): value-producing failure handlers,
  destination joins, cleanup, nested outcomes, and editor facts
- [Protocol-Driven Collection Iteration](iteration-protocol.md): trusted protocol roles, collection
  conversion, loop ownership, cleanup, and editor boundaries
- [Composable Iterators and Collection Builders](iterator-composition.md): capability sets,
  conditional conformance, adapter state, collection construction, and editor boundaries
- [Callable Values and Interface Default Methods](callable-default-methods.md): method generics,
  required/default methods, closure ownership, callable specialization, and iterator chains
- [Generic Requirement Architecture](generic-requirements.md): authored and resolved requirements,
  intrinsic copyability, specialization, and editor integration
- [Associated Type Identity and Projection Normalization](associated-types.md): interface-owned
  member identity, conformance bindings, normalization, and editor integration
- [Static Opaque Result Types](opaque-result-types.md): declaration identity, hidden witness
  elaboration, public interface view, and concrete lowering view
- [Allocator and Ownership](allocator-ownership.md): the shared allocation, ownership, partial
  initialization, `String`, and `Vec<T>` foundation
- [Standard Library](standard-library.md): runtime responsibilities, ownership invariants, target
  boundaries, and distributed-home verification
- [LSP](lsp.md): compiler-backed semantic, presentation, recovery, protocol, and verification
  boundaries
- [Documentation Site Generation](site-generation.md): public Markdown build and generated output
- [Test Ownership and Integration Boundaries](testing.md): layer ownership, redundancy rules,
  distributed-home scope, and verification profiles
- [Maintenance](maintenance.md): update ownership, verification, and commit policy
- [TODO](../TODO.md): internal short-term handoff state
- [Active Milestones](../milestones/README.md): candidate scope and qualification
- [Release Qualification Records](../releases/README.md): frozen compiler release evidence

## Information Ownership

| Information | Owner |
|---|---|
| Public language rules | `spec/` |
| Runnable user programs and packages | `examples/` |
| Compiler source-corpus fixtures | `../compiler/tests/fixtures/source_corpus/` |
| Published downloads and public release status | `releases/README.md` |
| Candidate scope, status, and qualification | `../milestones/<version>.md` |
| Published release qualification | `../releases/<version>.md` |
| Package/compiler responsibility boundary | `packages.md` |
| Package-wide editor state and invalidation | `lsp-snapshots.md` |
| Compiler responsibility boundaries | `architecture.md` |
| Region, provenance, and allocation-context implementation design | `region-provenance.md` |
| Typed literal and ephemeral element-pack implementation design | `typed-literals.md` |
| Explicit readonly/owned iteration and collection access design | `iteration.md` |
| Owned interpolation and formatting implementation design | `interpolation.md` |
| Public provenance contracts and generic interface-bound dispatch | `provenance-contracts.md` |
| Nested outcome lowering and executable process context | `outcomes-process-context.md` |
| First-class stored outcome values | `outcome-values.md` |
| Value-producing catch analysis and lowering | `value-producing-catch.md` |
| Protocol-driven collection iteration | `iteration-protocol.md` |
| Composable iterators and collection builders | `iterator-composition.md` |
| Callable values, interface default methods, and iterator chains | `callable-default-methods.md` |
| Generic requirement identity, copyability, and specialization | `generic-requirements.md` |
| Associated type identity, projection normalization, and conformance bindings | `associated-types.md` |
| Instance/conformance declaration ownership and member identity | `interface-conformances.md` |
| Independent destruction declarations and cleanup identity | `destruction-declarations.md` |
| Path-sensitive aggregate cleanup and runtime live state | `control-flow-drop-state.md` |
| Construction declarations and type-owned creation APIs | `construction-surfaces.md` |
| Borrow coercion declaration, selection, and lowering | `borrow-coercions.md` |
| Equality operator declaration, selection, and lowering | `equality-operators.md` |
| Strict ordering declaration, derivation, and lowering | `ordering-operators.md` |
| Index declarations, requirements, coercion selection, and lowering | `indexing.md` |
| Canonical standard-library public declaration ownership | `canonical-api-surfaces.md` |
| Built-in construction, instance, and conformance authority | `builtin-type-surfaces.md` |
| Allocator, ownership, and drop design | `allocator-ownership.md` |
| Distributed `std` implementation state | `standard-library.md` |
| LSP capabilities and analysis boundary | `lsp.md` |
| Next concrete internal task | `../TODO.md` |
| Documentation build process | `site-generation.md` |
| Test ownership, integration boundaries, and redundancy policy | `testing.md` |

Entry points may summarize an owned fact in one sentence and link to its owner. They must not carry
independent completion lists, qualification details, or mutable status. Git owns chronological
history.
