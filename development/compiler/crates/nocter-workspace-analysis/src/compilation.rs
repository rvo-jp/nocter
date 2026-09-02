use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use nocter_analysis::AnalysisSnapshot;
use nocter_compile_input::ModuleIdentity;
use nocter_compiler_computation::{CompilerDiscoveryError, CompilerSourceRevision};
use nocter_discovery::DiscoveryRequest;
use nocter_package::{
    PackageResolutionPolicy, PackageResolutionRequest, PackageRootCatalog,
    resolve_package_selection_with_root_catalog, resolve_standard_package_with_root_catalog,
};
use nocter_standard_profile::bundled_standard_toolchain;
use nocter_syntax::SourceSyntaxProvider;
use nocter_workspace_revision::GenerationId;

use crate::compilation_input::ScopeCompilationInput;
use crate::errors::preparation_diagnostics;
use crate::{WorkspaceAnalysisError, WorkspaceAnalysisState, WorkspaceConfiguration};

pub(crate) fn compile_scope(
    configuration: &WorkspaceConfiguration,
    input: &ScopeCompilationInput,
    generation: GenerationId,
    package_roots: PackageRootCatalog,
    computation: &mut nocter_compiler_computation::CompilerComputation,
    revision: &CompilerSourceRevision,
) -> WorkspaceAnalysisState {
    let source_overlay = package_roots.source_overlay().clone();
    let mut source_syntax = match computation.source_syntax(revision) {
        Ok(source_syntax) => source_syntax,
        Err(error) => {
            return preparation_failed(
                source_overlay,
                WorkspaceAnalysisError::compiler_computation(error),
            );
        }
    };
    let request = match input {
        ScopeCompilationInput::Package {
            root,
            requested_sources,
        } => prepare_package(
            configuration,
            root,
            requested_sources,
            package_roots.clone(),
            &mut source_syntax,
        ),
        ScopeCompilationInput::ToolchainStandard => {
            prepare_toolchain_standard(configuration, package_roots.clone(), &mut source_syntax)
        }
        ScopeCompilationInput::SingleFile(source) => {
            prepare_single_file(configuration, source, package_roots, &mut source_syntax)
        }
    };
    drop(source_syntax);
    let request = match request {
        Ok(request) => request,
        Err(error) => return preparation_failed(source_overlay, error),
    };
    let discovered = match computation.discover(revision, request) {
        Ok(discovered) => discovered,
        Err(CompilerDiscoveryError::Discovery(failure)) => {
            return WorkspaceAnalysisState::Complete(Box::new(
                AnalysisSnapshot::from_discovery_failure(generation, failure),
            ));
        }
        Err(CompilerDiscoveryError::Computation(error)) => {
            return preparation_failed(
                source_overlay,
                WorkspaceAnalysisError::compiler_computation(error),
            );
        }
    };
    {
        let product = match computation.analyze(&discovered) {
            Ok(product) => product,
            Err(error) => {
                return preparation_failed(
                    source_overlay.clone(),
                    WorkspaceAnalysisError::compiler_computation(error),
                );
            }
        };
        let analyzed = match nocter_session::analyze_unit_from_query(&product) {
            Ok(analyzed) => analyzed,
            Err(error) => {
                return preparation_failed(
                    source_overlay.clone(),
                    WorkspaceAnalysisError::semantic_analysis(error),
                );
            }
        };
        WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::from_analyzed_unit(
            generation, analyzed,
        )))
    }
}

fn preparation_failed(
    source_overlay: nocter_filesystem::SourceOverlay,
    error: WorkspaceAnalysisError,
) -> WorkspaceAnalysisState {
    WorkspaceAnalysisState::PreparationFailed {
        source_overlay,
        diagnostics: preparation_diagnostics(&error),
        error,
    }
}

fn prepare_toolchain_standard(
    configuration: &WorkspaceConfiguration,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<DiscoveryRequest, WorkspaceAnalysisError> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let package = resolve_standard_package_with_root_catalog(
        toolchain.standard().clone(),
        package_roots,
        source_syntax,
    )
    .map_err(WorkspaceAnalysisError::from)?;
    Ok(DiscoveryRequest::toolchain_standard(
        toolchain.target(),
        package,
        bundled_standard_toolchain(&standard),
    ))
}

fn prepare_package(
    configuration: &WorkspaceConfiguration,
    root: &Path,
    requested_sources: &[PathBuf],
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<DiscoveryRequest, WorkspaceAnalysisError> {
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
    .map_err(WorkspaceAnalysisError::from)?;
    let root_package = selected.root().clone();
    let standard = selected.standard().clone();
    let package = selected
        .graph()
        .packages()
        .iter()
        .find(|package| package.identity() == &root_package)
        .ok_or_else(|| WorkspaceAnalysisError::missing_root_package(root_package.clone()))?;
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
            .map_err(WorkspaceAnalysisError::module_owner)?,
        );
    }
    if let Some(declaration) = package.declaration() {
        roots.extend(declaration.targets().iter().map(|target| {
            ModuleIdentity::new(root_package.clone(), target.module().iter().cloned())
        }));
    }
    let (packages, _, _) = selected.into_parts();
    Ok(DiscoveryRequest::declared(
        toolchain.target(),
        packages,
        roots.into_iter().collect(),
        bundled_standard_toolchain(&standard),
    ))
}

fn prepare_single_file(
    configuration: &WorkspaceConfiguration,
    source: &Path,
    package_roots: PackageRootCatalog,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<DiscoveryRequest, WorkspaceAnalysisError> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let packages = resolve_standard_package_with_root_catalog(
        toolchain.standard().clone(),
        package_roots,
        source_syntax,
    )
    .map_err(WorkspaceAnalysisError::from)?;
    Ok(DiscoveryRequest::single_file(
        toolchain.target(),
        source,
        packages,
        bundled_standard_toolchain(&standard),
    ))
}
