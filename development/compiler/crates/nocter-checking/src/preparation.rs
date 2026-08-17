use std::fmt;

use nocter_declaration_lowering::CompileUnitInput;
use nocter_declarations::{DeclarationGraph, DeclarationProgram};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{Arena, BodyId, CompilationTarget, TypeStore};
use nocter_source_index::SourceIndex;

use crate::names::{NameResolutionInternalError, resolve_cataloged_body_names};
use crate::{
    BodySourceCatalog, ConformanceBuildError, ConformanceTable, ConstructionSurfaceBuildError,
    ConstructionSurfaceTable, CopyabilityBuildError, CopyabilityTable,
    DeclarationTypeValidityError, DropTable, DropTableError, InstanceOperationBuildError,
    InstanceOperationTable, NameResolutionError, ResolvedBodyNames, StandardSemanticError,
    StandardSemanticTable, build_conformance_table, build_instance_operation_table,
    catalog_body_sources, validate_declaration_types,
};

/// Fully validated, syntax-backed input to typed-body construction.
///
/// This value is deliberately not a partial `CheckedProgram`. It retains temporary syntax-backed
/// name uses and body sources, while owning the one declaration graph, extended type store, and
/// conformance authority that the final checked program will consume.
#[derive(Debug)]
pub struct PreparedChecking<'syntax> {
    graph: DeclarationGraph,
    types: TypeStore,
    conformances: ConformanceTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
    standard_semantics: StandardSemanticTable,
    body_sources: BodySourceCatalog<'syntax>,
    body_names: Arena<BodyId, ResolvedBodyNames>,
    source_index: SourceIndex,
}

impl<'syntax> PreparedChecking<'syntax> {
    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn conformances(&self) -> &ConformanceTable {
        &self.conformances
    }

    #[must_use]
    pub const fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        &self.construction_surfaces
    }

    #[must_use]
    pub const fn instance_operations(&self) -> &InstanceOperationTable {
        &self.instance_operations
    }

    #[must_use]
    pub const fn copyabilities(&self) -> &CopyabilityTable {
        &self.copyabilities
    }

    #[must_use]
    pub const fn drops(&self) -> &DropTable {
        &self.drops
    }

    #[must_use]
    pub const fn standard_semantics(&self) -> &StandardSemanticTable {
        &self.standard_semantics
    }

    #[must_use]
    pub const fn body_sources(&self) -> &BodySourceCatalog<'syntax> {
        &self.body_sources
    }

    #[must_use]
    pub const fn body_names(&self) -> &Arena<BodyId, ResolvedBodyNames> {
        &self.body_names
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    pub(crate) fn into_parts(self) -> PreparedCheckingParts<'syntax> {
        PreparedCheckingParts {
            graph: self.graph,
            types: self.types,
            conformances: self.conformances,
            construction_surfaces: self.construction_surfaces,
            instance_operations: self.instance_operations,
            copyabilities: self.copyabilities,
            drops: self.drops,
            standard_semantics: self.standard_semantics,
            body_sources: self.body_sources,
            body_names: self.body_names,
            source_index: self.source_index,
        }
    }
}

pub(crate) struct PreparedCheckingParts<'syntax> {
    pub(crate) graph: DeclarationGraph,
    pub(crate) types: TypeStore,
    pub(crate) conformances: ConformanceTable,
    pub(crate) construction_surfaces: ConstructionSurfaceTable,
    pub(crate) instance_operations: InstanceOperationTable,
    pub(crate) copyabilities: CopyabilityTable,
    pub(crate) drops: DropTable,
    pub(crate) standard_semantics: StandardSemanticTable,
    pub(crate) body_sources: BodySourceCatalog<'syntax>,
    pub(crate) body_names: Arena<BodyId, ResolvedBodyNames>,
    pub(crate) source_index: SourceIndex,
}

#[derive(Debug)]
pub enum PreparationError {
    TargetMismatch {
        input: CompilationTarget,
        program: CompilationTarget,
    },
    TypeValidity(DeclarationTypeValidityError),
    Copyability(CopyabilityBuildError),
    DropTable(DropTableError),
    Conformance(ConformanceBuildError),
    ConstructionSurfaces(ConstructionSurfaceBuildError),
    InstanceOperations(InstanceOperationBuildError),
    StandardSemantics(StandardSemanticError),
    NameResolution(NameResolutionError),
}

impl PreparationError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::TypeValidity(error) => error.source_diagnostic(),
            Self::Copyability(error) => error.source_diagnostic(),
            Self::TargetMismatch { .. }
            | Self::DropTable(_)
            | Self::ConstructionSurfaces(_)
            | Self::StandardSemantics(_) => None,
            Self::Conformance(error) => error.source_diagnostic(),
            Self::InstanceOperations(error) => error.source_diagnostic(),
            Self::NameResolution(error) => error.source_diagnostic(),
        }
    }
}

impl fmt::Display for PreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch { input, program } => write!(
                formatter,
                "checking input target {input} does not match declaration program target {program}"
            ),
            Self::TypeValidity(error) => error.fmt(formatter),
            Self::Copyability(error) => error.fmt(formatter),
            Self::DropTable(error) => error.fmt(formatter),
            Self::Conformance(error) => error.fmt(formatter),
            Self::ConstructionSurfaces(error) => error.fmt(formatter),
            Self::InstanceOperations(error) => error.fmt(formatter),
            Self::StandardSemantics(error) => error.fmt(formatter),
            Self::NameResolution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PreparationError {}

impl From<DeclarationTypeValidityError> for PreparationError {
    fn from(error: DeclarationTypeValidityError) -> Self {
        Self::TypeValidity(error)
    }
}

impl From<ConformanceBuildError> for PreparationError {
    fn from(error: ConformanceBuildError) -> Self {
        Self::Conformance(error)
    }
}

impl From<ConstructionSurfaceBuildError> for PreparationError {
    fn from(error: ConstructionSurfaceBuildError) -> Self {
        Self::ConstructionSurfaces(error)
    }
}

impl From<InstanceOperationBuildError> for PreparationError {
    fn from(error: InstanceOperationBuildError) -> Self {
        Self::InstanceOperations(error)
    }
}

impl From<CopyabilityBuildError> for PreparationError {
    fn from(error: CopyabilityBuildError) -> Self {
        Self::Copyability(error)
    }
}

impl From<DropTableError> for PreparationError {
    fn from(error: DropTableError) -> Self {
        Self::DropTable(error)
    }
}

impl From<StandardSemanticError> for PreparationError {
    fn from(error: StandardSemanticError) -> Self {
        Self::StandardSemantics(error)
    }
}

impl From<NameResolutionError> for PreparationError {
    fn from(error: NameResolutionError) -> Self {
        Self::NameResolution(error)
    }
}

/// Opens the Phase 2 program exactly once and prepares every program-wide Phase 3 authority.
///
/// Body-source integrity is checked first. Authored normalized type, conditional copyability, and
/// conformance rules are then selected before body-local name rules. No failure returns a
/// partially prepared value.
///
/// # Errors
///
/// Returns the exact authored or internal failure selected by body-source cataloging, normalized
/// type validation, copyability construction, conformance construction, or lexical name
/// resolution.
pub fn prepare_program_checking<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: DeclarationProgram,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationError> {
    if input.target() != program.target() {
        return Err(PreparationError::TargetMismatch {
            input: input.target(),
            program: program.target(),
        });
    }
    let (graph, mut types) = program.into_parts();
    let body_sources = catalog_body_sources(input, &graph, &source_index)
        .map_err(NameResolutionInternalError::from)
        .map_err(NameResolutionError::from)?;
    validate_declaration_types(&graph, &types, &source_index)?;
    let copyabilities = CopyabilityTable::build(&graph, &mut types, &source_index)?;
    let drops = DropTable::build(&graph, &types)?;
    let conformances = build_conformance_table(&graph, &mut types, &source_index)?;
    let construction_surfaces = ConstructionSurfaceTable::build(&graph, &types)?;
    let instance_operations = build_instance_operation_table(&graph, &mut types, &source_index)?;
    let standard_semantics =
        StandardSemanticTable::build(input.standard_roles(), &graph, &types, &source_index)?;
    let resolution = resolve_cataloged_body_names(input, &graph, source_index, body_sources)?;
    let (body_sources, body_names, source_index) = resolution.into_parts();
    Ok(PreparedChecking {
        graph,
        types,
        conformances,
        construction_surfaces,
        instance_operations,
        copyabilities,
        drops,
        standard_semantics,
        body_sources,
        body_names,
        source_index,
    })
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::NominalShape;

    use super::prepare_program_checking;
    use crate::CopyCondition;
    use crate::test_support::Fixture;

    #[test]
    fn preparation_owns_every_program_wide_checking_authority() {
        let fixture = Fixture::new(
            "pub interface Marker {}\nstruct Value {}\nconform Marker for Value {}\n\
             construct Value { pub func new(): Self { loop {} } }\n\
             func main(): void { return }\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();

        assert_eq!(prepared.conformances().entries().len(), 1);
        assert_eq!(prepared.construction_surfaces().len(), 1);
        assert_eq!(prepared.body_sources().len(), 2);
        assert_eq!(prepared.body_names().len(), 2);
        assert!(!prepared.source_index().is_empty());
        assert!(!prepared.types().is_empty());
        assert!(!prepared.graph().declarations().callables().is_empty());
    }

    #[test]
    fn program_wide_type_rules_precede_body_local_name_rules() {
        let fixture = Fixture::new(
            "struct Bad { value: void }\nfunc main(): void { missing\n    return\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let error = prepare_program_checking(&input, program, source_index).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0364");
    }

    #[test]
    fn copy_struct_rejects_an_unconditionally_move_only_field() {
        let fixture = Fixture::new(
            "struct Owned {\n    value: i32\n}\n\
             copy struct Invalid<T> {\n    owned: Owned\n    marker: &T\n}\n",
        );
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let error = prepare_program_checking(&input, program, source_index).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0366");
    }

    #[test]
    fn copy_struct_retains_its_generic_dependent_family_condition() {
        let fixture = Fixture::new("copy struct Box<T> {\n    value: T\n}\n");
        let (input, prelude) = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
        let (program, source_index) = lowered.into_parts();
        let prepared = prepare_program_checking(&input, program, source_index).unwrap();
        let (family, declaration) = prepared
            .graph()
            .declarations()
            .nominal_types()
            .iter()
            .find(|(_, declaration)| {
                matches!(
                    declaration.shape(),
                    NominalShape::Struct {
                        copy_declared: true,
                        ..
                    }
                )
            })
            .unwrap();
        let condition = prepared.copyabilities().family_condition(family).unwrap();

        assert_eq!(
            condition,
            &CopyCondition::Requires(declaration.generic_parameters().iter().copied().collect())
        );
    }
}
