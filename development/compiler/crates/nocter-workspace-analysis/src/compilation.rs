use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use nocter_analysis::{AnalysisSnapshot, GenerationId};
use nocter_compile_input::ModuleIdentity;
use nocter_discovery::{DiscoveryRequest, discover};
use nocter_package::{
    PackageResolutionPolicy, PackageResolutionRequest, PackageRootCatalog,
    resolve_package_selection_with_root_catalog, resolve_standard_package_with_root_catalog,
};
use nocter_session::bundled_standard_toolchain;

use crate::compilation_input::ScopeCompilationInput;
use crate::errors::preparation_diagnostics;
use crate::{WorkspaceAnalysisError, WorkspaceAnalysisState, WorkspaceConfiguration};

pub(crate) fn compile_scope(
    configuration: &WorkspaceConfiguration,
    input: &ScopeCompilationInput,
    generation: GenerationId,
    package_roots: PackageRootCatalog,
) -> WorkspaceAnalysisState {
    let source_overlay = package_roots.source_overlay().clone();
    let discovered = match input {
        ScopeCompilationInput::Package {
            root,
            requested_sources,
        } => discover_package(
            configuration,
            root,
            requested_sources,
            package_roots.clone(),
        ),
        ScopeCompilationInput::ToolchainStandard => {
            discover_toolchain_standard(configuration, package_roots.clone())
        }
        ScopeCompilationInput::SingleFile(source) => {
            discover_single_file(configuration, source, package_roots)
        }
    };
    match discovered {
        Ok(unit) => {
            WorkspaceAnalysisState::Complete(Box::new(AnalysisSnapshot::compile(generation, unit)))
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

fn discover_toolchain_standard(
    configuration: &WorkspaceConfiguration,
    package_roots: PackageRootCatalog,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let package =
        resolve_standard_package_with_root_catalog(toolchain.standard().clone(), package_roots)
            .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover(DiscoveryRequest::toolchain_standard(
        toolchain.target(),
        package,
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_package(
    configuration: &WorkspaceConfiguration,
    root: &Path,
    requested_sources: &[PathBuf],
    package_roots: PackageRootCatalog,
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
    discover(DiscoveryRequest::declared(
        toolchain.target(),
        packages,
        roots.into_iter().collect(),
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

fn discover_single_file(
    configuration: &WorkspaceConfiguration,
    source: &Path,
    package_roots: PackageRootCatalog,
) -> Result<nocter_discovery::DiscoveredUnit, AnalysisPreparationFailure> {
    let toolchain = configuration.toolchain();
    let standard = toolchain.standard().identity().clone();
    let packages =
        resolve_standard_package_with_root_catalog(toolchain.standard().clone(), package_roots)
            .map_err(|error| AnalysisPreparationFailure::Preparation(error.into()))?;
    discover(DiscoveryRequest::single_file(
        toolchain.target(),
        source,
        packages,
        bundled_standard_toolchain(&standard),
    ))
    .map_err(AnalysisPreparationFailure::Discovery)
}

enum AnalysisPreparationFailure {
    Preparation(WorkspaceAnalysisError),
    Discovery(nocter_discovery::DiscoveryFailure),
}
