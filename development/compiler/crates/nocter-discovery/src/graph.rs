use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Component, Path, PathBuf};

use nocter_compile_input::{
    ModuleIdentity, ModuleSourceKind, PackageMode, PackageTargetResolutionInput,
    SourceVisibilityResolutionInput, ToolchainInput, UseResolutionInput,
};
use nocter_filesystem::SourceOverlay;
use nocter_model::PackageIdentity;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{DirectSourceSyntax, ParseGoal, SourceSyntaxProvider, SyntaxTree};
use nocter_target_selection::TargetSelectionBuilder;

use crate::error::{SourceVisibilityFailure, ToolchainDiscoveryError, UseFailure};
use crate::module_catalog::{module_sources, toolchain_standard_modules};
use crate::request::{DiscoveryLayout, DiscoveryRequest};
use crate::snapshot::{
    DiscoveredModule, DiscoveredModuleDependency, DiscoveredPackage, DiscoveredSource,
    DiscoveredUnit,
};
use crate::source_visibility::source_visibility_paths;
use crate::syntax::active_use_paths;
use crate::{DiscoveryError, DiscoveryFailure};

#[derive(Debug)]
struct PackageState {
    identity: PackageIdentity,
    display_name: Box<str>,
    mode: PackageMode,
    canonical_root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    package_declaration: Option<nocter_package::PackageDeclaration>,
}

#[derive(Debug)]
struct LoadedPackages {
    states: BTreeMap<PackageIdentity, PackageState>,
    package_roots: nocter_package::PackageRootCatalogBuilder,
    source_overlay: SourceOverlay,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Work {
    Module(ModuleIdentity),
    SingleFile {
        module: ModuleIdentity,
        path: PathBuf,
    },
    Source {
        module: ModuleIdentity,
        path: PathBuf,
    },
}

struct Builder<'syntax> {
    target: nocter_model::CompilationTarget,
    packages: BTreeMap<PackageIdentity, PackageState>,
    package_roots: nocter_package::PackageRootCatalogBuilder,
    source_overlay: SourceOverlay,
    root_packages: Vec<PackageIdentity>,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    modules: BTreeMap<ModuleIdentity, Vec<DiscoveredSource>>,
    module_dependencies: Vec<DiscoveredModuleDependency>,
    source_owners: BTreeMap<PathBuf, ModuleIdentity>,
    source_visibility_resolutions: Vec<SourceVisibilityResolutionInput>,
    use_resolutions: Vec<UseResolutionInput>,
    package_target_resolutions: Vec<PackageTargetResolutionInput>,
    target_selection: TargetSelectionBuilder,
    pending: BTreeSet<Work>,
    toolchain: ToolchainInput,
    source_syntax: &'syntax mut dyn SourceSyntaxProvider,
}

#[derive(Debug)]
enum ResolveError {
    Use(UseFailure),
    Discovery(DiscoveryError),
}

impl From<UseFailure> for ResolveError {
    fn from(error: UseFailure) -> Self {
        Self::Use(error)
    }
}

impl From<DiscoveryError> for ResolveError {
    fn from(error: DiscoveryError) -> Self {
        Self::Discovery(error)
    }
}

/// Resolves one closed package graph and its selected root modules into an immutable source graph.
///
/// # Errors
///
/// Returns a filesystem or topology error when an exact package, module, source, or active import
/// cannot be selected unambiguously.
pub fn discover(request: DiscoveryRequest) -> Result<DiscoveredUnit, DiscoveryFailure> {
    discover_with_source_syntax(request, &mut DirectSourceSyntax)
}

/// Resolves one source graph while delegating only source-text parsing to `source_syntax`.
///
/// # Errors
///
/// Returns the same discovery failures as [`discover`], including infrastructure failures from
/// the supplied syntax provider.
pub fn discover_with_source_syntax(
    request: DiscoveryRequest,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<DiscoveredUnit, DiscoveryFailure> {
    let source_overlay = request.source_overlay().clone();
    Builder::new(request, source_syntax)
        .map_err(|error| DiscoveryFailure::before_source_snapshot(error, source_overlay))?
        .run()
}

impl<'syntax> Builder<'syntax> {
    fn new(
        request: DiscoveryRequest,
        source_syntax: &'syntax mut dyn SourceSyntaxProvider,
    ) -> Result<Self, DiscoveryError> {
        let (target, layout, toolchain) = request.into_parts();
        let (loaded, roots, single_file, root_packages) = match layout {
            DiscoveryLayout::Declared { packages, roots } => {
                let root_packages = roots
                    .iter()
                    .map(|root| root.package().clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                (loaded_package_graph(packages), roots, None, root_packages)
            }
            DiscoveryLayout::ToolchainStandard { package } => {
                let mut loaded = loaded_package_graph(package);
                let standard = toolchain.standard_package().clone();
                let state = loaded
                    .states
                    .get(&standard)
                    .ok_or_else(|| DiscoveryError::UnknownPackage(standard.clone()))?;
                let roots = toolchain_standard_modules(
                    &standard,
                    &state.canonical_root,
                    &mut loaded.package_roots,
                    source_syntax,
                )?;
                (loaded, roots, None, vec![standard])
            }
            DiscoveryLayout::SingleFile {
                source,
                support_packages,
            } => {
                let mut loaded = loaded_package_graph(support_packages);
                let single_file = load_single_file_package(&mut loaded, source, &toolchain)?;
                let root_packages = vec![single_file.0.package().clone()];
                (loaded, Vec::new(), Some(single_file), root_packages)
            }
        };
        let LoadedPackages {
            states: packages,
            package_roots,
            source_overlay,
            sources,
            syntax,
        } = loaded;
        validate_package_dependencies(&packages)?;
        validate_toolchain(&packages, &toolchain)?;
        let package_target_resolutions = discover_package_targets(&packages, &roots);
        let mut pending = initial_work(&packages, &roots, &toolchain)?;
        if let Some((module, path)) = single_file {
            pending.insert(Work::SingleFile { module, path });
        }

        Ok(Self {
            target,
            packages,
            package_roots,
            source_overlay,
            root_packages,
            sources,
            syntax,
            modules: BTreeMap::new(),
            module_dependencies: Vec::new(),
            source_owners: BTreeMap::new(),
            source_visibility_resolutions: Vec::new(),
            use_resolutions: Vec::new(),
            package_target_resolutions,
            target_selection: TargetSelectionBuilder::new(),
            pending,
            toolchain,
            source_syntax,
        })
    }

    fn run(mut self) -> Result<DiscoveredUnit, DiscoveryFailure> {
        while let Some(work) = self.pending.pop_first() {
            let loaded = match work {
                Work::Module(module) => self.load_module(module),
                Work::SingleFile { module, path } => {
                    self.load_module_source(module, &path, ModuleSourceKind::SingleFile)
                }
                Work::Source { module, path } => {
                    self.load_module_source(module, &path, ModuleSourceKind::Implementation)
                }
            };
            if let Err(error) = loaded {
                return Err(self.into_failure(error));
            }
        }

        let toolchain = Some(self.toolchain.clone());

        let packages = self
            .packages
            .into_values()
            .map(|state| DiscoveredPackage {
                identity: state.identity,
                display_name: state.display_name,
                mode: state.mode,
                dependencies: state.dependencies,
            })
            .collect();
        let modules = self
            .modules
            .into_iter()
            .map(|(identity, mut sources)| {
                sources.sort_unstable_by(|left, right| {
                    left.kind()
                        .ne(&ModuleSourceKind::Root)
                        .cmp(&right.kind().ne(&ModuleSourceKind::Root))
                        .then_with(|| left.canonical_path().cmp(right.canonical_path()))
                });
                DiscoveredModule::new(identity, sources)
            })
            .collect();
        self.source_visibility_resolutions
            .sort_unstable_by_key(|resolution| {
                let node = resolution.declaration();
                (node.source(), node.index())
            });
        self.use_resolutions.sort_unstable_by_key(|resolution| {
            let node = resolution.declaration();
            (node.source(), node.index())
        });
        self.module_dependencies.sort_unstable();
        self.module_dependencies.dedup();
        Ok(DiscoveredUnit {
            target: self.target,
            source_overlay: self.source_overlay,
            sources: self.sources,
            syntax: self.syntax,
            packages,
            root_packages: self.root_packages,
            modules,
            module_dependencies: self.module_dependencies,
            source_visibility_resolutions: self.source_visibility_resolutions,
            use_resolutions: self.use_resolutions,
            package_target_resolutions: self.package_target_resolutions,
            target_selection: self.target_selection.finish(),
            toolchain,
        })
    }

    fn into_failure(self, error: DiscoveryError) -> DiscoveryFailure {
        DiscoveryFailure::from_snapshot(error, self.source_overlay, self.sources, self.syntax)
    }

    fn load_module(&mut self, module: ModuleIdentity) -> Result<(), DiscoveryError> {
        if self.modules.contains_key(&module) {
            return Ok(());
        }
        let package = self
            .packages
            .get(module.package())
            .ok_or_else(|| DiscoveryError::UnknownPackage(module.package().clone()))?;
        if package.mode == PackageMode::SingleFile {
            return Err(DiscoveryError::MissingModuleRoot {
                module,
                path: package.canonical_root.join("index.nct"),
            });
        }
        let directory = join_module_path(&package.canonical_root, module.path());
        let paths = module_sources(
            module.package(),
            &package.canonical_root,
            &directory,
            &self.source_overlay,
        )?;
        for path in paths {
            let kind = if path == directory.join("index.nct") {
                ModuleSourceKind::Root
            } else {
                ModuleSourceKind::Implementation
            };
            let path = canonicalize(&self.source_overlay, "canonicalize module source", &path)?;
            self.validate_inside_package(
                module.package(),
                &path,
                (kind == ModuleSourceKind::Root).then_some(&module),
            )?;
            self.load_module_source(module.clone(), &path, kind)?;
        }
        Ok(())
    }

    fn load_module_source(
        &mut self,
        module: ModuleIdentity,
        path: &Path,
        kind: ModuleSourceKind,
    ) -> Result<(), DiscoveryError> {
        let path = canonicalize(&self.source_overlay, "canonicalize module source", path)?;
        if let Some(owner) = self.source_owners.get(&path) {
            if owner != &module {
                return Err(DiscoveryError::ConflictingSourceOwner {
                    path,
                    first: owner.clone(),
                    second: module,
                });
            }
            return Ok(());
        }
        self.validate_inside_package(
            module.package(),
            &path,
            (kind == ModuleSourceKind::Root).then_some(&module),
        )?;
        let canonical_name = canonical_text(&path)?;
        let syntax_index = if let Some(source) = self.sources.find_by_name(&canonical_name) {
            let parsed = self
                .source_syntax
                .parsed_syntax(source, ParseGoal::SourceFile)
                .map_err(|error| DiscoveryError::SourceSyntax {
                    path: path.clone(),
                    error,
                })?;
            let tree = parsed
                .bind(source)
                .ok_or(DiscoveryError::InconsistentSourceSnapshot(source.id()))?;
            let index = self.syntax.len();
            self.syntax.push(tree);
            index
        } else {
            load_source(
                &self.source_overlay,
                &mut self.sources,
                &mut self.syntax,
                &path,
                ParseGoal::SourceFile,
                self.source_syntax,
            )?
        };
        self.source_owners.insert(path.clone(), module.clone());
        self.modules
            .entry(module.clone())
            .or_default()
            .push(DiscoveredSource::new(
                canonical_text(&path)?,
                kind,
                syntax_index,
            ));

        let tree = &self.syntax[syntax_index];
        self.target_selection
            .include_tree(self.target, &self.sources, tree)
            .map_err(DiscoveryError::TargetSelection)?;
        let source = self.sources.get(tree.source()).ok_or_else(|| {
            DiscoveryError::TargetSelection(
                nocter_target_selection::TargetSelectionError::MissingSource(tree.source()),
            )
        })?;
        let source_visibility_paths = source_visibility_paths(source, tree)?;
        let active_use_paths = active_use_paths(source, tree, self.target_selection.selection())?;
        for (declaration, authored_path) in source_visibility_paths {
            let target =
                self.resolve_source_visibility(&module, &path, declaration, &authored_path)?;
            if self.package(module.package())?.mode == PackageMode::SingleFile {
                self.pending.insert(Work::Source {
                    module: module.clone(),
                    path: PathBuf::from(target.as_ref()),
                });
            }
            self.source_visibility_resolutions
                .push(SourceVisibilityResolutionInput::new(declaration, target));
        }
        for (declaration, authored_path) in active_use_paths {
            let target = self.resolve_use(&module, &path, declaration, &authored_path)?;
            self.module_dependencies
                .push(DiscoveredModuleDependency::new(
                    module.clone(),
                    target.clone(),
                ));
            self.pending.insert(Work::Module(target.clone()));
            self.use_resolutions
                .push(UseResolutionInput::new(declaration, target));
        }
        Ok(())
    }

    fn resolve_source_visibility(
        &mut self,
        importer: &ModuleIdentity,
        source: &Path,
        declaration: nocter_syntax::NodeId,
        authored: &str,
    ) -> Result<Box<str>, DiscoveryError> {
        let package_mode = self.package(importer.package())?.mode;
        let source_directory = source.parent().ok_or_else(|| {
            source_visibility_error(
                declaration,
                authored,
                SourceVisibilityFailure::OutsidePackage,
            )
        })?;
        let candidate = source_directory.join(authored);
        if !regular_file(&self.source_overlay, &candidate)? {
            return Err(source_visibility_error(
                declaration,
                authored,
                SourceVisibilityFailure::NotFound,
            ));
        }
        let candidate = canonicalize_dependency(&self.source_overlay, &candidate)?;
        self.validate_package_boundary(importer.package(), &candidate)
            .map_err(|error| source_visibility_boundary_error(declaration, authored, error))?;
        if package_mode == PackageMode::Declared {
            let owner = self
                .nearest_module(importer.package(), &candidate)
                .map_err(|error| source_visibility_boundary_error(declaration, authored, error))?;
            if &owner != importer {
                return Err(source_visibility_error(
                    declaration,
                    authored,
                    SourceVisibilityFailure::CrossesModule { module: owner },
                ));
            }
        }
        canonical_text(&candidate)
    }

    fn resolve_use(
        &mut self,
        importer: &ModuleIdentity,
        source: &Path,
        declaration: nocter_syntax::NodeId,
        authored: &str,
    ) -> Result<ModuleIdentity, DiscoveryError> {
        let package_mode = self.package(importer.package())?.mode;
        if package_mode == PackageMode::SingleFile
            && (authored.starts_with("./")
                || authored.starts_with("../")
                || authored.starts_with('/'))
        {
            return Err(use_error(
                declaration,
                authored,
                UseFailure::SingleFileLocalUse,
            ));
        }
        let segments: Vec<_> = authored.split('/').collect();
        let result = if authored.starts_with("./") || authored.starts_with("../") {
            self.resolve_relative(importer, source, authored)
        } else if authored.starts_with('/') {
            self.resolve_module_candidate(importer.package(), &segments[1..])
        } else {
            let alias = segments[0];
            let Some(target_package) = self.package(importer.package())?.dependencies.get(alias)
            else {
                return Err(use_error(
                    declaration,
                    authored,
                    UseFailure::UnknownDependency {
                        alias: alias.into(),
                    },
                ));
            };
            let target_package = target_package.clone();
            self.resolve_module_candidate(&target_package, &segments[1..])
        };
        result.map_err(|error| match error {
            ResolveError::Use(failure) => use_error(declaration, authored, failure),
            ResolveError::Discovery(error) => error,
        })
    }

    fn resolve_relative(
        &mut self,
        importer: &ModuleIdentity,
        source: &Path,
        authored: &str,
    ) -> Result<ModuleIdentity, ResolveError> {
        let canonical_root = self
            .packages
            .get(importer.package())
            .ok_or(UseFailure::OutsidePackage)?
            .canonical_root
            .clone();
        let source_directory = source.parent().ok_or(UseFailure::OutsidePackage)?;
        let relative = source_directory
            .strip_prefix(&canonical_root)
            .map_err(|_| UseFailure::OutsidePackage)?;
        let mut components = normalized_components(relative)?;
        for component in authored.split('/') {
            match component {
                "." => {}
                ".." => {
                    components.pop().ok_or(UseFailure::OutsidePackage)?;
                }
                segment => components.push(segment.into()),
            }
        }
        let base = components.iter().fold(canonical_root, |path, segment| {
            path.join(Path::new(segment.as_ref()))
        });
        let module_candidate = base.join("index.nct");
        if !regular_file(&self.source_overlay, &module_candidate)? {
            return Err(UseFailure::NotFound.into());
        }
        self.resolve_existing_module(importer.package(), &module_candidate)
    }

    fn resolve_module_candidate(
        &mut self,
        package: &PackageIdentity,
        segments: &[&str],
    ) -> Result<ModuleIdentity, ResolveError> {
        let canonical_root = self
            .packages
            .get(package)
            .ok_or(UseFailure::OutsidePackage)?
            .canonical_root
            .clone();
        let root = segments
            .iter()
            .fold(canonical_root, |path, segment| path.join(segment))
            .join("index.nct");
        if !regular_file(&self.source_overlay, &root)? {
            return Err(UseFailure::NotFound.into());
        }
        self.resolve_existing_module(package, &root)
    }

    fn resolve_existing_module(
        &mut self,
        package: &PackageIdentity,
        root: &Path,
    ) -> Result<ModuleIdentity, ResolveError> {
        let root = canonicalize_dependency(&self.source_overlay, root)?;
        self.validate_package_boundary(package, &root)?;
        let canonical_root = self
            .packages
            .get(package)
            .ok_or(UseFailure::OutsidePackage)?
            .canonical_root
            .clone();
        let directory = root.parent().ok_or(UseFailure::InvalidModuleDirectory)?;
        let relative = directory
            .strip_prefix(&canonical_root)
            .map_err(|_| UseFailure::OutsidePackage)?;
        let path = normalized_components(relative)?;
        Ok(ModuleIdentity::new(
            package.clone(),
            path.iter().map(AsRef::as_ref),
        ))
    }

    fn nearest_module(
        &self,
        package: &PackageIdentity,
        source: &Path,
    ) -> Result<ModuleIdentity, ResolveError> {
        let state = self
            .packages
            .get(package)
            .ok_or(UseFailure::OutsidePackage)?;
        let mut directory = source.parent().ok_or(UseFailure::OutsidePackage)?;
        loop {
            if regular_file(&self.source_overlay, &directory.join("index.nct"))? {
                let relative = directory
                    .strip_prefix(&state.canonical_root)
                    .map_err(|_| UseFailure::OutsidePackage)?;
                let path = normalized_components(relative)?;
                return Ok(ModuleIdentity::new(
                    package.clone(),
                    path.iter().map(AsRef::as_ref),
                ));
            }
            if directory == state.canonical_root {
                return Err(UseFailure::InvalidModuleDirectory.into());
            }
            directory = directory.parent().ok_or(UseFailure::OutsidePackage)?;
        }
    }

    fn package(&self, identity: &PackageIdentity) -> Result<&PackageState, DiscoveryError> {
        self.packages
            .get(identity)
            .ok_or_else(|| DiscoveryError::UnknownPackage(identity.clone()))
    }

    fn validate_inside_package(
        &mut self,
        package: &PackageIdentity,
        path: &Path,
        expected_module: Option<&ModuleIdentity>,
    ) -> Result<(), DiscoveryError> {
        let canonical_root = self.package(package)?.canonical_root.clone();
        if !path.starts_with(&canonical_root) {
            return Err(DiscoveryError::InvalidPackageRoot {
                package: package.clone(),
                path: path.to_path_buf(),
            });
        }
        if let Some(module) = expected_module {
            if let Err(error) = self.validate_package_boundary(package, path) {
                return Err(match error {
                    ResolveError::Use(failure) => DiscoveryError::InvalidModulePath {
                        module: module.clone(),
                        path: path.into(),
                        failure,
                    },
                    ResolveError::Discovery(error) => error,
                });
            }
            let root = path.parent().unwrap_or(path);
            let relative = root.strip_prefix(&canonical_root).map_err(|_| {
                DiscoveryError::InvalidPackageRoot {
                    package: package.clone(),
                    path: path.to_path_buf(),
                }
            })?;
            let actual = normalized_components(relative).map_err(|_| {
                DiscoveryError::InvalidPackageRoot {
                    package: package.clone(),
                    path: path.to_path_buf(),
                }
            })?;
            let expected: Vec<_> = module.path().iter().map(AsRef::as_ref).collect();
            if actual.iter().map(AsRef::as_ref).ne(expected) {
                return Err(DiscoveryError::InvalidPackageRoot {
                    package: package.clone(),
                    path: path.to_path_buf(),
                });
            }
        }
        Ok(())
    }

    fn validate_package_boundary(
        &mut self,
        package: &PackageIdentity,
        path: &Path,
    ) -> Result<(), ResolveError> {
        let state = self
            .packages
            .get(package)
            .ok_or(UseFailure::OutsidePackage)?;
        if !path.starts_with(&state.canonical_root) {
            return Err(UseFailure::OutsidePackage.into());
        }
        let mut directory = path.parent().ok_or(UseFailure::OutsidePackage)?;
        while directory != state.canonical_root {
            if self
                .package_roots
                .has_package_declaration(directory, self.source_syntax)
                .map_err(DiscoveryError::PackageRootProbe)?
            {
                return Err(UseFailure::CrossesPackage {
                    root: directory.into(),
                }
                .into());
            }
            directory = directory.parent().ok_or(UseFailure::OutsidePackage)?;
        }
        Ok(())
    }
}

fn discover_package_targets(
    packages: &BTreeMap<PackageIdentity, PackageState>,
    roots: &[ModuleIdentity],
) -> Vec<PackageTargetResolutionInput> {
    let selected = roots.iter().collect::<BTreeSet<_>>();
    let mut resolutions = Vec::new();
    for package in packages.values() {
        let Some(declaration) = package.package_declaration.as_ref() else {
            continue;
        };
        for target in declaration.targets() {
            let module = ModuleIdentity::new(
                package.identity.clone(),
                target.module().iter().map(AsRef::as_ref),
            );
            if selected.contains(&module) {
                resolutions.push(PackageTargetResolutionInput::new(
                    target.declaration(),
                    target.name().value(),
                    target.name().literal(),
                    target.kind(),
                    target.order(),
                    module,
                ));
            }
        }
    }
    resolutions.sort_unstable_by_key(|resolution| {
        let declaration = resolution.declaration();
        (declaration.source(), declaration.index())
    });
    resolutions
}

fn load_source(
    source_overlay: &SourceOverlay,
    sources: &mut SourceMap,
    syntax: &mut Vec<SyntaxTree>,
    path: &Path,
    goal: ParseGoal,
    source_syntax: &mut dyn SourceSyntaxProvider,
) -> Result<usize, DiscoveryError> {
    let bytes = source_overlay
        .read(path)
        .map_err(|error| DiscoveryError::Filesystem {
            operation: "read",
            path: path.into(),
            error,
        })?;
    let name = canonical_text(path)?;
    let source = sources
        .add_bytes(SourceName::new(name.as_ref()), &bytes)
        .map_err(|error| DiscoveryError::Source {
            path: path.into(),
            error,
        })?;
    let parsed = source_syntax
        .parsed_syntax(
            sources
                .get(source)
                .expect("newly allocated source remains in the source map"),
            goal,
        )
        .map_err(|error| DiscoveryError::SourceSyntax {
            path: path.into(),
            error,
        })?;
    let tree = parsed
        .bind(
            sources
                .get(source)
                .expect("newly allocated source remains in the source map"),
        )
        .ok_or(DiscoveryError::InconsistentSourceSnapshot(source))?;
    let index = syntax.len();
    syntax.push(tree);
    Ok(index)
}

fn join_module_path(root: &Path, path: &[Box<str>]) -> PathBuf {
    path.iter().fold(root.to_path_buf(), |path, segment| {
        path.join(Path::new(segment.as_ref()))
    })
}

fn loaded_package_graph(graph: nocter_package::ResolvedPackageGraph) -> LoadedPackages {
    let (package_roots, sources, syntax, packages) = graph.into_parts();
    let source_overlay = package_roots.source_overlay().clone();
    let states = packages
        .into_iter()
        .map(|package| {
            let identity = package.identity().clone();
            (
                identity.clone(),
                PackageState {
                    identity,
                    display_name: package.display_name().into(),
                    mode: PackageMode::Declared,
                    canonical_root: package.root().to_path_buf(),
                    dependencies: package.dependencies().clone(),
                    package_declaration: package.declaration().cloned(),
                },
            )
        })
        .collect();
    LoadedPackages {
        states,
        package_roots: package_roots.into_builder(),
        source_overlay,
        sources,
        syntax,
    }
}

fn load_single_file_package(
    loaded: &mut LoadedPackages,
    source: PathBuf,
    toolchain: &ToolchainInput,
) -> Result<(ModuleIdentity, PathBuf), DiscoveryError> {
    if source.extension().and_then(|extension| extension.to_str()) != Some("nct") {
        return Err(DiscoveryError::InvalidSingleFileExtension(source));
    }
    let source = canonicalize(
        &loaded.source_overlay,
        "canonicalize single-file input",
        &source,
    )?;
    if !regular_file(&loaded.source_overlay, &source)? {
        return Err(DiscoveryError::Filesystem {
            operation: "inspect single-file input",
            path: source,
            error: io::Error::new(io::ErrorKind::InvalidInput, "path is not a regular file"),
        });
    }
    let canonical = canonical_text(&source)?;
    let identity = PackageIdentity::new(format!("single:{canonical}"));
    if loaded.states.contains_key(&identity) {
        return Err(DiscoveryError::DuplicatePackage(identity));
    }
    let display_name = source
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| DiscoveryError::NonUnicodeCanonicalPath(source.clone()))?;
    let canonical_root = source
        .parent()
        .ok_or_else(|| DiscoveryError::InvalidPackageRoot {
            package: identity.clone(),
            path: source.clone(),
        })?
        .to_path_buf();
    let module = ModuleIdentity::new(identity.clone(), Vec::<&str>::new());
    loaded.states.insert(
        identity.clone(),
        PackageState {
            identity,
            display_name: display_name.into(),
            mode: PackageMode::SingleFile,
            canonical_root,
            dependencies: BTreeMap::from([(
                Box::<str>::from("std"),
                toolchain.standard_package().clone(),
            )]),
            package_declaration: None,
        },
    );
    Ok((module, source))
}

fn validate_package_dependencies(
    packages: &BTreeMap<PackageIdentity, PackageState>,
) -> Result<(), DiscoveryError> {
    for state in packages.values() {
        for dependency in state.dependencies.values() {
            if !packages.contains_key(dependency) {
                return Err(DiscoveryError::UnknownPackage(dependency.clone()));
            }
        }
    }
    Ok(())
}

fn validate_toolchain(
    packages: &BTreeMap<PackageIdentity, PackageState>,
    toolchain: &ToolchainInput,
) -> Result<(), DiscoveryError> {
    if !packages.contains_key(toolchain.standard_package()) {
        return Err(DiscoveryError::UnknownPackage(
            toolchain.standard_package().clone(),
        ));
    }
    let mut attachment_kinds = BTreeSet::new();
    for attachment in toolchain.structural_attachments() {
        validate_toolchain_module(toolchain, attachment.module())?;
        if !attachment_kinds.insert(attachment.attachment()) {
            return Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::DuplicateStructuralAttachment(attachment.attachment()),
            ));
        }
    }
    let mut role_kinds = BTreeSet::new();
    for role in toolchain.standard_roles() {
        validate_toolchain_module(toolchain, role.module())?;
        if !role_kinds.insert(role.role()) {
            return Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::DuplicateStandardRole(role.role()),
            ));
        }
    }
    let mut primitive_kinds = BTreeSet::new();
    for role in toolchain.primitive_roles() {
        validate_toolchain_module(toolchain, role.module())?;
        if !primitive_kinds.insert(role.role()) {
            return Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::DuplicatePrimitiveRole(role.role()),
            ));
        }
    }
    let mut builtin_kinds = BTreeSet::new();
    for builtin in toolchain.builtin_types() {
        validate_toolchain_module(toolchain, builtin.module())?;
        if !builtin_kinds.insert(builtin.builtin()) {
            return Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::DuplicateBuiltinType(builtin.builtin()),
            ));
        }
    }
    validate_toolchain_module(toolchain, toolchain.prelude())
}

fn initial_work(
    packages: &BTreeMap<PackageIdentity, PackageState>,
    roots: &[ModuleIdentity],
    toolchain: &ToolchainInput,
) -> Result<BTreeSet<Work>, DiscoveryError> {
    let mut pending = packages
        .values()
        .filter(|package| package.mode == PackageMode::Declared)
        .map(|package| {
            Work::Module(ModuleIdentity::new(
                package.identity.clone(),
                Vec::<&str>::new(),
            ))
        })
        .collect::<BTreeSet<_>>();
    for root in roots {
        if !packages.contains_key(root.package()) {
            return Err(DiscoveryError::UnknownPackage(root.package().clone()));
        }
        pending.insert(Work::Module(root.clone()));
    }
    pending.insert(Work::Module(toolchain.prelude().clone()));
    pending.extend(
        toolchain
            .structural_attachments()
            .iter()
            .map(|attachment| Work::Module(attachment.module().clone())),
    );
    pending.extend(
        toolchain
            .standard_roles()
            .iter()
            .map(|role| Work::Module(role.module().clone())),
    );
    pending.extend(
        toolchain
            .primitive_roles()
            .iter()
            .map(|role| Work::Module(role.module().clone())),
    );
    pending.extend(
        toolchain
            .builtin_types()
            .iter()
            .map(|builtin| Work::Module(builtin.module().clone())),
    );
    Ok(pending)
}

fn validate_toolchain_module(
    toolchain: &ToolchainInput,
    module: &ModuleIdentity,
) -> Result<(), DiscoveryError> {
    if module.package() == toolchain.standard_package() {
        Ok(())
    } else {
        Err(DiscoveryError::Toolchain(
            ToolchainDiscoveryError::ModuleOutsideStandardPackage(module.clone()),
        ))
    }
}

fn normalized_components(path: &Path) -> Result<Vec<Box<str>>, UseFailure> {
    path.components()
        .map(|component| match component {
            Component::Normal(segment) => segment
                .to_str()
                .map(Box::<str>::from)
                .ok_or(UseFailure::InvalidModuleDirectory),
            Component::CurDir => Ok(Box::<str>::from(".")),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                Err(UseFailure::InvalidModuleDirectory)
            }
        })
        .collect()
}

fn regular_file(source_overlay: &SourceOverlay, path: &Path) -> Result<bool, DiscoveryError> {
    match source_overlay.is_file(path) {
        Ok(is_file) => Ok(is_file),
        Err(error) => Err(DiscoveryError::Filesystem {
            operation: "inspect",
            path: path.into(),
            error,
        }),
    }
}

fn canonicalize(
    source_overlay: &SourceOverlay,
    operation: &'static str,
    path: &Path,
) -> Result<PathBuf, DiscoveryError> {
    source_overlay
        .canonicalize(path)
        .map_err(|error| DiscoveryError::Filesystem {
            operation,
            path: path.into(),
            error,
        })
}

fn canonicalize_dependency(
    source_overlay: &SourceOverlay,
    path: &Path,
) -> Result<PathBuf, DiscoveryError> {
    canonicalize(
        source_overlay,
        "canonicalize source dependency target",
        path,
    )
}

fn canonical_text(path: &Path) -> Result<Box<str>, DiscoveryError> {
    path.to_str()
        .map(Box::<str>::from)
        .ok_or_else(|| DiscoveryError::NonUnicodeCanonicalPath(path.into()))
}

fn use_error(
    declaration: nocter_syntax::NodeId,
    path: &str,
    failure: UseFailure,
) -> DiscoveryError {
    DiscoveryError::Use {
        declaration,
        path: path.into(),
        failure,
    }
}

fn source_visibility_error(
    declaration: nocter_syntax::NodeId,
    path: &str,
    failure: SourceVisibilityFailure,
) -> DiscoveryError {
    DiscoveryError::SourceVisibility {
        declaration,
        path: path.into(),
        failure,
    }
}

fn source_visibility_boundary_error(
    declaration: nocter_syntax::NodeId,
    path: &str,
    error: ResolveError,
) -> DiscoveryError {
    match error {
        ResolveError::Discovery(error) => error,
        ResolveError::Use(UseFailure::CrossesPackage { root }) => source_visibility_error(
            declaration,
            path,
            SourceVisibilityFailure::CrossesPackage { root },
        ),
        ResolveError::Use(
            UseFailure::OutsidePackage
            | UseFailure::InvalidModuleDirectory
            | UseFailure::SingleFileLocalUse
            | UseFailure::UnknownDependency { .. },
        ) => source_visibility_error(declaration, path, SourceVisibilityFailure::OutsidePackage),
        ResolveError::Use(UseFailure::NotFound) => {
            source_visibility_error(declaration, path, SourceVisibilityFailure::NotFound)
        }
    }
}
