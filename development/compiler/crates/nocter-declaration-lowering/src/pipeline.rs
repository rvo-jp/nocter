use std::fmt;

use crate::definitions::{HeaderDefinitionError, define_declaration_headers_recovering};
use crate::surface::collect_incomplete_body_declaration_surface;
use crate::toolchain::resolve_toolchain_surface;
use crate::{
    CompileUnitInput, DeclarationContractDiagnostic, DeclarationContractError,
    DeclarationDiagnostics, DeclarationLoweringRecovery, DefinitionDiagnostic, GenericDiagnostic,
    GenericError, HeaderError, ImportDiagnostic, ImportError, LoweredDeclarations,
    NamespaceDiagnostic, PreparedImports, PreparedNamespaces, PreparedTypeBindings, PreparedTypes,
    ReservationError, SourceDiagnostic, SurfaceDiagnostic, SurfaceError, ToolchainError,
    TopologyDiagnostic, TypeBindingDiagnostic, TypeBindingError, TypeNormalizationDiagnostic,
    TypeNormalizationError, analyze_declaration_contracts, apply_toolchain_profile,
    bind_header_type_syntax, collect_declaration_surface, evaluate_compile_time_values,
    normalize_header_types, prepare_authored_imports, prepare_declaration_headers,
    prepare_generic_binders,
};

#[derive(Clone, Debug)]
pub enum DeclarationLoweringError {
    Topology(TopologyDiagnostic),
    Surface(SurfaceDiagnostic),
    InternalSurface(SurfaceError),
    DeclarationContract(DeclarationContractDiagnostic),
    InternalContract(DeclarationContractError),
    Reservation(ReservationError),
    Namespace(NamespaceDiagnostic),
    InternalHeader(HeaderError),
    Generic(GenericDiagnostic),
    InternalGeneric(GenericError),
    Import(ImportDiagnostic),
    InternalImport(ImportError),
    Toolchain(ToolchainError),
    TypeBinding(TypeBindingDiagnostic),
    InternalTypeBinding(TypeBindingError),
    TypeNormalization(TypeNormalizationDiagnostic),
    InternalTypeNormalization(TypeNormalizationError),
    Definition(DefinitionDiagnostic),
    Declaration(DeclarationDiagnostics),
    InternalDefinition(HeaderDefinitionError),
}

impl DeclarationLoweringError {
    /// Returns the complete common public diagnostic set selected by the rejecting phase.
    ///
    /// An empty slice identifies a stage error that has not crossed a public diagnostic boundary or
    /// an internal compiler inconsistency. Consumers must not manufacture a public code for it.
    #[must_use]
    pub fn source_diagnostics(&self) -> &[SourceDiagnostic] {
        match self {
            Self::Topology(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Surface(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::DeclarationContract(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Namespace(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Generic(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Import(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::TypeBinding(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::TypeNormalization(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Definition(diagnostic) => std::slice::from_ref(diagnostic.source()),
            Self::Declaration(diagnostics) => diagnostics.sources(),
            Self::InternalSurface(_)
            | Self::InternalContract(_)
            | Self::Reservation(_)
            | Self::InternalHeader(_)
            | Self::InternalGeneric(_)
            | Self::InternalImport(_)
            | Self::Toolchain(_)
            | Self::InternalTypeBinding(_)
            | Self::InternalTypeNormalization(_)
            | Self::InternalDefinition(_) => &[],
        }
    }
}

impl fmt::Display for DeclarationLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topology(error) => error.fmt(formatter),
            Self::Surface(error) => error.fmt(formatter),
            Self::InternalSurface(error) => error.fmt(formatter),
            Self::DeclarationContract(error) => error.fmt(formatter),
            Self::InternalContract(error) => error.fmt(formatter),
            Self::Reservation(error) => error.fmt(formatter),
            Self::Namespace(error) => error.fmt(formatter),
            Self::InternalHeader(error) => error.fmt(formatter),
            Self::Generic(error) => error.fmt(formatter),
            Self::InternalGeneric(error) => error.fmt(formatter),
            Self::Import(error) => error.fmt(formatter),
            Self::InternalImport(error) => error.fmt(formatter),
            Self::Toolchain(error) => error.fmt(formatter),
            Self::TypeBinding(error) => error.fmt(formatter),
            Self::InternalTypeBinding(error) => error.fmt(formatter),
            Self::TypeNormalization(error) => error.fmt(formatter),
            Self::InternalTypeNormalization(error) => error.fmt(formatter),
            Self::Definition(error) => error.fmt(formatter),
            Self::Declaration(error) => error.fmt(formatter),
            Self::InternalDefinition(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DeclarationLoweringError {}

#[derive(Clone, Debug)]
pub struct DeclarationLoweringFailure {
    error: Box<DeclarationLoweringError>,
    recovery: Option<Box<DeclarationLoweringRecovery>>,
}

impl DeclarationLoweringFailure {
    fn new(error: DeclarationLoweringError, recovery: Option<DeclarationLoweringRecovery>) -> Self {
        Self {
            error: Box::new(error),
            recovery: recovery.map(Box::new),
        }
    }

    fn without_recovery(error: DeclarationLoweringError) -> Self {
        Self::new(error, None)
    }

    #[must_use]
    pub fn current_branch(&self) -> Self {
        self.clone()
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DeclarationLoweringError,
        Option<DeclarationLoweringRecovery>,
    ) {
        (*self.error, self.recovery.map(|recovery| *recovery))
    }

    #[must_use]
    pub fn into_error(self) -> DeclarationLoweringError {
        *self.error
    }
}

/// Lowers one discovery-owned compile unit eagerly for focused tests and stage diagnostics.
///
/// Production compilation and editor analysis enter through `nocter-compiler-computation`, which
/// owns recovery and query composition. This convenience endpoint deliberately discards recovery;
/// it is not a second production scheduler.
///
/// # Errors
///
/// Returns the exact failing stage. Source-backed module-surface, declaration-contract, namespace,
/// and freeze-time declaration rules are already projected to common diagnostics;
/// remaining stage errors stay typed until their diagnostic mappings are completed.
#[cfg(any(test, feature = "test-api"))]
pub fn lower_compile_unit_declarations(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringError> {
    lower_compile_unit_declarations_recovering(input)
        .map_err(DeclarationLoweringFailure::into_error)
}

/// Lowers declarations while retaining the immutable declaration snapshot reached before an
/// authored declaration rule rejected the program.
///
/// # Errors
///
/// Returns the exact production lowering error and optional editor recovery. Earlier-stage and
/// internal-integrity failures never expose a recovery program.
#[cfg(any(test, feature = "test-api"))]
pub fn lower_compile_unit_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    lower_complete_declarations_recovering(input)
}

fn lower_complete_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    let normalized =
        prepare_compile_unit_declarations_from(input, collect_declaration_surface(input))
            .map_err(DeclarationLoweringFailure::without_recovery)?;
    finish_declarations_recovering(input, normalized)
}

/// Computes only the source-neutral accepted declaration product for a semantic query.
///
/// Current frontend bindings and source projection are deliberately discarded at this boundary;
/// the query consumer must materialize them from the retained recipe against its current input.
///
/// # Errors
///
/// Returns the same authored or integrity failure as complete declaration lowering.
pub fn lower_reusable_declarations(
    input: &CompileUnitInput<'_>,
) -> Result<crate::ReusableDeclarations, DeclarationLoweringFailure> {
    lower_complete_declarations_recovering(input).map(LoweredDeclarations::into_reusable)
}

/// Lowers declarations from an incomplete-body source while retaining declaration-only facts
/// rejected by an independent authored declaration rule.
///
/// # Errors
///
/// Returns the ordinary declaration failure. A recovery snapshot is present only when the
/// declaration graph and its source projection are both internally consistent.
pub fn lower_incomplete_body_declarations_recovering(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    let normalized = prepare_compile_unit_declarations_from(
        input,
        collect_incomplete_body_declaration_surface(input),
    )
    .map_err(DeclarationLoweringFailure::without_recovery)?;
    finish_declarations_recovering(input, normalized)
}

fn finish_declarations_recovering(
    input: &CompileUnitInput<'_>,
    normalized: PreparedTypes<'_>,
) -> Result<LoweredDeclarations, DeclarationLoweringFailure> {
    match define_declaration_headers_recovering(normalized) {
        Ok(lowered) => Ok(lowered),
        Err(failure) => {
            let (error, recovery) = failure.into_parts();
            Err(DeclarationLoweringFailure::new(
                project_definition_error(error, input),
                recovery,
            ))
        }
    }
}

fn prepare_compile_unit_declarations_from<'syntax>(
    input: &CompileUnitInput<'syntax>,
    surface: Result<crate::DeclarationSurface<'syntax>, SurfaceError>,
) -> Result<PreparedTypes<'syntax>, DeclarationLoweringError> {
    let surface = match surface {
        Ok(surface) => surface,
        Err(SurfaceError::Topology(crate::LoweringError::Rule(violation))) => {
            return match TopologyDiagnostic::project(&violation, input) {
                Some(diagnostic) => Err(DeclarationLoweringError::Topology(diagnostic)),
                None => Err(DeclarationLoweringError::InternalSurface(
                    SurfaceError::Topology(crate::LoweringError::Rule(violation)),
                )),
            };
        }
        Err(error) => {
            return match SurfaceDiagnostic::project(error, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Surface(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalSurface(internal)),
            };
        }
    };
    let toolchain_input = input
        .toolchain()
        .ok_or(DeclarationLoweringError::Toolchain(
            ToolchainError::MissingProfile,
        ))?;
    let toolchain = resolve_toolchain_surface(&surface, toolchain_input)
        .map_err(DeclarationLoweringError::Toolchain)?;
    if let Err(error) = crate::surface::validate_primitive_type_authority(
        &surface,
        toolchain.builtin_types(),
        toolchain.runtime_storage_roles(),
    ) {
        return match SurfaceDiagnostic::project(error, input) {
            Ok(diagnostic) => Err(DeclarationLoweringError::Surface(diagnostic)),
            Err(internal) => Err(DeclarationLoweringError::InternalSurface(internal)),
        };
    }
    let contracts = match analyze_declaration_contracts(&surface) {
        Ok(contracts) => contracts,
        Err(error) => {
            return match DeclarationContractDiagnostic::project(error, &surface) {
                Ok(diagnostic) => Err(DeclarationLoweringError::DeclarationContract(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalContract(internal)),
            };
        }
    };
    let reserved = crate::reservation::reserve_with_contracts(surface, contracts, toolchain)
        .map_err(DeclarationLoweringError::Reservation)?;
    let headers = match prepare_declaration_headers(reserved) {
        Ok(headers) => headers,
        Err(HeaderError::Namespace(violation)) => {
            return match NamespaceDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Namespace(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalHeader(
                    HeaderError::Namespace(internal),
                )),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalHeader(internal)),
    };
    let generics = match prepare_generic_binders(headers) {
        Ok(generics) => generics,
        Err(GenericError::Rule(violation)) => {
            return match GenericDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Generic(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalGeneric(
                    GenericError::Rule(internal),
                )),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalGeneric(internal)),
    };
    let imports = match prepare_authored_imports(generics) {
        Ok(imports) => imports,
        Err(ImportError::Namespace(violation)) => {
            return match NamespaceDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Namespace(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalImport(
                    ImportError::Namespace(internal),
                )),
            };
        }
        Err(ImportError::Rule(violation)) => {
            return match ImportDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Import(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalImport(ImportError::Rule(
                    internal,
                ))),
            };
        }
        Err(internal) => return Err(DeclarationLoweringError::InternalImport(internal)),
    };
    let namespaces = prepare_toolchain_namespaces(imports, input)?;
    let bound = bind_types(namespaces, input)?;
    let bound = evaluate_compile_time_values_for_pipeline(bound, input)?;
    normalize_types(bound, input)
}

fn evaluate_compile_time_values_for_pipeline<'syntax>(
    bound: PreparedTypeBindings<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypeBindings<'syntax>, DeclarationLoweringError> {
    match evaluate_compile_time_values(bound) {
        Ok(bound) => Ok(bound),
        Err(HeaderDefinitionError::Rule(violation)) => {
            match DefinitionDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::Definition(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalDefinition(
                    HeaderDefinitionError::Rule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalDefinition(internal)),
    }
}

fn prepare_toolchain_namespaces<'syntax>(
    imports: PreparedImports<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedNamespaces<'syntax>, DeclarationLoweringError> {
    match apply_toolchain_profile(imports) {
        Ok(namespaces) => Ok(namespaces),
        Err(ToolchainError::Rule(violation)) => match ImportDiagnostic::project(violation, input) {
            Ok(diagnostic) => Err(DeclarationLoweringError::Import(diagnostic)),
            Err(internal) => Err(DeclarationLoweringError::Toolchain(ToolchainError::Rule(
                internal,
            ))),
        },
        Err(internal) => Err(DeclarationLoweringError::Toolchain(internal)),
    }
}

fn bind_types<'syntax>(
    namespaces: PreparedNamespaces<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypeBindings<'syntax>, DeclarationLoweringError> {
    match bind_header_type_syntax(namespaces) {
        Ok(bound) => Ok(bound),
        Err(TypeBindingError::Rule(violation)) => {
            match TypeBindingDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::TypeBinding(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalTypeBinding(
                    TypeBindingError::Rule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalTypeBinding(internal)),
    }
}

fn project_definition_error(
    error: HeaderDefinitionError,
    input: &CompileUnitInput<'_>,
) -> DeclarationLoweringError {
    match error {
        HeaderDefinitionError::Rule(violation) => {
            match DefinitionDiagnostic::project(violation, input) {
                Ok(diagnostic) => DeclarationLoweringError::Definition(diagnostic),
                Err(internal) => DeclarationLoweringError::InternalDefinition(
                    HeaderDefinitionError::Rule(internal),
                ),
            }
        }
        HeaderDefinitionError::Declaration(diagnostic) => {
            DeclarationLoweringError::Declaration(diagnostic)
        }
        internal => DeclarationLoweringError::InternalDefinition(internal),
    }
}

fn normalize_types<'syntax>(
    bound: PreparedTypeBindings<'syntax>,
    input: &CompileUnitInput<'syntax>,
) -> Result<PreparedTypes<'syntax>, DeclarationLoweringError> {
    match normalize_header_types(bound) {
        Ok(normalized) => Ok(normalized),
        Err(TypeNormalizationError::Rule(violation)) => {
            match TypeNormalizationDiagnostic::project(&violation, input) {
                Some(diagnostic) => Err(DeclarationLoweringError::TypeNormalization(diagnostic)),
                None => Err(DeclarationLoweringError::InternalTypeNormalization(
                    TypeNormalizationError::Rule(violation),
                )),
            }
        }
        Err(TypeNormalizationError::RequirementRule(violation)) => {
            match TypeBindingDiagnostic::project(violation, input) {
                Ok(diagnostic) => Err(DeclarationLoweringError::TypeBinding(diagnostic)),
                Err(internal) => Err(DeclarationLoweringError::InternalTypeNormalization(
                    TypeNormalizationError::RequirementRule(internal),
                )),
            }
        }
        Err(internal) => Err(DeclarationLoweringError::InternalTypeNormalization(
            internal,
        )),
    }
}

#[cfg(test)]
mod tests;
