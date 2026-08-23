# Declaration Diagnostic Boundary

This document records the closed failure classification of the declaration-lowering production
facade. It defines compiler responsibility, not language behavior. Public diagnostic meanings and
codes remain owned by [Diagnostics](../../spec/12-diagnostics.md).

## Boundary Rule

`lower_compile_unit_declarations` accepts a discovery-complete, successfully parsed compile unit.
Every failure leaving that facade belongs to exactly one of these classes:

- **authored rule**: the compiler selected a stable language rule and retained its diagnostic
  subjects before the representation that selected the rule was consumed;
- **upstream rejection**: source has lexer or parser diagnostics and cannot enter semantic
  lowering; the source error is authored, but its diagnostic belongs to the source/syntax stage;
- **discovery contract**: the caller supplied missing, duplicate, stale, unreachable, or
  contradictory package, module, source, include, or use inputs;
- **compiler integrity**: a completed earlier pass, semantic builder, source projection, or
  canonical arena is inconsistent with its own guarantees.

Only authored rules become `SourceDiagnostic` values in this facade. An upstream rejection is not
relabeled as an internal compiler error by the complete compiler, but the declaration facade does
not manufacture a second semantic diagnostic for it. Discovery and compiler-integrity failures
never receive a public language code.

## Exhaustive Stage Classification

The following tables cover every error variant reachable through the facade. Variants grouped in
one row have the same classification and boundary reason.

| Stage error | Class | Reason |
|---|---|---|
| `LoweringError::Rule` | authored rule | `E0270` and `E0271` retain exact source-edge subjects. |
| `DuplicatePackage`, `DuplicateModule`, `DuplicateSourcePath`, `DuplicateSource`, `UnknownPackage`, `MissingSource`, `InvalidPackageDeclaration`, `InvalidModuleSource`, `InvalidModuleSegment`, `InvalidModuleLayout`, `InvalidPackageModuleSet`, `InvalidSingleFilePackage`, `MissingIncludeResolution`, `DuplicateIncludeResolution`, `InvalidIncludeResolution`, `UnknownIncludeTarget`, `MissingUseResolution`, `DuplicateUseResolution`, `InvalidUseResolution`, `UnknownUseTarget`, `UnreachableImplementationSource` | discovery contract | Package discovery owns identities, physical source ownership, layouts, reachability, and one exact-source edge per authored `include` plus one module edge per authored `use`. |
| `InconsistentSyntax`, `MissingCollectedSymbol`, `Program`, `DuplicateSourceBinding` | compiler integrity | Valid syntax projection, complete symbol collection, legal builder operations, and unique source bindings are earlier-pass guarantees. |

| Surface error | Class | Reason |
|---|---|---|
| `Topology(Rule)` | authored rule | Delegates to the topology rule domain. |
| `ImplementationVisibility`, `InvalidNominalContract`, `MissingConstructionContractVisibility` | authored rule | `E0230`-`E0232` retain their exact visibility or declaration node. |
| `SyntaxErrors` | upstream rejection | The syntax tree already contains its authoritative lexer/parser diagnostics. |
| `Topology` with a non-rule error | discovery contract or compiler integrity | Preserves the topology classification above. |
| `InvalidRootShape`, `InvalidItemShape`, `InconsistentIncludeResolution`, `InconsistentUseResolution` | compiler integrity | A valid parse goal and prepared topology guarantee these shapes and independently retained source and module resolutions. |

| Contract/reservation error | Class | Reason |
|---|---|---|
| `DeclarationContractError::MissingBody`, `MismatchedBody`, `DuplicateBody`, `InvalidBodyOmission` | authored rule | `E0250`-`E0253` distinguish public callable contracts from their private definitions. Private implementation-only members need no public contract. |
| `DeclarationContractError::UncontractedConformance` | authored rule | `E0254` prevents an implementation source from adding program-wide conformance outside `index.nct`. |
| `DeclarationContractError::MissingRepresentation`, `MismatchedRepresentation`, `DuplicateRepresentation`, `RepresentationCompletedAgain` | authored rule | `E0255`-`E0258` distinguish public nominal contracts from their one private representation definition. |
| `DeclarationContractError::InconsistentSurface` | compiler integrity | Contract joining consumes the already validated surface inventory. |
| every `ReservationError` variant: `Contract`, `Program`, `DuplicateSourceBinding`, `MissingSymbol`, `UnknownPackage`, `UnknownModule`, `InvalidOwner`, `InconsistentSurface`, `InconsistentSource` | compiler integrity | Production reservation receives analyzed contracts, canonical symbols/topology, valid owners, and unused builder/source-index slots. |

| Header/generic error | Class | Reason |
|---|---|---|
| `HeaderError::Namespace` | authored rule | `E0240`-`E0242` retain the exact name or visibility syntax. |
| `HeaderError::Program`, `DuplicateSourceBinding`, `MissingDeclaration`, `MissingSource`, `MissingName`, `InconsistentName`, `InvalidVisibility`, `InconsistentSource` | compiler integrity | Parsing and surface collection close declaration names and visibility forms; contract joining and topology close representative names and sources. |
| `GenericError::Rule` | authored rule | `E0280`-`E0282` retain the offending binder and any first/inherited binder. |
| `GenericError::MissingSource`, `InconsistentSource`, `InconsistentBinder`, `InvalidOwner`, `InconsistentContract`, `DuplicateSourceBinding` | compiler integrity | Generic preparation consumes canonical surface owners and exact joined headers. |

| Import/prelude error | Class | Reason |
|---|---|---|
| `ImportError::Rule`, `ImportError::Namespace`, `PreludeError::Rule` | authored rule | `E0260`-`E0262`, `E0412`, and shared namespace rules retain exact import syntax. |
| `ImportError::Program`, `DuplicateSourceBinding`, `MissingSource`, `InvalidSyntax`, `UnknownModule`, `InvalidVisibility`, `DependencyCycle`, `InconsistentSource` | compiler integrity | Topology, syntax, header visibility, and canonical dependency ordering are complete before import preparation. |
| `PreludeError::UnknownModule` | discovery contract | Toolchain discovery must provide the selected standard prelude identity. |
| `PreludeError::InconsistentImport`, `Program` | compiler integrity | Authored imports retain their path nodes, and the builder owns prelude attachment authority. |

| Type error | Class | Reason |
|---|---|---|
| `TypeBindingError::Rule` | authored rule | `E0290`-`E0302` retain exact type, name, argument, requirement, and duplicate subjects. |
| `TypeBindingError::MissingSource`, `InvalidSyntax`, `InconsistentSource`, `DuplicateSourceBinding` | compiler integrity | Binding accepts syntax-complete declarations and canonical source/symbol projections. |
| `TypeNormalizationError::Rule` | authored rule | `E0310`-`E0313` and `E0320` consume subjects retained in `NormalizationOrigins`. |
| `TypeNormalizationError::InvalidBoundType`, `InconsistentTypeStore`, `MissingCapabilityContext`, `MissingAlias`, `InvalidSelf`, `InconsistentAssociatedIndex` | compiler integrity | Every item is a broken binding-arena, canonical-store, declaration-context, or semantic-index invariant. |

| Definition/freeze error | Class | Reason |
|---|---|---|
| `HeaderDefinitionError::Rule` | authored rule | `E0314`-`E0319` retain exact `default`, provenance, result-type, or associated-binding subjects. |
| `HeaderDefinitionError::Declaration` | authored rule | `E0200`-`E0212` select syntax-independent semantic declaration sites and project them through the frozen source index. |
| `MissingSource`, `MissingName`, `MissingSite`, `MissingType`, `MissingCallableResult`, `InvalidOwner`, `InvalidSurface`, `InvalidTypePattern`, `InvalidTargetGate`, `InvalidProvenance`, `InconsistentType`, `InconsistentSource`, `MissingDiagnosticSubject`, `Definition`, `Program`, `DuplicateSourceBinding` | compiler integrity | All are absent normalized state, an invalid semantic relationship, a failed exact source projection, or rejected builder authority after the responsible authored rule has already been separated. |

`ProgramBuildError::InvalidProgram(Declaration)` is the only nested builder failure converted to a
public declaration diagnostic. `InvalidProgram(Integrity)`, all other `ProgramBuildError` variants,
and both `DefinitionError` variants (`UnknownId`, `AlreadyDefined`) are compiler-integrity failures.

## Facade Enforcement

The outer `DeclarationLoweringError` prevents categories from being confused:

- `Topology`, `Surface`, `CallableContract`, `Namespace`, `Generic`, `Import`, `TypeBinding`,
  `TypeNormalization`, `Definition`, and `Declaration` contain projected authored diagnostics;
- every `Internal*`, `Reservation`, and non-rule `Prelude` variant contains no public diagnostic;
- `source_diagnostic()` is exhaustive over the outer enum and returns `Some` only for projected
  authored variants.

Each stage adapter must return the original typed failure when projection cannot find its retained
subject. The facade then exposes that failure through the matching internal variant. This makes a
lost source origin an integrity failure instead of silently widening a span or inventing a code.

## Grammar Boundary Ownership

The G001-G018 semantic-boundary matrix spans more than declaration lowering. Rows are assigned as
follows:

| Grammar rows | Owning boundary |
|---|---|
| G001 | package declaration analysis after syntax; duplicate directives are not a declaration-lowering rule |
| G002-G004, G006-G013, G015-G018 where the current rule concerns declarations, names, imports, headers, or header types | declaration lowering and the diagnostic families classified above |
| G005 | target selection after the declaration program has retained target gates |
| G014 and the data-position part of G016/G018 | checked-program type well-formedness |
| body/conformance behavior not decidable from headers | checked-program body and conformance checking |

Tests for a row must enter its owning facade. Reusing the declaration facade for a later semantic
row would either guess behavior early or turn a valid intermediate program into an error.
