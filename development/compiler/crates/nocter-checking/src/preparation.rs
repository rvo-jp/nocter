use std::fmt;

use nocter_compile_input::CompileUnitInput;
use nocter_declarations::{DeclarationGraph, DeclarationProgram};
use nocter_diagnostics::SourceDiagnostic;
use nocter_frontend_bindings::{FrontendBindings, SourceAccessTable, SourceNamespaceTable};
use nocter_model::{Arena, BodyId, CompilationTarget, TypeStore};
use nocter_source_index::SourceIndex;

use crate::names::{NameResolutionInternalError, resolve_cataloged_body_names_recovering};
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
    source_namespaces: SourceNamespaceTable,
    source_access: SourceAccessTable,
    source_index: SourceIndex,
}

/// Syntax-independent semantic state completed before typed body construction.
///
/// A tooling snapshot may retain this value when body checking rejects authored source. It is not
/// a partial [`crate::CheckedProgram`]: checked nodes, local types, dispatch, ownership, and
/// provenance are deliberately absent.
#[derive(Debug)]
pub struct PreparedSemanticProgram {
    graph: DeclarationGraph,
    types: TypeStore,
    conformances: ConformanceTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
    standard_semantics: StandardSemanticTable,
    body_names: Arena<BodyId, ResolvedBodyNames>,
    source_access: SourceAccessTable,
    source_index: SourceIndex,
}

impl PreparedSemanticProgram {
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
    pub const fn body_names(&self) -> &Arena<BodyId, ResolvedBodyNames> {
        &self.body_names
    }

    #[must_use]
    pub const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
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

    #[must_use]
    pub const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
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
            source_namespaces: self.source_namespaces,
            source_access: self.source_access,
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
    pub(crate) source_namespaces: SourceNamespaceTable,
    pub(crate) source_access: SourceAccessTable,
    pub(crate) source_index: SourceIndex,
}

impl PreparedCheckingParts<'_> {
    pub(crate) fn into_semantic_program(self) -> PreparedSemanticProgram {
        PreparedSemanticProgram {
            graph: self.graph,
            types: self.types,
            conformances: self.conformances,
            construction_surfaces: self.construction_surfaces,
            instance_operations: self.instance_operations,
            copyabilities: self.copyabilities,
            drops: self.drops,
            standard_semantics: self.standard_semantics,
            body_names: self.body_names,
            source_access: self.source_access,
            source_index: self.source_index,
        }
    }
}

#[derive(Debug)]
pub enum PreparationError {
    MissingToolchain,
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

/// A preparation failure with the deepest current-generation semantic recovery stage retained for
/// editor analysis.
#[derive(Debug)]
pub struct PreparationFailure {
    error: PreparationError,
    recovery: Option<Box<crate::SemanticAnalysisRecovery>>,
}

impl PreparationFailure {
    fn new(error: PreparationError, recovery: Option<crate::SemanticAnalysisRecovery>) -> Self {
        Self {
            error,
            recovery: recovery.map(Box::new),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &PreparationError {
        &self.error
    }

    #[must_use]
    pub fn recovery(&self) -> Option<&crate::SemanticAnalysisRecovery> {
        self.recovery.as_deref()
    }

    #[must_use]
    pub fn into_parts(self) -> (PreparationError, Option<crate::SemanticAnalysisRecovery>) {
        (self.error, self.recovery.map(|recovery| *recovery))
    }
}

impl From<PreparationError> for PreparationFailure {
    fn from(error: PreparationError) -> Self {
        Self::new(error, None)
    }
}

impl PreparationError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::TypeValidity(error) => error.source_diagnostic(),
            Self::Copyability(error) => error.source_diagnostic(),
            Self::MissingToolchain
            | Self::TargetMismatch { .. }
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
            Self::MissingToolchain => formatter.write_str("compile input has no toolchain profile"),
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
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationError> {
    prepare_program_checking_internal(input, program, bindings, source_index, false)
        .map_err(|failure| failure.error)
}

/// Prepares the ordinary checking input while retaining partial lexical state on a name rule.
///
/// # Errors
///
/// Returns the exact preparation failure and a recovery snapshot only when lexical resolution
/// completed explicit scopes and bindings before rejecting authored source.
pub fn prepare_program_checking_recovering<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: DeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationFailure> {
    prepare_program_checking_internal(input, program, bindings, source_index, true)
}

fn prepare_program_checking_internal<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: DeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    retain_names: bool,
) -> Result<PreparedChecking<'syntax>, PreparationFailure> {
    let toolchain = input
        .toolchain()
        .ok_or(PreparationError::MissingToolchain)?;
    if input.target() != program.target() {
        return Err(PreparationError::TargetMismatch {
            input: input.target(),
            program: program.target(),
        }
        .into());
    }
    let (graph, mut types) = program.into_parts();
    macro_rules! declaration_stage {
        ($operation:expr) => {
            match $operation {
                Ok(value) => value,
                Err(error) => {
                    return Err(declaration_failure(
                        error.into(),
                        retain_names,
                        graph,
                        types,
                        source_index,
                    ));
                }
            }
        };
    }

    let body_sources = declaration_stage!(
        catalog_body_sources(input, &graph, bindings)
            .map_err(NameResolutionInternalError::from)
            .map_err(NameResolutionError::from)
            .map_err(PreparationError::from)
    );
    declaration_stage!(validate_declaration_types(&graph, &types, &source_index));
    let copyabilities =
        declaration_stage!(CopyabilityTable::build(&graph, &mut types, &source_index));
    let drops = declaration_stage!(DropTable::build(&graph, &types));
    let conformances =
        declaration_stage!(build_conformance_table(&graph, &mut types, &source_index));
    let construction_surfaces = declaration_stage!(ConstructionSurfaceTable::build(
        &graph,
        &types,
        bindings.source_access(),
    ));
    let instance_operations = declaration_stage!(build_instance_operation_table(
        &graph,
        &mut types,
        &source_index
    ));
    let standard_semantics = declaration_stage!(StandardSemanticTable::build(
        toolchain.standard_roles(),
        &graph,
        &types,
        bindings,
    ));
    let resolution = match resolve_cataloged_body_names_recovering(
        input,
        &graph,
        bindings,
        source_index,
        body_sources,
    ) {
        Ok(resolution) => resolution,
        Err(failure) => {
            let recovery = retain_names
                .then_some(failure.recovery)
                .flatten()
                .map(|partial| {
                    crate::NameAnalysisRecovery::new(
                        graph,
                        types,
                        partial.bodies,
                        partial.source_index,
                    )
                });
            return Err(PreparationFailure::new(
                PreparationError::NameResolution(*failure.error),
                recovery.map(|recovery| crate::SemanticAnalysisRecovery::Names(Box::new(recovery))),
            ));
        }
    };
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
        source_namespaces: bindings.source_namespaces().clone(),
        source_access: bindings.source_access().clone(),
        source_index,
    })
}

fn declaration_failure(
    error: PreparationError,
    retain_recovery: bool,
    graph: DeclarationGraph,
    types: TypeStore,
    source_index: SourceIndex,
) -> PreparationFailure {
    let recovery = retain_recovery.then(|| {
        crate::SemanticAnalysisRecovery::Declarations(Box::new(
            crate::DeclarationAnalysisRecovery::new(graph, types, source_index),
        ))
    });
    PreparationFailure::new(error, recovery)
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::lower_compile_unit_declarations;
    use nocter_declarations::NominalShape;

    use super::prepare_program_checking;
    use crate::CopyCondition;
    use crate::test_support::Fixture;

    #[test]
    fn preparation_retains_syntax_owned_documentation_on_semantic_identities() {
        let fixture = Fixture::new(
            "//! Application module.\n\n/// Stored value.\nstruct Value {\n    /// Numeric field.\n    value: i32\n}\n\n/// Runs the program.\nfunc main(): void {\n    /// Temporary value.\n    let local = 1\n    return\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
        let index = prepared.source_index();
        let declarations = prepared.graph().declarations();
        let (nominal, _) = declarations.nominal_types().iter().next().unwrap();
        let (field, _) = declarations.fields().iter().next().unwrap();
        let (callable, _) = declarations.callables().iter().next().unwrap();
        let (body, names) = prepared
            .body_names()
            .iter()
            .find(|(_, names)| !names.locals().is_empty())
            .unwrap();
        let (local, _) = names.locals().iter().next().unwrap();

        assert_eq!(
            index.documentation(nocter_source_index::SemanticEntity::NominalType(nominal)),
            Some("Stored value.")
        );
        assert_eq!(
            index.documentation(nocter_source_index::SemanticEntity::Field(field)),
            Some("Numeric field.")
        );
        assert_eq!(
            index.documentation(nocter_source_index::SemanticEntity::Callable(callable)),
            Some("Runs the program.")
        );
        assert_eq!(
            index.documentation(nocter_source_index::SemanticEntity::LocalBinding(
                body, local
            )),
            Some("Temporary value.")
        );
        assert!(prepared.graph().modules().iter().any(|(module, _)| {
            index.documentation(nocter_source_index::SemanticEntity::Module(module))
                == Some("Application module.")
        }));
    }

    #[test]
    fn preparation_owns_every_program_wide_checking_authority() {
        let fixture = Fixture::new(
            "pub interface Marker {}\nstruct Value {}\nconform Marker for Value {}\n\
             construct Value { pub func new(): Self { loop {} } }\n\
             func main(): void { return }\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();

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
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let error = prepare_program_checking(&input, program, &frontend_bindings, source_index)
            .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0364");
    }

    #[test]
    fn copy_struct_rejects_an_unconditionally_move_only_field() {
        let fixture = Fixture::new(
            "struct Owned {\n    value: i32\n}\n\
             copy struct Invalid<T> {\n    owned: Owned\n    marker: &T\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let error = prepare_program_checking(&input, program, &frontend_bindings, source_index)
            .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0366");
    }

    #[test]
    fn copy_struct_retains_its_generic_dependent_family_condition() {
        let fixture = Fixture::new("copy struct Box<T> {\n    value: T\n}\n");
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
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

    #[test]
    fn successful_semantics_do_not_depend_on_the_presentation_index() {
        let fixture = Fixture::new(
            "func identity(value: i32): i32 { value }\nfunc main(): i32 { identity(7) }\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, _) = lowered.into_checking_parts(&input);

        let prepared = prepare_program_checking(
            &input,
            program,
            &frontend_bindings,
            nocter_source_index::SourceIndex::default(),
        )
        .unwrap();
        let checked = crate::check_prepared_program(&input, prepared).unwrap();

        assert_eq!(checked.program().bodies().len(), 2);
    }
}
