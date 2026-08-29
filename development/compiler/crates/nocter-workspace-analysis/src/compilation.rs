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
use nocter_semantic_computation::{DeclarationQueryOutcome, ProgramPreparationOutcome};
use nocter_session::{
    analyze_unit, analyze_unit_from_declaration_failure, analyze_unit_from_declarations,
    analyze_unit_from_prepared_body_names, analyze_unit_from_prepared_declarations,
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
            let unit = Arc::new(unit);
            let (scope, publication, body_inputs) =
                match prepare_semantic_inputs(computation, Arc::clone(&unit)) {
                    Ok(inputs) => inputs,
                    Err(error) => {
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
            for body in body_inputs {
                body.publish(&mut revision);
            }
            let _ = revision.commit();
            let products = match crate::semantic_products::demand(computation, &scope) {
                Ok(products) => products,
                Err(error) => {
                    return WorkspaceAnalysisState::PreparationFailed {
                        source_overlay,
                        diagnostics: preparation_diagnostics(&error),
                        error,
                    };
                }
            };
            let analyzed = match analyze_declaration_outcome(
                unit,
                products.declarations.outcome(),
                products.preparation.outcome(),
                products.body_names.as_ref(),
                products.typed_bodies.as_ref(),
            ) {
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

fn prepare_semantic_inputs(
    computation: &Database,
    unit: Arc<nocter_discovery::DiscoveredUnit>,
) -> Result<
    (
        nocter_semantic_computation::SemanticScopeKey,
        nocter_semantic_computation::ScopeInputPublication,
        Vec<nocter_semantic_computation::BodySourcePublication>,
    ),
    WorkspaceAnalysisError,
> {
    let module_surface = crate::module_surface::fingerprint(computation, &unit)
        .map_err(WorkspaceAnalysisError::computation)?;
    let body_inputs = crate::body_inputs::collect(computation, &unit)
        .map_err(WorkspaceAnalysisError::computation)?;
    let (scope, publication) =
        nocter_semantic_computation::ScopeInputPublication::for_unit(unit, module_surface)
            .map_err(WorkspaceAnalysisError::semantic_computation)?;
    Ok((scope, publication, body_inputs))
}

fn analyze_declaration_outcome(
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    outcome: &DeclarationQueryOutcome,
    preparation: &ProgramPreparationOutcome,
    body_names: Option<&nocter_semantic_computation::ResolvedBodyNameSet>,
    typed_bodies: Option<&nocter_semantic_computation::TypedBodySet>,
) -> Result<nocter_session::AnalyzedUnit, WorkspaceAnalysisError> {
    match outcome {
        DeclarationQueryOutcome::Accepted(declarations) => match preparation {
            ProgramPreparationOutcome::Prepared(prepared) => match (body_names, typed_bodies) {
                (Some(body_names), Some(typed_bodies)) => {
                    Ok(nocter_session::analyze_unit_from_typed_bodies(
                        unit,
                        declarations,
                        prepared,
                        body_names,
                        typed_bodies,
                    ))
                }
                (Some(body_names), None) => Ok(analyze_unit_from_prepared_body_names(
                    unit,
                    declarations,
                    prepared,
                    body_names,
                )),
                (None, _) => Ok(analyze_unit_from_prepared_declarations(
                    unit,
                    declarations,
                    prepared,
                )),
            },
            ProgramPreparationOutcome::Unavailable => {
                Ok(analyze_unit_from_declarations(unit, declarations))
            }
        },
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
