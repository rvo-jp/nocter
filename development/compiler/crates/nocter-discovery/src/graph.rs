use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use nocter_compile_input::{
    ModuleIdentity, ModuleSourceKind, PackageIdentity, PackageMode, PrimitiveRoleInput,
    StandardRoleInput, ToolchainInput, UseResolutionInput, UseTargetInput,
};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxElement, SyntaxTree, declaration_name_token, parse};
use nocter_target_selection::TargetSelection;

use crate::DiscoveryError;
use crate::error::{ImportFailure, ToolchainDiscoveryError};
use crate::request::{
    DiscoveryLayout, DiscoveryRequest, PrimitiveRoleLocator, ResolvedPackage, StandardRoleLocator,
    ToolchainRequest,
};
use crate::snapshot::{DiscoveredModule, DiscoveredPackage, DiscoveredSource, DiscoveredUnit};
use crate::syntax::active_use_paths;

#[derive(Debug)]
struct PackageState {
    identity: PackageIdentity,
    display_name: Box<str>,
    mode: PackageMode,
    canonical_root: PathBuf,
    dependencies: BTreeMap<Box<str>, PackageIdentity>,
    declaration: Option<(PathBuf, usize)>,
}

#[derive(Debug)]
struct LoadedPackages {
    states: BTreeMap<PackageIdentity, PackageState>,
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

#[derive(Debug)]
struct Builder {
    target: nocter_model::CompilationTarget,
    packages: BTreeMap<PackageIdentity, PackageState>,
    sources: SourceMap,
    syntax: Vec<SyntaxTree>,
    modules: BTreeMap<ModuleIdentity, Vec<DiscoveredSource>>,
    source_owners: BTreeMap<PathBuf, ModuleIdentity>,
    use_resolutions: Vec<UseResolutionInput>,
    pending: BTreeSet<Work>,
    toolchain: ToolchainRequest,
}

#[derive(Debug)]
enum ResolveError {
    Import(ImportFailure),
    Discovery(DiscoveryError),
}

impl From<ImportFailure> for ResolveError {
    fn from(error: ImportFailure) -> Self {
        Self::Import(error)
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
pub fn discover(request: DiscoveryRequest) -> Result<DiscoveredUnit, DiscoveryError> {
    Builder::new(request)?.run()
}

impl Builder {
    fn new(request: DiscoveryRequest) -> Result<Self, DiscoveryError> {
        let (target, layout, toolchain) = request.into_parts();
        let (loaded, roots, single_file) = match layout {
            DiscoveryLayout::Declared { packages, roots } => {
                (load_packages(packages)?, roots, None)
            }
            DiscoveryLayout::SingleFile {
                source,
                support_packages,
            } => {
                let mut loaded = load_packages(support_packages)?;
                let single_file = load_single_file_package(&mut loaded, source, &toolchain)?;
                (loaded, Vec::new(), Some(single_file))
            }
        };
        let LoadedPackages {
            states: packages,
            sources,
            syntax,
        } = loaded;
        validate_package_dependencies(&packages)?;
        validate_toolchain(&packages, &toolchain)?;
        let mut pending = initial_work(&packages, &roots, &toolchain)?;
        if let Some((module, path)) = single_file {
            pending.insert(Work::SingleFile { module, path });
        }

        Ok(Self {
            target,
            packages,
            sources,
            syntax,
            modules: BTreeMap::new(),
            source_owners: BTreeMap::new(),
            use_resolutions: Vec::new(),
            pending,
            toolchain,
        })
    }

    fn run(mut self) -> Result<DiscoveredUnit, DiscoveryError> {
        while let Some(work) = self.pending.pop_first() {
            match work {
                Work::Module(module) => self.load_module(module)?,
                Work::SingleFile { module, path } => {
                    self.load_module_source(module, &path, ModuleSourceKind::SingleFile)?;
                }
                Work::Source { module, path } => {
                    self.load_module_source(module, &path, ModuleSourceKind::Implementation)?;
                }
            }
        }

        let toolchain = if self.syntax.iter().any(SyntaxTree::has_errors) {
            None
        } else {
            Some(self.resolve_toolchain()?)
        };

        let packages = self
            .packages
            .into_values()
            .map(|state| {
                Ok(DiscoveredPackage {
                    identity: state.identity,
                    display_name: state.display_name,
                    mode: state.mode,
                    declaration: state
                        .declaration
                        .map(|(path, syntax)| Ok((canonical_text(&path)?, syntax)))
                        .transpose()?,
                })
            })
            .collect::<Result<_, DiscoveryError>>()?;
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
        self.use_resolutions.sort_unstable_by_key(|resolution| {
            let node = resolution.declaration();
            (node.source(), node.index())
        });
        Ok(DiscoveredUnit {
            target: self.target,
            sources: self.sources,
            syntax: self.syntax,
            packages,
            modules,
            use_resolutions: self.use_resolutions,
            toolchain,
        })
    }

    fn resolve_toolchain(&self) -> Result<ToolchainInput, DiscoveryError> {
        let mut attachments = self.toolchain.builtin_attachments().to_vec();
        attachments.sort_unstable_by(|left, right| {
            left.attachment()
                .cmp(&right.attachment())
                .then_with(|| left.module().cmp(right.module()))
        });
        let mut locators = self.toolchain.standard_roles().to_vec();
        locators.sort_unstable_by_key(StandardRoleLocator::role);
        let roles = locators
            .iter()
            .map(|locator| self.resolve_standard_role(locator))
            .collect::<Result<_, _>>()?;
        let mut primitive_locators = self.toolchain.primitive_roles().to_vec();
        primitive_locators.sort_unstable_by_key(PrimitiveRoleLocator::role);
        let primitives = primitive_locators
            .iter()
            .map(|locator| self.resolve_primitive_role(locator))
            .collect::<Result<_, _>>()?;
        Ok(ToolchainInput::new(
            self.toolchain.standard_package().clone(),
            self.toolchain.prelude().clone(),
            attachments,
            roles,
        )
        .with_primitive_roles(primitives))
    }

    fn resolve_standard_role(
        &self,
        locator: &StandardRoleLocator,
    ) -> Result<StandardRoleInput, DiscoveryError> {
        let matches = self
            .declaration_matches(locator.module(), locator.kind(), locator.name())
            .ok_or_else(|| {
                DiscoveryError::Toolchain(ToolchainDiscoveryError::MissingRoleDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    kind: locator.kind(),
                    name: locator.name().into(),
                })
            })?;
        match matches.as_slice() {
            [token] => Ok(StandardRoleInput::new(locator.role(), *token)),
            [] => Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::MissingRoleDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    kind: locator.kind(),
                    name: locator.name().into(),
                },
            )),
            _ => Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::AmbiguousRoleDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    kind: locator.kind(),
                    name: locator.name().into(),
                },
            )),
        }
    }

    fn resolve_primitive_role(
        &self,
        locator: &PrimitiveRoleLocator,
    ) -> Result<PrimitiveRoleInput, DiscoveryError> {
        let matches = self
            .declaration_matches(
                locator.module(),
                nocter_syntax::NodeKind::PrimitiveDeclaration,
                locator.name(),
            )
            .ok_or_else(|| {
                DiscoveryError::Toolchain(ToolchainDiscoveryError::MissingPrimitiveDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    name: locator.name().into(),
                })
            })?;
        match matches.as_slice() {
            [token] => Ok(PrimitiveRoleInput::new(locator.role(), *token)),
            [] => Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::MissingPrimitiveDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    name: locator.name().into(),
                },
            )),
            _ => Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::AmbiguousPrimitiveDeclaration {
                    role: locator.role(),
                    module: locator.module().clone(),
                    name: locator.name().into(),
                },
            )),
        }
    }

    fn declaration_matches(
        &self,
        module: &ModuleIdentity,
        kind: nocter_syntax::NodeKind,
        name: &str,
    ) -> Option<Vec<nocter_syntax::SyntaxToken>> {
        let module = self.modules.get(module)?;
        let mut matches = Vec::new();
        for source in module {
            let tree = &self.syntax[source.syntax_index()];
            let mut pending = vec![tree.root_id()];
            while let Some(node) = pending.pop() {
                if tree.node(node).is_some_and(|syntax| syntax.kind() == kind)
                    && let Some(token) = declaration_name_token(tree, node)
                    && self
                        .sources
                        .get(token.source())
                        .and_then(|source| source.text_at(token.range()))
                        == Some(name)
                {
                    matches.push(token);
                }
                for child in tree.children(node).iter().rev() {
                    if let SyntaxElement::Node(child) = child {
                        pending.push(*child);
                    }
                }
            }
        }
        Some(matches)
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
        let root = directory.join("index.nct");
        if !regular_file(&root)? {
            return Err(DiscoveryError::MissingModuleRoot { module, path: root });
        }
        let root = canonicalize("canonicalize module root", &root)?;
        self.validate_inside_package(module.package(), &root, None)?;
        self.load_module_source(module, &root, ModuleSourceKind::Root)
    }

    fn load_module_source(
        &mut self,
        module: ModuleIdentity,
        path: &Path,
        kind: ModuleSourceKind,
    ) -> Result<(), DiscoveryError> {
        let path = canonicalize("canonicalize module source", path)?;
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
        let syntax_index = load_source(
            &mut self.sources,
            &mut self.syntax,
            &path,
            ParseGoal::ModuleSource,
        )?;
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
        if tree.has_errors() {
            return Ok(());
        }
        let selection = TargetSelection::prepare(self.target, &self.sources, [tree])
            .map_err(DiscoveryError::TargetSelection)?;
        let source = self.sources.get(tree.source()).ok_or_else(|| {
            DiscoveryError::TargetSelection(
                nocter_target_selection::TargetSelectionError::MissingSource(tree.source()),
            )
        })?;
        for (declaration, authored_path) in active_use_paths(source, tree, &selection)? {
            let target = self.resolve_use(&module, &path, declaration, &authored_path)?;
            match &target {
                UseTargetInput::Source(path) => {
                    self.pending.insert(Work::Source {
                        module: module.clone(),
                        path: PathBuf::from(path.as_ref()),
                    });
                }
                UseTargetInput::Module(module) => {
                    self.pending.insert(Work::Module(module.clone()));
                }
            }
            self.use_resolutions
                .push(UseResolutionInput::new(declaration, target));
        }
        Ok(())
    }

    fn resolve_use(
        &self,
        importer: &ModuleIdentity,
        source: &Path,
        declaration: nocter_syntax::NodeId,
        authored: &str,
    ) -> Result<UseTargetInput, DiscoveryError> {
        let package = self.package(importer.package())?;
        if package.mode == PackageMode::SingleFile
            && (authored.starts_with("./")
                || authored.starts_with("../")
                || authored.starts_with('/'))
        {
            return Err(import_error(
                declaration,
                authored,
                ImportFailure::SingleFileLocalImport,
            ));
        }
        let segments: Vec<_> = authored.split('/').collect();
        let result = if authored.starts_with("./") || authored.starts_with("../") {
            self.resolve_relative(importer, source, authored)
        } else if authored.starts_with('/') {
            self.resolve_module_candidate(importer.package(), &segments[1..])
        } else {
            let alias = segments[0];
            let Some(target_package) = package.dependencies.get(alias) else {
                return Err(import_error(
                    declaration,
                    authored,
                    ImportFailure::UnknownDependency {
                        alias: alias.into(),
                    },
                ));
            };
            self.resolve_module_candidate(target_package, &segments[1..])
        };
        result.map_err(|error| match error {
            ResolveError::Import(failure) => import_error(declaration, authored, failure),
            ResolveError::Discovery(error) => error,
        })
    }

    fn resolve_relative(
        &self,
        importer: &ModuleIdentity,
        source: &Path,
        authored: &str,
    ) -> Result<UseTargetInput, ResolveError> {
        let package = self
            .packages
            .get(importer.package())
            .ok_or(ImportFailure::OutsidePackage)?;
        let source_directory = source.parent().ok_or(ImportFailure::OutsidePackage)?;
        let relative = source_directory
            .strip_prefix(&package.canonical_root)
            .map_err(|_| ImportFailure::OutsidePackage)?;
        let mut components = normalized_components(relative)?;
        for component in authored.split('/') {
            match component {
                "." => {}
                ".." => {
                    components.pop().ok_or(ImportFailure::OutsidePackage)?;
                }
                segment => components.push(segment.into()),
            }
        }
        let base = components
            .iter()
            .fold(package.canonical_root.clone(), |path, segment| {
                path.join(Path::new(segment.as_ref()))
            });
        let mut source_candidate = base.clone();
        source_candidate.set_extension("nct");
        let module_candidate = base.join("index.nct");
        let has_source = regular_file(&source_candidate)?;
        let has_module = regular_file(&module_candidate)?;
        match (has_source, has_module) {
            (true, true) => Err(ImportFailure::Ambiguous {
                source: source_candidate,
                module: module_candidate,
            }
            .into()),
            (false, false) => Err(ImportFailure::NotFound.into()),
            (false, true) => self.resolve_existing_module(importer.package(), &module_candidate),
            (true, false) => {
                let source_candidate = canonicalize_import(&source_candidate)?;
                self.validate_import_path(importer.package(), &source_candidate)?;
                let owner = self.nearest_module(importer.package(), &source_candidate)?;
                if &owner != importer {
                    return Err(ImportFailure::CrossesModule { module: owner }.into());
                }
                Ok(UseTargetInput::Source(canonical_text(&source_candidate)?))
            }
        }
    }

    fn resolve_module_candidate(
        &self,
        package: &PackageIdentity,
        segments: &[&str],
    ) -> Result<UseTargetInput, ResolveError> {
        let state = self
            .packages
            .get(package)
            .ok_or(ImportFailure::OutsidePackage)?;
        let root = segments
            .iter()
            .fold(state.canonical_root.clone(), |path, segment| {
                path.join(segment)
            })
            .join("index.nct");
        if !regular_file(&root)? {
            return Err(ImportFailure::NotFound.into());
        }
        self.resolve_existing_module(package, &root)
    }

    fn resolve_existing_module(
        &self,
        package: &PackageIdentity,
        root: &Path,
    ) -> Result<UseTargetInput, ResolveError> {
        let root = canonicalize_import(root)?;
        self.validate_import_path(package, &root)?;
        let state = self
            .packages
            .get(package)
            .ok_or(ImportFailure::OutsidePackage)?;
        let directory = root.parent().ok_or(ImportFailure::InvalidModuleDirectory)?;
        let relative = directory
            .strip_prefix(&state.canonical_root)
            .map_err(|_| ImportFailure::OutsidePackage)?;
        let path = normalized_components(relative)?;
        Ok(UseTargetInput::Module(ModuleIdentity::new(
            package.clone(),
            path.iter().map(AsRef::as_ref),
        )))
    }

    fn nearest_module(
        &self,
        package: &PackageIdentity,
        source: &Path,
    ) -> Result<ModuleIdentity, ResolveError> {
        let state = self
            .packages
            .get(package)
            .ok_or(ImportFailure::OutsidePackage)?;
        let mut directory = source.parent().ok_or(ImportFailure::OutsidePackage)?;
        loop {
            if regular_file(&directory.join("index.nct"))? {
                let relative = directory
                    .strip_prefix(&state.canonical_root)
                    .map_err(|_| ImportFailure::OutsidePackage)?;
                let path = normalized_components(relative)?;
                return Ok(ModuleIdentity::new(
                    package.clone(),
                    path.iter().map(AsRef::as_ref),
                ));
            }
            if directory == state.canonical_root {
                return Err(ImportFailure::InvalidModuleDirectory.into());
            }
            directory = directory.parent().ok_or(ImportFailure::OutsidePackage)?;
        }
    }

    fn package(&self, identity: &PackageIdentity) -> Result<&PackageState, DiscoveryError> {
        self.packages
            .get(identity)
            .ok_or_else(|| DiscoveryError::UnknownPackage(identity.clone()))
    }

    fn validate_inside_package(
        &self,
        package: &PackageIdentity,
        path: &Path,
        expected_module: Option<&ModuleIdentity>,
    ) -> Result<(), DiscoveryError> {
        let state = self.package(package)?;
        if !path.starts_with(&state.canonical_root) {
            return Err(DiscoveryError::InvalidPackageRoot {
                package: package.clone(),
                path: path.to_path_buf(),
            });
        }
        if let Some(module) = expected_module {
            if let Err(error) = self.validate_import_path(package, path) {
                return Err(match error {
                    ResolveError::Import(failure) => DiscoveryError::InvalidModulePath {
                        module: module.clone(),
                        path: path.into(),
                        failure,
                    },
                    ResolveError::Discovery(error) => error,
                });
            }
            let root = path.parent().unwrap_or(path);
            let relative = root.strip_prefix(&state.canonical_root).map_err(|_| {
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

    fn validate_import_path(
        &self,
        package: &PackageIdentity,
        path: &Path,
    ) -> Result<(), ResolveError> {
        let state = self
            .packages
            .get(package)
            .ok_or(ImportFailure::OutsidePackage)?;
        if !path.starts_with(&state.canonical_root) {
            return Err(ImportFailure::OutsidePackage.into());
        }
        let mut directory = path.parent().ok_or(ImportFailure::OutsidePackage)?;
        while directory != state.canonical_root {
            let nested = directory.join("nocter.nct");
            if regular_file(&nested)? {
                return Err(ImportFailure::CrossesPackage {
                    root: directory.into(),
                }
                .into());
            }
            directory = directory.parent().ok_or(ImportFailure::OutsidePackage)?;
        }
        Ok(())
    }
}

fn load_source(
    sources: &mut SourceMap,
    syntax: &mut Vec<SyntaxTree>,
    path: &Path,
    goal: ParseGoal,
) -> Result<usize, DiscoveryError> {
    let bytes = fs::read(path).map_err(|error| DiscoveryError::Filesystem {
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
    let tree = parse(
        sources
            .get(source)
            .expect("newly allocated source remains in the source map"),
        goal,
    );
    let index = syntax.len();
    syntax.push(tree);
    Ok(index)
}

fn join_module_path(root: &Path, path: &[Box<str>]) -> PathBuf {
    path.iter().fold(root.to_path_buf(), |path, segment| {
        path.join(Path::new(segment.as_ref()))
    })
}

fn load_packages(
    mut package_specs: Vec<ResolvedPackage>,
) -> Result<LoadedPackages, DiscoveryError> {
    package_specs.sort_unstable_by(|left, right| left.identity().cmp(right.identity()));
    let mut packages = BTreeMap::new();
    let mut root_owners = BTreeMap::new();
    let mut sources = SourceMap::new();
    let mut syntax = Vec::new();
    for package in package_specs {
        let identity = package.identity().clone();
        if packages.contains_key(&identity) {
            return Err(DiscoveryError::DuplicatePackage(identity));
        }
        let canonical_root = canonicalize("canonicalize package root", package.root())?;
        if !canonical_root.is_dir() {
            return Err(DiscoveryError::InvalidPackageRoot {
                package: identity,
                path: canonical_root,
            });
        }
        if let Some(first) = root_owners.insert(canonical_root.clone(), identity.clone()) {
            return Err(DiscoveryError::DuplicateCanonicalRoot {
                first,
                second: identity,
                path: canonical_root,
            });
        }
        let declaration_path = canonical_root.join("nocter.nct");
        if !regular_file(&declaration_path)? {
            return Err(DiscoveryError::MissingPackageFile {
                package: identity,
                path: declaration_path,
            });
        }
        let declaration_path = canonicalize("canonicalize package file", &declaration_path)?;
        if !declaration_path.starts_with(&canonical_root) {
            return Err(DiscoveryError::InvalidPackageRoot {
                package: identity,
                path: declaration_path,
            });
        }
        let declaration_syntax = load_source(
            &mut sources,
            &mut syntax,
            &declaration_path,
            ParseGoal::PackageFile,
        )?;
        packages.insert(
            identity,
            PackageState {
                identity: package.identity().clone(),
                display_name: package.display_name().into(),
                mode: PackageMode::Declared,
                canonical_root,
                dependencies: package.dependencies().clone(),
                declaration: Some((declaration_path, declaration_syntax)),
            },
        );
    }
    Ok(LoadedPackages {
        states: packages,
        sources,
        syntax,
    })
}

fn load_single_file_package(
    loaded: &mut LoadedPackages,
    source: PathBuf,
    toolchain: &ToolchainRequest,
) -> Result<(ModuleIdentity, PathBuf), DiscoveryError> {
    if source.extension().and_then(|extension| extension.to_str()) != Some("nct") {
        return Err(DiscoveryError::InvalidSingleFileExtension(source));
    }
    let source = canonicalize("canonicalize single-file input", &source)?;
    if !regular_file(&source)? {
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
            declaration: None,
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
    toolchain: &ToolchainRequest,
) -> Result<(), DiscoveryError> {
    if !packages.contains_key(toolchain.standard_package()) {
        return Err(DiscoveryError::UnknownPackage(
            toolchain.standard_package().clone(),
        ));
    }
    let mut attachment_kinds = BTreeSet::new();
    for attachment in toolchain.builtin_attachments() {
        validate_toolchain_module(toolchain, attachment.module())?;
        if !attachment_kinds.insert(attachment.attachment()) {
            return Err(DiscoveryError::Toolchain(
                ToolchainDiscoveryError::DuplicateBuiltinAttachment(attachment.attachment()),
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
    validate_toolchain_module(toolchain, toolchain.prelude())
}

fn initial_work(
    packages: &BTreeMap<PackageIdentity, PackageState>,
    roots: &[ModuleIdentity],
    toolchain: &ToolchainRequest,
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
            .builtin_attachments()
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
    Ok(pending)
}

fn validate_toolchain_module(
    toolchain: &ToolchainRequest,
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

fn normalized_components(path: &Path) -> Result<Vec<Box<str>>, ImportFailure> {
    path.components()
        .map(|component| match component {
            Component::Normal(segment) => segment
                .to_str()
                .map(Box::<str>::from)
                .ok_or(ImportFailure::InvalidModuleDirectory),
            Component::CurDir => Ok(Box::<str>::from(".")),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                Err(ImportFailure::InvalidModuleDirectory)
            }
        })
        .collect()
}

fn regular_file(path: &Path) -> Result<bool, DiscoveryError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(DiscoveryError::Filesystem {
            operation: "inspect",
            path: path.into(),
            error,
        }),
    }
}

fn canonicalize(operation: &'static str, path: &Path) -> Result<PathBuf, DiscoveryError> {
    fs::canonicalize(path).map_err(|error| DiscoveryError::Filesystem {
        operation,
        path: path.into(),
        error,
    })
}

fn canonicalize_import(path: &Path) -> Result<PathBuf, DiscoveryError> {
    canonicalize("canonicalize import target", path)
}

fn canonical_text(path: &Path) -> Result<Box<str>, DiscoveryError> {
    path.to_str()
        .map(Box::<str>::from)
        .ok_or_else(|| DiscoveryError::NonUnicodeCanonicalPath(path.into()))
}

fn import_error(
    declaration: nocter_syntax::NodeId,
    path: &str,
    failure: ImportFailure,
) -> DiscoveryError {
    DiscoveryError::Import {
        declaration,
        path: path.into(),
        failure,
    }
}
