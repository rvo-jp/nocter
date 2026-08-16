# Checked Program Design

This document assigns implementation responsibility for v0.14.0 Phase 3. It derives work from the
public specification and does not define language behavior. The specification remains authoritative
when this plan and a normative rule disagree.

## Boundary

`CheckedProgram` is the first complete, syntax-independent executable-semantics graph. It consumes
the immutable `DeclarationProgram` from Phase 2 exactly once and owns its `DeclarationGraph` plus
the same `TypeStore` extended with checked-body types. A body, type, callable, or module ID therefore
cannot be paired with declarations from another compile unit, and Phase 3 cannot create a parallel
type interner. Every body owns one typed node arena. Nodes contain the exact declaration, local
binding, field, variant, requirement, conversion, dispatch, ownership, loan, provenance, region,
and cleanup decisions selected while checking them.

Syntax trees and source ranges exist only in the checking boundary. Temporary scope tables and
syntax-origin indexes may exist while a body is being checked, but they are consumed before the
`CheckedProgram` is frozen. Source projection remains a separate value that is extended with
checked-node and body-local identities. A canonical checked node never contains a syntax node,
byte range, rendered name, or reverse lookup key.

Authored checking failures use the phase-neutral diagnostic envelope shared by compiler stages.
The checker owns rule selection and projects the retained failing syntax subject exactly once;
diagnostic construction must not rerun lookup, typing, ownership analysis, or source discovery.

The production checker consumes the complete declaration-lowering result and the same explicit
compile-unit syntax snapshots. It locates each body by the existing `BodyId` projection; it never
finds a declaration by source containment. It returns a complete `CheckedProgram` or one typed
authored/internal failure. No public partial checked program exists.

## Authority Map

| Decision | Sole authority | Later consumers |
|---|---|---|
| Packages, modules, declaration identity, header requirements, authored module imports, and prelude fallback | `DeclarationGraph` frozen through `DeclarationProgram` | checker, target validation, instantiation, presentation |
| Header, body, closure, inferred, and specialized structural type identity | the single inherited and extended `TypeStore` | every semantic stage |
| Block imports, lexical scopes, parameters, locals, pattern payloads, catch bindings, loop bindings, and closure captures | body checker | checked nodes and source projection |
| Conformance completeness, normalized signature compatibility, associated binding satisfaction, and overlap | program-wide Phase 3 conformance checker | body dispatch and instantiation |
| Data-position type well-formedness after normalization | Phase 3 type-validity checker | every checked destination and generic constraint |
| Expected types, inference constraints, outcome injection, direct/abstract calls, members, operators, coercions, construction, literals, iteration, and interpolation | typed body node construction | instantiation and MIR |
| Reachability, initialization, moves, copies, loans, provenance, regions, destruction, and generated semantic operations | checked control-flow and ownership analysis | target validation and MIR |
| Target gates, selected primitive availability, entry validity, and toolchain capability | `TargetProgram` | executable instantiation |
| Concrete generic substitution, requirement proof, conformance dispatch, opaque witness, and reachable callable graph | executable-program instantiation | MIR |
| Basic blocks, explicit cleanup edges, concrete places, and operation sequencing | MIR | machine lowering |

The checker may record an abstract interface or structural requirement selected for a generic
operation. It must not choose a concrete conformance until instantiation supplies a concrete type.
Conversely, MIR never receives a method name or requirement set from which it could repeat dispatch.

## Grammar Conformance Ownership

The remaining grammar semantic boundaries enter Phase 3 as follows:

| Rows | Phase 3 responsibility |
|---|---|
| G011 | normalized conformance signature compatibility and overlap |
| G014 | `void`, `never`, unsized, optional, and fallible data-position validity |
| G019-G021 | body result, assignment, and control-transfer checking |
| G022-G024 | loops, regions, iteration, pattern branches, recovery, and fallback |
| G025-G030 | operators, conversions, moves, calls, construction, literals, spread, and interpolation |
| G031-G032 | explicit closure capture and contextual control-expression typing |
| G033 | contextual source spellings already bound by syntax/declarations; remaining value uses follow ordinary body lookup |

Each row receives a valid, boundary, and invalid case through the production checking facade.
Package and module input permutations must not change the selected semantic target or complete
diagnostic.

## Name Resolution

The immutable declaration program retains two namespace layers for every module:

- authored declarations and imports, including effective visibility and re-exports
- compiler-selected prelude fallback, which is shadowable and never exportable

The body checker consumes those layers directly. It does not reconstruct a namespace by iterating
declarations or imports. Block imports are body-owned because their visibility and collision scope
are lexical; they do not enter `DeclarationProgram` as hypothetical declaration imports.

One temporary scope stack covers parameters, locals, block imports, pattern payloads, catch
bindings, loop bindings, closure parameters, and explicit captures. A declaration records its
semantic identity at the point where its name becomes visible. A reference immediately resolves to
that identity or to one exact module namespace entity. Because Nocter forbids shadowing, insertion
checks every enclosing lexical binding, parameter, authored module name, and built-in type name
before accepting a new visible name. Compiler-selected prelude names remain a distinct fallback
layer and are deliberately shadowable by valid authored or lexical names.

Closure capture lookup is a distinct operation over enclosing callable bindings. The capture list
selects exact outer identities first; the closure body resolves the capture spelling to a new
environment projection identity. Free-name scanning and implicit capture are prohibited.

## Checked Body Shape

Each body owns dense arenas for scopes, local/capture identities, typed nodes, places, and
control-flow edges. A typed node stores its `TypeId` and one closed operation variant. Examples are
direct call, abstract requirement call, selected coercion, outcome injection, field place, index
place, move, borrow, branch, loop, propagation, cleanup, and terminal operation. Compiler-generated
operations use the same variants and differ only in source role.

Reachability is an explicit control-flow fact, not an absent node. Unreachable source is still
name-resolved and type-checked, but flow-dependent initialization, move, loan, and provenance state
does not invent an incoming executable edge. Scope exit records generated drops in reverse
declaration order and conditional drops for maybe-initialized storage.

## Construction Order

1. Validate and index every `BodyId` projection against the supplied immutable syntax snapshots.
2. Validate program-wide conformance and normalized type-position rules needed by all bodies.
3. Check bodies in canonical `BodyId` order while assigning only body-local dense identities.
4. Infer and validate body-owned callable provenance and opaque witnesses.
5. Freeze all body arenas, consume temporary scope/origin tables, and validate every cross-ID edge.
6. Return `CheckedProgram` plus the extended source projection.

An error before step 5 destroys the builder. A later stage therefore cannot observe a body where
name resolution succeeded but ownership or provenance checking did not.

## Phase 3 Increments

1. Retain canonical module/prelude namespaces in `DeclarationProgram` and move block-import
   ownership out of declaration imports.
2. Add the checked-program model, source-projection extension, body-source catalog, and exhaustive
   internal boundary validation.
3. Implement lexical declaration/capture identity and value-name resolution.
4. Implement normalized conformance and data-position type validity.
5. Implement typed expressions, expected-type inference, calls, members, operators, coercions,
   construction, literals, outcomes, and closures.
6. Implement control flow, reachability, initialization, ownership, loans, provenance, regions,
   destruction, and complete checked-program validation.

An increment is complete only when its superseded temporary authority is consumed, its public
failures retain exact source subjects, and input-order permutation tests pass.
