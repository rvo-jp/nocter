use std::fmt;

pub(crate) mod semantic_authority;

use nocter_compile_input::CompileUnitInput;
use nocter_declarations::{
    AcceptedDeclarationProgram, BodyAnalysisDeclarationProgram, DeclarationGraph,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_frontend_bindings::{FrontendBindings, SourceAccessTable, SourceNamespaceTable};
use nocter_model::{Arena, BodyId, CompilationTarget, TypeAuthority, TypeStore};
use nocter_source_index::{DiagnosticOrigins, SourceIndex};

use crate::body_check::{BodyAssumptionTable, CapabilityEvidenceTable};
use crate::declaration_patterns::DeclarationPatternTable;
use crate::instance_operations::build_instance_operation_table_from_ids;
use crate::interface_implementation::build_interface_implementation_table_from_ids;
use crate::names::{NameResolutionInternalError, resolve_cataloged_body_names_recovering};
use crate::type_validity::validate_associated_projection_uses;
use crate::{
    BodySourceCatalog, ConstructionSurfaceBuildError, ConstructionSurfaceTable,
    CopyabilityBuildError, CopyabilityTable, DeclarationTypeValidityError, DropTable,
    DropTableError, InstanceOperationBuildError, InstanceOperationTable,
    InterfaceImplementationBuildError, InterfaceImplementationTable, NameResolutionError,
    ResolvedBodyNames, StandardSemanticError, StandardSemanticTable, catalog_body_sources,
    validate_declaration_types,
};

/// Fully validated, syntax-backed input to typed-body construction.
///
/// This value is deliberately not a partial `CheckedProgram`. It retains temporary syntax-backed
/// name uses and body sources, while owning the one declaration graph, extended type store, and
/// interface implementation authority that the final checked program will consume.
#[derive(Debug)]
pub struct PreparedChecking<'syntax> {
    semantic: PreparedSemanticProgram,
    body_sources: BodySourceCatalog<'syntax>,
    body_names: Arena<BodyId, ResolvedBodyNames>,
    source_namespaces: SourceNamespaceTable,
    source_index: SourceIndex,
}

/// Editor-only prepared bodies originating from a structurally valid but rejected declaration
/// graph. This type deliberately has no conversion into [`PreparedChecking`] and can only enter
/// the analysis-body endpoint.
#[derive(Debug)]
pub struct PreparedBodyAnalysis<'syntax>(PreparedChecking<'syntax>);

impl<'syntax> PreparedBodyAnalysis<'syntax> {
    pub(crate) fn into_parts(self) -> PreparedCheckingParts<'syntax> {
        self.0.into_parts()
    }
}

/// Syntax-independent semantic state completed before typed body construction.
///
/// A tooling snapshot may retain this value when body checking rejects authored source. It is not
/// a partial [`crate::CheckedProgram`]: checked nodes, local types, dispatch, ownership, and
/// provenance are deliberately absent.
#[derive(Clone, Debug)]
pub struct PreparedSemanticProgram {
    environment: crate::program_environment::ProgramEnvironment,
    semantics: crate::semantic_authority::SemanticAuthority,
    source_access: SourceAccessTable,
}

/// Source-neutral program-wide checking authorities reusable across body revisions.
///
/// The declaration graph contains only its stable declaration-symbol prefix. Opening a current
/// generation appends body spellings to a graph branch and pairs source access with that branch;
/// neither operation rebuilds these authorities.
#[derive(Clone, Debug)]
pub struct ReusablePreparedProgram {
    environment: crate::program_environment::ProgramEnvironment,
    semantics: crate::semantic_authority::SemanticAuthority,
}

/// Source-backed program-preparation rejection retained by the computation graph.
#[derive(Debug)]
pub struct QueriedProgramPreparationRejection {
    rule: QueriedPreparationRule,
    analysis: crate::DeclarationAnalysisRecovery,
}

impl QueriedProgramPreparationRejection {
    /// Opens an owned session branch without rerunning program preparation.
    #[must_use]
    pub fn current_branch(&self) -> PreparationFailure {
        PreparationFailure::with_declaration_recovery(
            self.rule.current_error(),
            Box::new(self.analysis.current_branch()),
        )
    }
}

#[derive(Debug)]
enum QueriedPreparationRule {
    TypeValidity(SourceDiagnostic),
    Copyability(SourceDiagnostic),
    InterfaceImplementation {
        diagnostic: Box<SourceDiagnostic>,
        missing_methods: Option<Box<crate::MissingInterfaceImplementationMethods>>,
    },
    InstanceOperations(SourceDiagnostic),
}

impl QueriedPreparationRule {
    fn capture(error: PreparationError) -> Result<Self, PreparationError> {
        match error {
            PreparationError::TypeValidity(crate::DeclarationTypeValidityError::Rule(
                diagnostic,
            )) => Ok(Self::TypeValidity(diagnostic)),
            PreparationError::Copyability(crate::CopyabilityBuildError::Rule(diagnostic)) => {
                Ok(Self::Copyability(diagnostic))
            }
            PreparationError::InterfaceImplementation(
                crate::InterfaceImplementationBuildError::Rule {
                    diagnostic,
                    missing_methods,
                },
            ) => Ok(Self::InterfaceImplementation {
                diagnostic,
                missing_methods,
            }),
            PreparationError::InstanceOperations(crate::InstanceOperationBuildError::Rule(
                diagnostic,
            )) => Ok(Self::InstanceOperations(diagnostic)),
            error => Err(error),
        }
    }

    fn current_error(&self) -> PreparationError {
        match self {
            Self::TypeValidity(diagnostic) => PreparationError::TypeValidity(
                crate::DeclarationTypeValidityError::Rule(diagnostic.clone()),
            ),
            Self::Copyability(diagnostic) => PreparationError::Copyability(
                crate::CopyabilityBuildError::Rule(diagnostic.clone()),
            ),
            Self::InterfaceImplementation {
                diagnostic,
                missing_methods,
            } => PreparationError::InterfaceImplementation(
                crate::InterfaceImplementationBuildError::Rule {
                    diagnostic: diagnostic.clone(),
                    missing_methods: missing_methods.clone(),
                },
            ),
            Self::InstanceOperations(diagnostic) => PreparationError::InstanceOperations(
                crate::InstanceOperationBuildError::Rule(diagnostic.clone()),
            ),
        }
    }
}

#[derive(Debug)]
pub enum ReusableProgramPreparationQueryOutcome {
    Prepared(Box<ReusablePreparedProgram>),
    Rejected(Box<QueriedProgramPreparationRejection>),
}

impl ReusablePreparedProgram {
    #[must_use]
    pub fn graph(&self) -> &DeclarationGraph {
        self.environment.graph()
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        self.semantics.types()
    }

    pub(crate) fn open_current<S>(
        &self,
        spellings: impl IntoIterator<Item = S>,
        source_access: SourceAccessTable,
    ) -> PreparedSemanticProgram
    where
        S: AsRef<str>,
    {
        PreparedSemanticProgram {
            environment: self.environment.with_checking_symbols(spellings),
            semantics: self.semantics.clone(),
            source_access,
        }
    }
}

impl PreparedSemanticProgram {
    pub(crate) const fn environment(&self) -> &crate::program_environment::ProgramEnvironment {
        &self.environment
    }

    pub(crate) const fn semantics(&self) -> &crate::semantic_authority::SemanticAuthority {
        &self.semantics
    }

    #[must_use]
    pub fn graph(&self) -> &DeclarationGraph {
        self.environment.graph()
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        self.semantics.types()
    }

    #[must_use]
    pub fn interface_implementations(&self) -> &InterfaceImplementationTable {
        self.environment.interface_implementations()
    }

    #[must_use]
    pub fn capability_evidence(
        &self,
        evidence: nocter_model::CapabilityEvidenceId,
    ) -> Option<&crate::CapabilityEvidence> {
        self.environment.capability_evidence().get(evidence)
    }

    #[must_use]
    pub(crate) fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        self.environment.construction_surfaces()
    }

    #[must_use]
    pub fn instance_operations(&self) -> &InstanceOperationTable {
        self.environment.instance_operations()
    }

    #[must_use]
    pub const fn copyabilities(&self) -> &CopyabilityTable {
        self.semantics.copyabilities()
    }

    #[must_use]
    pub fn drops(&self) -> &DropTable {
        self.environment.drops()
    }

    #[must_use]
    pub fn standard_semantics(&self) -> &StandardSemanticTable {
        self.environment.standard_semantics()
    }

    #[must_use]
    pub(crate) const fn source_access(&self) -> &SourceAccessTable {
        &self.source_access
    }

    #[must_use]
    pub const fn source_ownership(&self) -> &nocter_frontend_bindings::SourceOwnershipTable {
        self.source_access.ownership()
    }
}

impl<'syntax> PreparedChecking<'syntax> {
    #[must_use]
    pub fn graph(&self) -> &DeclarationGraph {
        self.semantic.graph()
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        self.semantic.types()
    }

    #[must_use]
    pub fn interface_implementations(&self) -> &InterfaceImplementationTable {
        self.semantic.interface_implementations()
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn construction_surfaces(&self) -> &ConstructionSurfaceTable {
        self.semantic.construction_surfaces()
    }

    #[must_use]
    pub fn instance_operations(&self) -> &InstanceOperationTable {
        self.semantic.instance_operations()
    }

    #[must_use]
    pub const fn copyabilities(&self) -> &CopyabilityTable {
        self.semantic.copyabilities()
    }

    #[must_use]
    pub fn drops(&self) -> &DropTable {
        self.semantic.drops()
    }

    #[must_use]
    pub fn standard_semantics(&self) -> &StandardSemanticTable {
        self.semantic.standard_semantics()
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

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn source_access(&self) -> &SourceAccessTable {
        self.semantic.source_access()
    }

    pub(crate) fn into_parts(self) -> PreparedCheckingParts<'syntax> {
        let PreparedSemanticProgram {
            environment,
            semantics,
            source_access,
        } = self.semantic;
        PreparedCheckingParts {
            environment,
            semantics,
            source_access,
            body_sources: self.body_sources,
            body_names: self.body_names,
            source_namespaces: self.source_namespaces,
            source_index: self.source_index,
        }
    }
}

pub(crate) struct PreparedCheckingParts<'syntax> {
    pub(crate) environment: crate::program_environment::ProgramEnvironment,
    pub(crate) semantics: crate::semantic_authority::SemanticAuthority,
    pub(crate) source_access: SourceAccessTable,
    pub(crate) body_sources: BodySourceCatalog<'syntax>,
    pub(crate) body_names: Arena<BodyId, ResolvedBodyNames>,
    pub(crate) source_namespaces: SourceNamespaceTable,
    pub(crate) source_index: SourceIndex,
}

impl<'syntax> PreparedCheckingParts<'syntax> {
    pub(crate) fn into_body_parts(
        self,
    ) -> (
        crate::semantic_authority::SemanticAuthority,
        BodyCheckingParts<'syntax>,
    ) {
        (
            self.semantics,
            BodyCheckingParts {
                environment: self.environment,
                source_access: self.source_access,
                body_sources: self.body_sources,
                body_names: self.body_names,
                source_namespaces: self.source_namespaces,
                source_index: self.source_index,
            },
        )
    }
}

pub(crate) struct BodyCheckingParts<'syntax> {
    pub(crate) environment: crate::program_environment::ProgramEnvironment,
    pub(crate) source_access: SourceAccessTable,
    pub(crate) body_sources: BodySourceCatalog<'syntax>,
    pub(crate) body_names: Arena<BodyId, ResolvedBodyNames>,
    pub(crate) source_namespaces: SourceNamespaceTable,
    pub(crate) source_index: SourceIndex,
}

impl BodyCheckingParts<'_> {
    pub(crate) fn into_semantic_parts(
        self,
        semantics: crate::semantic_authority::SemanticAuthority,
    ) -> (
        PreparedSemanticProgram,
        Arena<BodyId, ResolvedBodyNames>,
        SourceIndex,
    ) {
        let program = PreparedSemanticProgram {
            environment: self.environment,
            semantics,
            source_access: self.source_access,
        };
        (program, self.body_names, self.source_index)
    }
}

#[derive(Clone, Debug)]
pub enum PreparationError {
    MissingToolchain,
    TargetMismatch {
        input: CompilationTarget,
        program: CompilationTarget,
    },
    TypeValidity(DeclarationTypeValidityError),
    Copyability(CopyabilityBuildError),
    DropTable(DropTableError),
    InterfaceImplementation(InterfaceImplementationBuildError),
    ConstructionSurfaces(ConstructionSurfaceBuildError),
    InstanceOperations(InstanceOperationBuildError),
    DeclarationPatterns(crate::SubstitutionError),
    StandardSemantics(StandardSemanticError),
    NameResolution(NameResolutionError),
}

/// A preparation failure with the deepest current-generation semantic recovery stage retained for
/// editor analysis.
#[derive(Clone, Debug)]
pub struct PreparationFailure {
    error: PreparationError,
    evidence: Option<Box<PreparationFailureEvidence>>,
}

/// Typed editor repair evidence separated from one rejected preparation attempt.
#[derive(Clone, Debug)]
pub enum PreparationRepairEvidence {
    MissingInterfaceMethods(Box<crate::MissingInterfaceImplementationMethods>),
}

/// Recovery and repair facts that are valid together for one preparation failure.
#[derive(Clone, Debug)]
pub enum PreparationFailureEvidence {
    Declarations {
        analysis: Box<crate::DeclarationAnalysisRecovery>,
        repair: Option<PreparationRepairEvidence>,
    },
    Names(Box<crate::NameAnalysisRecovery>),
}

impl PreparationFailure {
    const fn new(error: PreparationError) -> Self {
        Self {
            error,
            evidence: None,
        }
    }

    fn with_declaration_recovery(
        mut error: PreparationError,
        analysis: Box<crate::DeclarationAnalysisRecovery>,
    ) -> Self {
        let repair = error.take_repair_evidence();
        Self {
            error,
            evidence: Some(Box::new(PreparationFailureEvidence::Declarations {
                analysis,
                repair,
            })),
        }
    }

    fn with_name_recovery(
        error: PreparationError,
        analysis: Box<crate::NameAnalysisRecovery>,
    ) -> Self {
        Self {
            error,
            evidence: Some(Box::new(PreparationFailureEvidence::Names(analysis))),
        }
    }

    #[must_use]
    pub const fn error(&self) -> &PreparationError {
        &self.error
    }

    #[must_use]
    pub fn into_parts(self) -> (PreparationError, Option<PreparationFailureEvidence>) {
        (self.error, self.evidence.map(|evidence| *evidence))
    }
}

impl From<PreparationError> for PreparationFailure {
    fn from(error: PreparationError) -> Self {
        Self::new(error)
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
            | Self::DeclarationPatterns(_)
            | Self::StandardSemantics(_) => None,
            Self::InterfaceImplementation(error) => error.source_diagnostic(),
            Self::InstanceOperations(error) => error.source_diagnostic(),
            Self::NameResolution(error) => error.source_diagnostic(),
        }
    }

    /// Removes declaration-repair evidence while leaving the authored diagnostic intact.
    #[must_use]
    fn take_repair_evidence(&mut self) -> Option<PreparationRepairEvidence> {
        match self {
            Self::InterfaceImplementation(error) => error
                .take_missing_methods()
                .map(PreparationRepairEvidence::MissingInterfaceMethods),
            Self::MissingToolchain
            | Self::TargetMismatch { .. }
            | Self::TypeValidity(_)
            | Self::Copyability(_)
            | Self::DropTable(_)
            | Self::ConstructionSurfaces(_)
            | Self::InstanceOperations(_)
            | Self::DeclarationPatterns(_)
            | Self::StandardSemantics(_)
            | Self::NameResolution(_) => None,
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
            Self::InterfaceImplementation(error) => error.fmt(formatter),
            Self::ConstructionSurfaces(error) => error.fmt(formatter),
            Self::InstanceOperations(error) => error.fmt(formatter),
            Self::DeclarationPatterns(error) => error.fmt(formatter),
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

impl From<InterfaceImplementationBuildError> for PreparationError {
    fn from(error: InterfaceImplementationBuildError) -> Self {
        Self::InterfaceImplementation(error)
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

impl From<crate::SubstitutionError> for PreparationError {
    fn from(error: crate::SubstitutionError) -> Self {
        Self::DeclarationPatterns(error)
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
/// interface implementation rules are then selected before body-local name rules. No failure returns a
/// partially prepared value.
///
/// # Errors
///
/// Returns the exact authored or internal failure selected by body-source cataloging, normalized
/// type validation, copyability construction, interface implementation construction, or lexical name
/// resolution.
pub fn prepare_program_checking<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: AcceptedDeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationError> {
    prepare_program_checking_internal(
        input,
        PreparationProgram::Accepted(program),
        bindings,
        source_index,
        false,
    )
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
    program: AcceptedDeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationFailure> {
    prepare_program_checking_internal(
        input,
        PreparationProgram::Accepted(program),
        bindings,
        source_index,
        true,
    )
}

/// Prepares structurally valid declarations rejected by a recoverable authored authority rule.
///
/// Unauthorized construction, instance, and interface implementation containers retain bodies and lexical
/// identities but are quarantined from global operation lookup. The input type has no production
/// transition, so successful editor checking cannot authorize compilation.
///
/// # Errors
///
/// Returns the same preparation failures and explicit recovery contracts as ordinary checking.
pub fn prepare_analysis_program_checking_recovering<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: BodyAnalysisDeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedBodyAnalysis<'syntax>, PreparationFailure> {
    prepare_program_checking_internal(
        input,
        PreparationProgram::Analysis(program),
        bindings,
        source_index,
        true,
    )
    .map(PreparedBodyAnalysis)
}

enum PreparationProgram {
    Accepted(AcceptedDeclarationProgram),
    Analysis(BodyAnalysisDeclarationProgram),
}

impl PreparationProgram {
    fn graph(&self) -> &DeclarationGraph {
        match self {
            Self::Accepted(program) => program.graph(),
            Self::Analysis(program) => program.graph(),
        }
    }

    fn into_parts(
        self,
    ) -> (
        DeclarationGraph,
        TypeAuthority,
        nocter_declarations::DeclarationAnalysisAdmission,
    ) {
        match self {
            Self::Accepted(program) => program.into_parts(),
            Self::Analysis(program) => program.into_parts(),
        }
    }
}

struct PreparedProgramAuthorities {
    interface_implementations: InterfaceImplementationTable,
    construction_surfaces: ConstructionSurfaceTable,
    instance_operations: InstanceOperationTable,
    body_assumptions: BodyAssumptionTable,
    capability_evidence: CapabilityEvidenceTable,
    copyabilities: CopyabilityTable,
    drops: DropTable,
}

struct ReusablePreparationFailure {
    error: PreparationError,
    graph: DeclarationGraph,
    types: TypeStore,
    standard_semantics: Option<StandardSemanticTable>,
}

/// Builds source-neutral program-wide checking authorities from an accepted declaration branch.
///
/// Body syntax, body spellings, source access, name resolution, and source projection are not
/// retained in the returned value. A computation query may therefore reuse it while the accepted
/// declaration surface remains equal.
///
/// # Errors
///
/// Returns a program-wide declaration or authority failure. Editor recovery is composed only by
/// the current-generation preparation endpoint, which owns the required source domain.
pub fn prepare_reusable_program(
    input: &CompileUnitInput<'_>,
    program: AcceptedDeclarationProgram,
    bindings: &FrontendBindings,
    diagnostic_origins: DiagnosticOrigins<'_>,
) -> Result<ReusablePreparedProgram, PreparationError> {
    prepare_reusable_program_internal(
        input,
        PreparationProgram::Accepted(program),
        bindings,
        diagnostic_origins,
    )
    .map_err(|failure| failure.error)
}

/// Builds the query-owned program-wide preparation outcome for one exact current projection.
///
/// # Errors
///
/// Returns internal or non-authored preparation failures. Authored rules are retained as an
/// exact-current rejection with declaration-level recovery.
pub fn prepare_reusable_program_for_query(
    input: &CompileUnitInput<'_>,
    program: AcceptedDeclarationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<ReusableProgramPreparationQueryOutcome, PreparationError> {
    match prepare_reusable_program_internal(
        input,
        PreparationProgram::Accepted(program),
        bindings,
        source_index.diagnostic_origins(),
    ) {
        Ok(prepared) => Ok(ReusableProgramPreparationQueryOutcome::Prepared(Box::new(
            prepared,
        ))),
        Err(failure) => {
            let ReusablePreparationFailure {
                error,
                graph,
                types,
                standard_semantics,
            } = *failure;
            let rule = QueriedPreparationRule::capture(error)?;
            Ok(ReusableProgramPreparationQueryOutcome::Rejected(Box::new(
                QueriedProgramPreparationRejection {
                    rule,
                    analysis: crate::DeclarationAnalysisRecovery::new(
                        graph,
                        types,
                        bindings.source_ownership().clone(),
                        source_index,
                        standard_semantics,
                    ),
                },
            )))
        }
    }
}

/// Opens one current source generation from reusable program-wide authorities and resolves its
/// body names without rebuilding those authorities.
///
/// # Errors
///
/// Returns current body-source or name-resolution failures. Source-backed name rejection retains
/// the same typed recovery contract as ordinary preparation.
pub fn prepare_program_checking_from_reusable_recovering<'syntax, S>(
    input: &'syntax CompileUnitInput<'syntax>,
    reusable: &ReusablePreparedProgram,
    checking_spellings: impl IntoIterator<Item = S>,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
) -> Result<PreparedChecking<'syntax>, PreparationFailure>
where
    S: AsRef<str>,
{
    validate_preparation_target(input, reusable.graph())?;
    let semantic = reusable.open_current(checking_spellings, bindings.source_access().clone());
    let body_sources =
        prepare_body_sources(input, semantic.graph(), bindings).map_err(PreparationFailure::new)?;
    let resolution = resolve_cataloged_body_names_recovering(
        input,
        semantic.graph(),
        bindings,
        source_index,
        body_sources,
    );
    let (body_sources, body_names, source_index) = match resolution {
        Ok(resolution) => resolution.into_parts(),
        Err(failure) => {
            let recovery = failure.recovery.map(|partial| {
                crate::NameAnalysisRecovery::new(
                    semantic.graph().clone(),
                    semantic.types().clone(),
                    partial.bodies,
                    bindings.source_ownership().clone(),
                    partial.source_index,
                )
            });
            let error = PreparationError::NameResolution(*failure.error);
            return Err(match recovery {
                Some(recovery) => PreparationFailure::with_name_recovery(error, Box::new(recovery)),
                None => PreparationFailure::new(error),
            });
        }
    };
    Ok(PreparedChecking {
        semantic,
        body_sources,
        body_names,
        source_namespaces: bindings.source_namespaces().clone(),
        source_index,
    })
}

/// Opens one current source generation from reusable program and per-body lexical query outcomes.
///
/// This is the accepted query path: it catalogs current body syntax and rebinds stable body-local
/// locators without running lexical resolution again.
///
/// # Errors
///
/// Returns a current source-domain or reusable-name integrity failure.
pub fn prepare_program_checking_from_queried_names<'syntax, S>(
    input: &'syntax CompileUnitInput<'syntax>,
    reusable: &ReusablePreparedProgram,
    checking_spellings: impl IntoIterator<Item = S>,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    names: &[&crate::ReusableBodyNames],
    rejections: &[&crate::QueriedBodyNameRejection],
) -> Result<PreparedChecking<'syntax>, PreparationFailure>
where
    S: AsRef<str>,
{
    validate_preparation_target(input, reusable.graph())?;
    let semantic = reusable.open_current(checking_spellings, bindings.source_access().clone());
    prepare_program_checking_from_current_queried_names(
        input,
        semantic,
        bindings,
        source_index,
        names,
        rejections,
    )
}

pub(crate) fn prepare_program_checking_from_current_queried_names<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    semantic: PreparedSemanticProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    names: &[&crate::ReusableBodyNames],
    rejections: &[&crate::QueriedBodyNameRejection],
) -> Result<PreparedChecking<'syntax>, PreparationFailure> {
    let body_sources =
        prepare_body_sources(input, semantic.graph(), bindings).map_err(PreparationFailure::new)?;
    let catalog = crate::names::materialize_queried_body_name_catalog(
        semantic.graph(),
        &body_sources,
        names,
        rejections,
        source_index,
    )
    .map_err(NameResolutionInternalError::ReusableBodyNames)
    .map_err(NameResolutionError::Internal)
    .map_err(PreparationError::NameResolution)
    .map_err(PreparationFailure::new)?;
    match catalog {
        crate::names::QueriedBodyNameCatalog::Resolved {
            bodies: body_names,
            source_index,
        } => Ok(PreparedChecking {
            semantic,
            body_sources,
            body_names,
            source_namespaces: bindings.source_namespaces().clone(),
            source_index,
        }),
        crate::names::QueriedBodyNameCatalog::Rejected {
            bodies,
            source_index,
            diagnostic,
        } => {
            let recovery = crate::NameAnalysisRecovery::new(
                semantic.graph().clone(),
                semantic.types().clone(),
                bodies,
                bindings.source_ownership().clone(),
                source_index,
            );
            Err(PreparationFailure::with_name_recovery(
                PreparationError::NameResolution(NameResolutionError::Rule(diagnostic)),
                Box::new(recovery),
            ))
        }
    }
}

fn prepare_program_checking_internal<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    program: PreparationProgram,
    bindings: &FrontendBindings,
    source_index: SourceIndex,
    retain_names: bool,
) -> Result<PreparedChecking<'syntax>, PreparationFailure> {
    input
        .toolchain()
        .ok_or(PreparationError::MissingToolchain)?;
    validate_preparation_target(input, program.graph())?;
    let body_sources = match prepare_body_sources(input, program.graph(), bindings) {
        Ok(body_sources) => body_sources,
        Err(error) => {
            let (graph, types, _) = program.into_parts();
            return Err(declaration_failure(
                error,
                retain_names,
                graph,
                types.into_store(),
                bindings.source_ownership().clone(),
                source_index,
                None,
            ));
        }
    };
    let reusable = match prepare_reusable_program_internal(
        input,
        program,
        bindings,
        source_index.diagnostic_origins(),
    ) {
        Ok(reusable) => reusable,
        Err(failure) => {
            let ReusablePreparationFailure {
                error,
                graph,
                types,
                standard_semantics,
            } = *failure;
            return Err(declaration_failure(
                error,
                retain_names,
                graph,
                types,
                bindings.source_ownership().clone(),
                source_index,
                standard_semantics,
            ));
        }
    };
    let resolution = match resolve_cataloged_body_names_recovering(
        input,
        reusable.graph(),
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
                        reusable.graph().clone(),
                        reusable.types().clone(),
                        partial.bodies,
                        bindings.source_ownership().clone(),
                        partial.source_index,
                    )
                });
            let error = PreparationError::NameResolution(*failure.error);
            return Err(match recovery {
                Some(recovery) => PreparationFailure::with_name_recovery(error, Box::new(recovery)),
                None => PreparationFailure::new(error),
            });
        }
    };
    let (body_sources, body_names, source_index) = resolution.into_parts();
    Ok(PreparedChecking {
        semantic: reusable
            .open_current(std::iter::empty::<&str>(), bindings.source_access().clone()),
        body_sources,
        body_names,
        source_namespaces: bindings.source_namespaces().clone(),
        source_index,
    })
}

fn prepare_reusable_program_internal(
    input: &CompileUnitInput<'_>,
    program: PreparationProgram,
    bindings: &FrontendBindings,
    diagnostic_origins: DiagnosticOrigins<'_>,
) -> Result<ReusablePreparedProgram, Box<ReusablePreparationFailure>> {
    if input.toolchain().is_none() {
        let (graph, types, _) = program.into_parts();
        return Err(Box::new(ReusablePreparationFailure {
            error: PreparationError::MissingToolchain,
            graph,
            types: types.into_store(),
            standard_semantics: None,
        }));
    }
    let (graph, types, admission) = program.into_parts();
    if input.target() != graph.target() {
        let program_target = graph.target();
        return Err(Box::new(ReusablePreparationFailure {
            error: PreparationError::TargetMismatch {
                input: input.target(),
                program: program_target,
            },
            graph,
            types: types.into_store(),
            standard_semantics: None,
        }));
    }
    let standard_semantics = match StandardSemanticTable::build(&graph, types.store()) {
        Ok(semantics) => semantics,
        Err(error) => {
            return Err(Box::new(ReusablePreparationFailure {
                error: error.into(),
                graph,
                types: types.into_store(),
                standard_semantics: None,
            }));
        }
    };
    let mut type_transaction = types.transaction();
    let authorities = match build_program_authorities(
        input,
        &graph,
        &mut type_transaction,
        bindings,
        diagnostic_origins,
        &admission,
    ) {
        Ok(authorities) => authorities,
        Err(error) => {
            return Err(Box::new(ReusablePreparationFailure {
                error,
                graph,
                types: type_transaction.freeze().into_store(),
                standard_semantics: Some(standard_semantics),
            }));
        }
    };
    let types = type_transaction
        .commit(&types)
        .expect("preparation must commit to its exact declaration authority");
    let PreparedProgramAuthorities {
        interface_implementations,
        construction_surfaces,
        instance_operations,
        body_assumptions,
        capability_evidence,
        copyabilities,
        drops,
    } = authorities;
    Ok(ReusablePreparedProgram {
        environment: crate::program_environment::ProgramEnvironment::new(
            graph,
            interface_implementations,
            construction_surfaces,
            instance_operations,
            body_assumptions,
            capability_evidence,
            drops,
            standard_semantics,
        ),
        semantics: crate::semantic_authority::SemanticAuthority::seal(types, copyabilities),
    })
}

fn validate_preparation_target(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
) -> Result<(), PreparationFailure> {
    if input.target() == graph.target() {
        return Ok(());
    }
    Err(PreparationError::TargetMismatch {
        input: input.target(),
        program: graph.target(),
    }
    .into())
}

fn prepare_body_sources<'syntax>(
    input: &'syntax CompileUnitInput<'syntax>,
    graph: &DeclarationGraph,
    bindings: &FrontendBindings,
) -> Result<BodySourceCatalog<'syntax>, PreparationError> {
    catalog_body_sources(input, graph, bindings)
        .map_err(NameResolutionInternalError::from)
        .map_err(NameResolutionError::from)
        .map_err(PreparationError::from)
}

fn build_program_authorities(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    bindings: &FrontendBindings,
    diagnostic_origins: DiagnosticOrigins<'_>,
    admission: &nocter_declarations::DeclarationAnalysisAdmission,
) -> Result<PreparedProgramAuthorities, PreparationError> {
    let operations = crate::admitted_operations::AdmittedOperations::new(graph, admission);
    validate_declaration_types(graph, types, diagnostic_origins)?;
    let copyabilities = CopyabilityTable::build(graph, types, diagnostic_origins)?;
    let declaration_patterns = DeclarationPatternTable::build(graph, types)?;
    let drops = DropTable::build_from_ids(graph, types, operations.drops())?;
    let interface_implementations = build_interface_implementation_table_from_ids(
        graph,
        types,
        diagnostic_origins,
        &declaration_patterns,
        operations.interface_implementations(),
    )?;
    validate_associated_projection_uses(input, graph, types, bindings, &interface_implementations)?;
    let construction_surfaces =
        ConstructionSurfaceTable::build_from_ids(graph, types, operations.constructions())?;
    let instance_operations = build_instance_operation_table_from_ids(
        graph,
        types,
        diagnostic_origins,
        &declaration_patterns,
        operations.instances(),
    )?;
    crate::interface_implementation::validate_interface_prerequisites(
        graph,
        types,
        diagnostic_origins,
        &interface_implementations,
        &instance_operations,
        &copyabilities,
    )?;
    let (body_assumptions, capability_evidence) =
        BodyAssumptionTable::build(graph, types, &declaration_patterns)?;
    Ok(PreparedProgramAuthorities {
        interface_implementations,
        construction_surfaces,
        instance_operations,
        body_assumptions,
        capability_evidence,
        copyabilities,
        drops,
    })
}

fn declaration_failure(
    error: PreparationError,
    retain_recovery: bool,
    graph: DeclarationGraph,
    types: TypeStore,
    source_ownership: nocter_frontend_bindings::SourceOwnershipTable,
    source_index: SourceIndex,
    standard_semantics: Option<StandardSemanticTable>,
) -> PreparationFailure {
    let recovery = retain_recovery.then(|| {
        Box::new(crate::DeclarationAnalysisRecovery::new(
            graph,
            types,
            source_ownership,
            source_index,
            standard_semantics,
        ))
    });
    match recovery {
        Some(recovery) => PreparationFailure::with_declaration_recovery(error, recovery),
        None => PreparationFailure::new(error),
    }
}

#[cfg(test)]
mod tests {
    use nocter_declaration_lowering::{
        lower_compile_unit_declarations, lower_reusable_declarations,
    };
    use nocter_declarations::NominalShape;

    use super::{prepare_program_checking, prepare_reusable_program};
    use crate::CopyCondition;
    use crate::test_support::Fixture;

    #[test]
    fn reusable_program_authority_excludes_body_symbols_until_current_open() {
        let fixture =
            Fixture::new("func main(): void {\n    let body_only_name = 1\n    return\n}\n");
        let input = fixture.input(false);
        let declarations = lower_reusable_declarations(&input).unwrap();
        let projection = declarations.materialize_projection(&input).unwrap();
        let (bindings, source_index, checking_symbols) = projection.into_parts();
        let reusable = prepare_reusable_program(
            &input,
            declarations.checking_branch(),
            &bindings,
            source_index.diagnostic_origins(),
        )
        .unwrap();

        assert!(reusable.graph().symbols().get("body_only_name").is_none());

        let current = reusable.open_current(
            checking_symbols.spellings(),
            bindings.source_access().clone(),
        );
        assert!(current.graph().symbols().get("body_only_name").is_some());
    }

    #[test]
    fn preparation_retains_syntax_owned_documentation_on_semantic_identities() {
        let fixture = Fixture::new(
            "//! Application package.\n\n/// Stored value.\nstruct Value {\n    /// Numeric field.\n    value: i32\n}\n\n/// Runs the program.\nfunc main(): void {\n    /// Temporary value.\n    let local = 1\n    return\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
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
        assert!(prepared.graph().packages().iter().any(|(package, _)| {
            index.documentation(nocter_source_index::SemanticEntity::Package(package))
                == Some("Application package.")
        }));
    }

    #[test]
    fn preparation_owns_every_program_wide_checking_authority() {
        let fixture = Fixture::new(
            "pub interface Marker {}\nstruct Value {}\ninstance Value { impl Marker }\n\
             construct Value { pub func new(): Self { loop {} } }\n\
             func main(): void { return }\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let prepared =
            prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();

        assert_eq!(prepared.interface_implementations().entries().len(), 1);
        assert_eq!(prepared.construction_surfaces().len(), 1);
        assert_eq!(prepared.body_sources().len(), 2);
        assert_eq!(prepared.body_names().len(), 2);
        assert!(!prepared.source_index().is_empty());
        assert!(prepared.types().type_count() >= nocter_model::BuiltinType::ALL.len());
        assert!(!prepared.graph().declarations().callables().is_empty());
    }

    #[test]
    fn program_wide_type_rules_precede_body_local_name_rules() {
        let fixture = Fixture::new(
            "struct Bad { value: void }\nfunc main(): void { missing\n    return\n}\n",
        );
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
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
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let error = prepare_program_checking(&input, program, &frontend_bindings, source_index)
            .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0366");
    }

    #[test]
    fn copy_struct_retains_its_generic_dependent_family_condition() {
        let fixture = Fixture::new("copy struct Box<T> {\n    value: T\n}\n");
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
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
        let (program, frontend_bindings, _) = lowered.into_checking_parts();

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
