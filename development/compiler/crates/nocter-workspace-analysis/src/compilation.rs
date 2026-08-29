use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use nocter_analysis::AnalysisSnapshot;
use nocter_compile_input::ModuleIdentity;
use nocter_computation::Database;
use nocter_discovery::{DiscoveryRequest, discover_with_source_syntax};
use nocter_package::{
    PackageResolutionPolicy, PackageResolutionRequest, PackageRootCatalog,
    resolve_package_selection_with_root_catalog, resolve_standard_package_with_root_catalog,
};
use nocter_semantic_computation::DeclarationQueryOutcome;
use nocter_session::{
    analyze_unit, analyze_unit_from_declaration_failure, analyze_unit_from_declarations,
    bundled_standard_toolchain,
};
use nocter_syntax::SourceSyntaxProvider;
use nocter_workspace_revision::GenerationId;

use crate::compilation_input::ScopeCompilationInput;
use crate::errors::preparation_diagnostics;
use crate::source_syntax::ComputedSourceSyntax;
use crate::{WorkspaceAnalysisError, WorkspaceAnalysisState, WorkspaceConfiguration};

pub(crate) fn compile_scope(
    configuration: &WorkspaceConfiguration,
    input: &ScopeCompilationInput,
    generation: GenerationId,
    package_roots: PackageRootCatalog,
    computation: &mut Database,
) -> WorkspaceAnalysisState {
    let source_overlay = package_roots.source_overlay().clone();
    let mut source_syntax = ComputedSourceSyntax::new(computation);
    let discovered = match input {
        ScopeCompilationInput::Package {
            root,
            requested_sources,
        } => discover_package(
            configuration,
            root,
            requested_sources,
            package_roots.clone(),
            &mut source_syntax,
        ),
        ScopeCompilationInput::ToolchainStandard => {
            discover_toolchain_standard(configuration, package_roots.clone(), &mut source_syntax)
        }
        ScopeCompilationInput::SingleFile(source) => {
            discover_single_file(configuration, source, package_roots, &mut source_syntax)
        }
    };
    match discovered {
        Ok(unit) => {
            let module_surface = match crate::module_surface::fingerprint(computation, &unit) {
                Ok(fingerprint) => fingerprint,
                Err(error) => {
                    let error = WorkspaceAnalysisError::computation(error);
                    return WorkspaceAnalysisState::PreparationFailed {
                        source_overlay,
                        diagnostics: preparation_diagnostics(&error),
                        error,
                    };
                }
            };
            let unit = Arc::new(unit);
            let (scope, publication) =
                match nocter_semantic_computation::ScopeInputPublication::for_unit(
                    Arc::clone(&unit),
                    module_surface,
                ) {
                    Ok(publication) => publication,
                    Err(error) => {
                        let error = WorkspaceAnalysisError::semantic_computation(error);
                        return WorkspaceAnalysisState::PreparationFailed {
                            source_overlay,
                            diagnostics: preparation_diagnostics(&error),
                            error,
                        };
                    }
                };
            let mut revision = match computation.advance_revision() {
                Ok(revision) => revision,
                Err(error) => {
                    let error = WorkspaceAnalysisError::computation(error);
                    return WorkspaceAnalysisState::PreparationFailed {
                        source_overlay,
                        diagnostics: preparation_diagnostics(&error),
                        error,
                    };
                }
            };
            publication.publish(&mut revision, &scope);
            let _ = revision.commit();
            let declarations = match nocter_semantic_computation::declarations(computation, scope) {
                Ok(declarations) => declarations,
                Err(error) => {
                    let error = WorkspaceAnalysisError::computation(error);
                    return WorkspaceAnalysisState::PreparationFailed {
                        source_overlay,
                        diagnostics: preparation_diagnostics(&error),
                        error,
                    };
                }
            };
            let analyzed = match analyze_declaration_outcome(unit, declarations.outcome()) {
                Ok(analyzed) => analyzed,
                Err(error) => {
                    return WorkspaceAnalysisState::PreparationFailed {
                        source_overlay,
                        diagnostics: preparation_diagnostics(&error),
                        error,
                    };
                }
            };
            WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::from_analyzed_unit(
                generation, analyzed,
            )))
        }
        Err(AnalysisPreparationFailure::Discovery(failure)) => {
            WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::from_discovery_failure(
                generation, failure,
            )))
        }
        Err(AnalysisPreparationFailure::Preparation(error)) => {
            WorkspaceAnalysisState::PreparationFailed {
                source_overlay,
                diagnostics: preparation_diagnostics(&error),
                error,
            }
        }
    }
}

fn analyze_declaration_outcome(
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    outcome: &DeclarationQueryOutcome,
) -> Result<nocter_session::AnalyzedUnit, WorkspaceAnalysisError> {
    match outcome {
        DeclarationQueryOutcome::Accepted(declarations) => {
            Ok(analyze_unit_from_declarations(unit, declarations))
        }
        DeclarationQueryOutcome::Rejected(rejection) => {
            analyze_unit_from_declaration_failure(unit, rejection.unit(), rejection.failure())
                .map_err(WorkspaceAnalysisError::declaration_rejection)
        }
        DeclarationQueryOutcome::Unavailable => Ok(analyze_unit(unit)),
    }
}

fn discover_toolchain_standard(
    configuration: &WorkspaceConfiguration,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let package = resolve_standard_package_with_root_catalog(
        toolchain.standard().clone(),
        package_roots,
        source_syntax,
    )
    .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover_with_source_syntax(
        DiscoveryRequest::toolchain_standard(
            toolchain.target(),
            package,
            bundled_standard_toolchain(&standard),
        ),
        source_syntax,
    )
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_package(
    configuration: &WorkspaceConfiguration,
    root: &Path,
    requested_sources: &[PathBuf],
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let selected = resolve_package_selection_with_root_catalog(
        PackageResolutionRequest::new(
            root,
            toolchain.nocter_home(),
            toolchain.standard().clone(),
            PackageResolutionPolicy::new(true, true),
        ),
        package_roots,
        source_syntax,
    )
    .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    let root_package = selected.root().clone();
    let standard = selected.standard().clone();
    let package = selected
        .graph()
        .packages()
        .iter()
        .find(|package| package.identity() == &root_package)
        .ok_or_else(|| {
            AnalysisPreparationFailure::Preparation(WorkspaceAnalysisError::missing_root_package(
                root_package.clone(),
            ))
        })?;
    let mut roots = BTreeSet::new();
    roots.insert(ModuleIdentity::new(
        root_package.clone(),
        Vec::<Box<str>>::new(),
    ));
    for source in requested_sources {
        roots.insert(
            nocter_discovery::module_for_source(
                &root_package,
                root,
                source,
                selected.graph().source_overlay(),
            )
            .map_err(|error| {
                AnalysisPreparationFailure::Preparation(WorkspaceAnalysisError::module_owner(error))
            })?,
        );
    }
    if let Some(declaration) = package.declaration() {
        roots.extend(declaration.targets().iter().map(|target| {
            ModuleIdentity::new(root_package.clone(), target.module().iter().cloned())
        }));
    }
    let (packages, _, _) = selected.into_parts();
    discover_with_source_syntax(
        DiscoveryRequest::declared(
            toolchain.target(),
            packages,
            roots.into_iter().collect(),
            bundled_standard_toolchain(&standard),
        ),
        source_syntax,
    )
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_single_file(
    configuration: &WorkspaceConfiguration,
    source: &Path,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let packages = resolve_standard_package_with_root_catalog(
        toolchain.standard().clone(),
        package_roots,
        source_syntax,
    )
    .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover_with_source_syntax(
        DiscoveryRequest::single_file(
            toolchain.target(),
            source,
            packages,
            bundled_standard_toolchain(&standard),
        ),
        source_syntax,
    )
    .map_err(AnalysisPreparationFailure::Discovery)
}

enum AnalysisPreparationFailure {
    Preparation(WorkspaceAnalysisError),
    Discovery(nocter_discovery::DiscoveryFailure),
}
