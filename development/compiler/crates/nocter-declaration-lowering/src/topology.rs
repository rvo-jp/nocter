use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    AcceptedDeclarationProgram, DeclarationProgram, DeclarationProgramBuilder, ModuleNamespace,
    ModulePath, ProgramBuildError,
};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{ModuleId, SymbolTable};
use nocter_runtime_contract::PrimitiveBinding;
use nocter_source::SourceId;
use nocter_source_index::{
    SemanticEntity, SourceIndex, SourceIndexBuilder, SourceOrigin, SourceRole,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, TokenKind};

use crate::package_source::validate_package_directive_ownership;
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, SourceVisibilityResolutionInput, TopologyViolation,
    UseResolutionInput,
};
use nocter_target_selection::{TargetSelection, TargetSelectionError};

pub(crate) type SourceVisibilityResolutionKey = (SourceId, usize);
pub(crate) type UseResolutionKey = (SourceId, usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UseScope {
    Module,
    Block,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PreparedUseResolution<'input> {
    input: &'input UseResolutionInput,
    scope: UseScope,
}

impl PreparedUseResolution<'_> {
    pub(crate) const fn declaration(self) -> NodeId {
        self.input.declaration()
    }

    pub(crate) const fn target_module(&self) -> &ModuleIdentity {
        self.input.target_module()
    }

    pub(crate) const fn scope(self) -> UseScope {
        self.scope
    }
}

#[derive(Debug)]
pub struct LoweredDeclarations {
    reusable: ReusableDeclarations,
    current_symbols: crate::current_symbols::CurrentCheckingSymbols,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

/// Source-neutral declaration query result reusable across equal module surfaces.
///
/// This owner contains no generation-local source or syntax identity. Each checking request takes
/// an explicit owned branch; source presentation must be materialized from the paired recipe for
/// the current generation.
#[derive(Debug)]
pub struct ReusableDeclarations {
    program: AcceptedDeclarationProgram,
    primitive_bindings: Box<[PrimitiveBinding]>,
    module_bindings: Box<[(ModuleIdentity, ModuleId)]>,
    projection_recipe: crate::projection_recipe::FrontendProjectionRecipe,
}

/// Topology-only output used by the focused topology pass.
#[derive(Debug)]
pub struct LoweredTopology {
    program: AcceptedDeclarationProgram,
    source_index: SourceIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageTargetResolutionError {
    Duplicate(NodeId),
    Invalid(NodeId),
    UnknownModule(NodeId),
    OutsidePackage(NodeId),
}

impl fmt::Display for PackageTargetResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate(declaration) => write!(
                formatter,
                "package target {declaration:?} has more than one resolved module"
            ),
            Self::Invalid(declaration) => write!(
                formatter,
                "package target resolution {declaration:?} does not identify a selected target directive"
            ),
            Self::UnknownModule(declaration) => write!(
                formatter,
                "package target {declaration:?} resolves outside the compile-unit module graph"
            ),
            Self::OutsidePackage(declaration) => write!(
                formatter,
                "package target {declaration:?} resolves to a module in another package"
            ),
        }
    }
}

impl std::error::Error for PackageTargetResolutionError {}

impl LoweredDeclarations {
    pub(crate) const fn new(
        program: AcceptedDeclarationProgram,
        frontend_bindings: FrontendBindings,
        source_index: SourceIndex,
        primitive_bindings: Box<[PrimitiveBinding]>,
        module_bindings: Box<[(ModuleIdentity, ModuleId)]>,
        projection_recipe: crate::projection_recipe::FrontendProjectionRecipe,
        current_symbols: crate::current_symbols::CurrentCheckingSymbols,
    ) -> Self {
        Self {
            reusable: ReusableDeclarations {
                program,
                primitive_bindings,
                module_bindings,
                projection_recipe,
            },
            current_symbols,
            frontend_bindings,
            source_index,
        }
    }

    #[must_use]
    pub const fn program(&self) -> &DeclarationProgram {
        self.reusable.program()
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub const fn primitive_bindings(&self) -> &[PrimitiveBinding] {
        self.reusable.primitive_bindings()
    }

    /// Returns the source-neutral projection recipe emitted with this semantic program.
    #[must_use]
    pub const fn projection_recipe(&self) -> &crate::FrontendProjectionRecipe {
        self.reusable.projection_recipe()
    }

    /// Discards current-generation projection after extracting the reusable declaration result.
    #[must_use]
    pub fn into_reusable(self) -> ReusableDeclarations {
        self.reusable
    }

    #[must_use]
    pub fn into_parts(self) -> (AcceptedDeclarationProgram, SourceIndex) {
        (
            self.current_symbols.extend_accepted(self.reusable.program),
            self.source_index,
        )
    }

    /// Separates semantic checking input from the independently retained presentation index.
    #[must_use]
    pub fn into_checking_parts(
        self,
    ) -> (AcceptedDeclarationProgram, FrontendBindings, SourceIndex) {
        (
            self.current_symbols.extend_accepted(self.reusable.program),
            self.frontend_bindings,
            self.source_index,
        )
    }
}

impl ReusableDeclarations {
    #[must_use]
    pub const fn program(&self) -> &DeclarationProgram {
        self.program.program()
    }

    /// Opens one owned checking branch without rebuilding declaration decisions.
    #[must_use]
    pub fn checking_branch(&self) -> AcceptedDeclarationProgram {
        self.program.checking_branch()
    }

    #[must_use]
    pub const fn primitive_bindings(&self) -> &[PrimitiveBinding] {
        &self.primitive_bindings
    }

    #[must_use]
    pub const fn projection_recipe(&self) -> &crate::FrontendProjectionRecipe {
        &self.projection_recipe
    }

    pub(crate) fn module_binding(&self, identity: &ModuleIdentity) -> Option<ModuleId> {
        self.module_bindings
            .binary_search_by(|(candidate, _)| candidate.cmp(identity))
            .ok()
            .map(|index| self.module_bindings[index].1)
    }
}

impl LoweredTopology {
    const fn new(program: AcceptedDeclarationProgram, source_index: SourceIndex) -> Self {
        Self {
            program,
            source_index,
        }
    }

    #[must_use]
    pub const fn program(&self) -> &DeclarationProgram {
        self.program.program()
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoweringError {
    Rule(TopologyViolation),
    DuplicatePackage(PackageIdentity),
    DuplicateModule(ModuleIdentity),
    DuplicateSourcePath(Box<str>),
    DuplicateSource(SourceId),
    UnknownPackage(PackageIdentity),
    MissingSource(SourceId),
    InvalidModuleSource(Box<str>),
    InconsistentSyntax(SourceId),
    MissingCollectedSymbol(Box<str>),
    InvalidModuleSegment(Box<str>),
    InvalidModuleLayout(ModuleIdentity),
    InvalidPackageModuleSet(PackageIdentity),
    InvalidSingleFilePackage(PackageIdentity),
    PackageTargetResolution(PackageTargetResolutionError),
    MissingSourceVisibilityResolution(NodeId),
    DuplicateSourceVisibilityResolution(NodeId),
    InvalidSourceVisibilityResolution(NodeId),
    UnknownSourceVisibilityTarget {
        declaration: NodeId,
        target: Box<str>,
    },
    MissingUseResolution(NodeId),
    UnknownTargetGate(NodeId),
    DuplicateUseResolution(NodeId),
    InvalidUseResolution(NodeId),
    UnknownUseTarget(NodeId),
    Program(ProgramBuildError),
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(result) = format_resolution_error(self, formatter) {
            return result;
        }
        match self {
            Self::PackageTargetResolution(error) => error.fmt(formatter),
            Self::Rule(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::DuplicatePackage(package) => {
                write!(formatter, "duplicate package identity {}", package.as_str())
            }
            Self::DuplicateModule(module) => {
                write!(formatter, "duplicate module identity {module:?}")
            }
            Self::DuplicateSourcePath(path) => {
                write!(formatter, "duplicate canonical source path {path}")
            }
            Self::DuplicateSource(source) => {
                write!(
                    formatter,
                    "{source} is claimed by more than one physical input"
                )
            }
            Self::UnknownPackage(package) => {
                write!(
                    formatter,
                    "module names unknown package {}",
                    package.as_str()
                )
            }
            Self::MissingSource(source) => {
                write!(formatter, "{source} is absent from the source map")
            }
            Self::InvalidModuleSource(path) => {
                write!(formatter, "{path} is not a module-source syntax tree")
            }
            Self::InconsistentSyntax(source) => {
                write!(formatter, "{source} has syntax outside its source snapshot")
            }
            Self::MissingCollectedSymbol(spelling) => {
                write!(formatter, "collected symbol table is missing {spelling}")
            }
            Self::InvalidModuleSegment(segment) => {
                write!(formatter, "invalid normalized module segment {segment}")
            }
            Self::InvalidModuleLayout(module) => {
                write!(
                    formatter,
                    "invalid root/implementation layout for {module:?}"
                )
            }
            Self::InvalidPackageModuleSet(package) => write!(
                formatter,
                "package {} does not contain one root module with sources matching its mode",
                package.as_str()
            ),
            Self::InvalidSingleFilePackage(package) => write!(
                formatter,
                "single-file package {} does not contain exactly one single-file module",
                package.as_str()
            ),
            Self::MissingSourceVisibilityResolution(_)
            | Self::DuplicateSourceVisibilityResolution(_)
            | Self::InvalidSourceVisibilityResolution(_)
            | Self::UnknownSourceVisibilityTarget { .. }
            | Self::MissingUseResolution(_)
            | Self::DuplicateUseResolution(_)
            | Self::InvalidUseResolution(_)
            | Self::UnknownUseTarget(_) => unreachable!("resolution errors returned above"),
            Self::UnknownTargetGate(literal) => {
                write!(formatter, "unknown compilation target in {literal:?}")
            }
            Self::Program(error) => error.fmt(formatter),
        }
    }
}

fn format_resolution_error(
    error: &LoweringError,
    formatter: &mut fmt::Formatter<'_>,
) -> Option<fmt::Result> {
    let (declaration, message) = match error {
        LoweringError::MissingSourceVisibilityResolution(node) => {
            (*node, "see declaration has no resolved source")
        }
        LoweringError::DuplicateSourceVisibilityResolution(node) => {
            (*node, "see declaration has more than one resolved source")
        }
        LoweringError::InvalidSourceVisibilityResolution(node) => (
            *node,
            "resolved see does not identify an authored see declaration",
        ),
        LoweringError::UnknownSourceVisibilityTarget {
            declaration,
            target: _,
        } => (
            *declaration,
            "resolved see names a source outside the compile unit",
        ),
        LoweringError::MissingUseResolution(node) => {
            (*node, "use declaration has no resolved target")
        }
        LoweringError::DuplicateUseResolution(node) => {
            (*node, "use declaration has more than one resolved target")
        }
        LoweringError::InvalidUseResolution(node) => (
            *node,
            "resolved use does not identify an authored use declaration",
        ),
        LoweringError::UnknownUseTarget(node) => (
            *node,
            "resolved use names a target outside the compile unit",
        ),
        _ => return None,
    };
    Some(resolution_message(formatter, declaration, message))
}

fn resolution_message(
    formatter: &mut fmt::Formatter<'_>,
    declaration: NodeId,
    message: &str,
) -> fmt::Result {
    write!(formatter, "{message}: {declaration:?}")
}

impl std::error::Error for LoweringError {}

impl From<ProgramBuildError> for LoweringError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<TopologyViolation> for LoweringError {
    fn from(violation: TopologyViolation) -> Self {
        Self::Rule(violation)
    }
}

/// Lowers canonical package, module, and physical-source topology without resolving declarations.
///
/// # Errors
///
/// Returns [`LoweringError`] when the explicit compile-unit graph is incomplete, ambiguous, or
/// inconsistent with its parsed source snapshots.
pub fn lower_compile_unit_topology(
    input: &CompileUnitInput<'_>,
) -> Result<LoweredTopology, LoweringError> {
    let prepared = prepare_compile_unit(input)?;
    let mut program = DeclarationProgramBuilder::new(input.target(), prepared.symbols);
    let mut source_index = SourceIndexBuilder::new();
    let mut package_ids = BTreeMap::new();

    for package in &prepared.packages {
        let display_name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| LoweringError::MissingCollectedSymbol(package.display_name().into()))?;
        let id = program.add_package(package.identity().clone(), display_name)?;
        package_ids.insert(package.identity().clone(), id);
        if package.mode() == PackageMode::Declared {
            let tree = package_root_source(package.identity(), &prepared.modules)
                .ok_or_else(|| LoweringError::InvalidPackageModuleSet(package.identity().clone()))?
                .syntax();
            source_index.insert(
                SemanticEntity::Package(id),
                SourceRole::Declaration,
                SourceOrigin::from_node(tree, tree.root_id())
                    .map_err(|_| LoweringError::InconsistentSyntax(tree.source()))?,
            );
        }
    }
    let root_packages = input
        .root_packages()
        .iter()
        .map(|identity| {
            package_ids
                .get(identity)
                .copied()
                .ok_or_else(|| LoweringError::UnknownPackage(identity.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    program.set_root_packages(root_packages)?;

    for module in &prepared.modules {
        let package = *package_ids
            .get(module.identity().package())
            .ok_or_else(|| LoweringError::UnknownPackage(module.identity().package().clone()))?;
        let path = module
            .identity()
            .path()
            .iter()
            .map(|segment| {
                program
                    .symbols()
                    .get(segment)
                    .ok_or_else(|| LoweringError::MissingCollectedSymbol(segment.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let id = program.add_module(package, ModulePath::from_segments(path))?;
        program.define_module_namespace(id, ModuleNamespace::default())?;
        project_module_sources(&mut source_index, id, module)?;
    }

    Ok(LoweredTopology::new(
        program.finish()?,
        source_index.finish(),
    ))
}

pub(crate) struct PreparedCompileUnit<'input, 'syntax> {
    pub(crate) symbols: SymbolTable,
    pub(crate) packages: Vec<&'input PackageInput>,
    pub(crate) modules: Vec<&'input ModuleInput<'syntax>>,
    pub(crate) source_visibility_resolutions:
        BTreeMap<SourceVisibilityResolutionKey, &'input SourceVisibilityResolutionInput>,
    pub(crate) use_resolutions: BTreeMap<UseResolutionKey, PreparedUseResolution<'input>>,
    pub(crate) package_target_resolutions: Vec<&'input crate::PackageTargetResolutionInput>,
    pub(crate) target_selection: &'input TargetSelection,
}

pub(crate) fn prepare_compile_unit<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
) -> Result<PreparedCompileUnit<'input, 'syntax>, LoweringError> {
    let packages = canonical_packages(input)?;
    let modules = canonical_modules(input, &packages)?;
    validate_sources(input, &packages, &modules)?;
    let target_selection = input.target_selection().map_err(|error| match error {
        TargetSelectionError::MissingSource(source) => LoweringError::MissingSource(source),
        TargetSelectionError::InconsistentSyntax(source) => {
            LoweringError::InconsistentSyntax(source)
        }
        TargetSelectionError::UnknownTarget(literal) => LoweringError::UnknownTargetGate(literal),
    })?;
    let package_target_resolutions =
        validate_package_target_resolutions(input, &packages, &modules)?;
    let source_visibility_resolutions = validate_source_visibility_resolutions(input, &modules)?;
    let use_resolutions = validate_use_resolutions(input, &modules, target_selection)?;
    let symbols = collect_symbols(input, &packages, &modules, target_selection)?;
    Ok(PreparedCompileUnit {
        symbols,
        packages,
        modules,
        source_visibility_resolutions,
        use_resolutions,
        package_target_resolutions,
        target_selection,
    })
}

fn validate_package_target_resolutions<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    packages: &[&'input PackageInput],
    modules: &[&'input ModuleInput<'syntax>],
) -> Result<Vec<&'input crate::PackageTargetResolutionInput>, LoweringError> {
    let package_sources: BTreeMap<_, _> = packages
        .iter()
        .filter(|package| package.mode() == PackageMode::Declared)
        .filter_map(|package| {
            package_root_source(package.identity(), modules)
                .map(|source| (source.syntax().source(), package.identity()))
        })
        .collect();
    let module_packages: BTreeMap<_, _> = modules
        .iter()
        .map(|module| (module.identity(), module.identity().package()))
        .collect();
    let mut resolutions: Vec<_> = input.package_target_resolutions().iter().collect();
    resolutions.sort_unstable_by_key(|resolution| {
        let declaration = resolution.declaration();
        (declaration.source(), declaration.index())
    });
    for pair in resolutions.windows(2) {
        if pair[0].declaration() == pair[1].declaration() {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::Duplicate(pair[0].declaration()),
            ));
        }
    }
    let mut selected_orders = BTreeSet::new();
    for resolution in &resolutions {
        let declaration = resolution.declaration();
        let Some(package) = package_sources.get(&declaration.source()).copied() else {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::Invalid(declaration),
            ));
        };
        let tree = modules
            .iter()
            .flat_map(|module| module.sources())
            .find(|source| {
                source.kind() == ModuleSourceKind::Root
                    && source.syntax().source() == declaration.source()
            })
            .map(ModuleSourceInput::syntax)
            .ok_or(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::Invalid(declaration),
            ))?;
        if tree
            .node(declaration)
            .is_none_or(|node| node.kind() != NodeKind::PackageDirective)
            || !tree
                .children(tree.root_id())
                .iter()
                .any(|child| matches!(child, SyntaxElement::Node(node) if *node == declaration))
            || tree
                .node(resolution.name_literal())
                .is_none_or(|node| node.kind() != NodeKind::StringLiteral)
            || !contains_node(tree, declaration, resolution.name_literal())
        {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::Invalid(declaration),
            ));
        }
        let Some(module_package) = module_packages.get(resolution.module()).copied() else {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::UnknownModule(declaration),
            ));
        };
        if module_package != package {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::OutsidePackage(declaration),
            ));
        }
        if !selected_orders.insert((package, resolution.declaration_order())) {
            return Err(LoweringError::PackageTargetResolution(
                PackageTargetResolutionError::Invalid(declaration),
            ));
        }
    }
    resolutions.sort_unstable_by(|left, right| {
        left.module()
            .package()
            .cmp(right.module().package())
            .then_with(|| left.declaration().index().cmp(&right.declaration().index()))
    });
    Ok(resolutions)
}

fn contains_node(tree: &nocter_syntax::SyntaxTree, root: NodeId, expected: NodeId) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if node == expected {
            return true;
        }
        pending.extend(
            tree.children(node)
                .iter()
                .filter_map(|element| match element {
                    SyntaxElement::Node(child) => Some(*child),
                    SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
                }),
        );
    }
    false
}

fn package_root_source<'input, 'syntax>(
    package: &PackageIdentity,
    modules: &[&'input ModuleInput<'syntax>],
) -> Option<&'input ModuleSourceInput<'syntax>> {
    modules
        .iter()
        .find(|module| {
            module.identity().package() == package && module.identity().path().is_empty()
        })?
        .sources()
        .iter()
        .find(|source| source.kind() == ModuleSourceKind::Root)
}

fn canonical_packages<'input>(
    input: &'input CompileUnitInput<'_>,
) -> Result<Vec<&'input PackageInput>, LoweringError> {
    let mut packages: Vec<_> = input.packages().iter().collect();
    packages.sort_unstable_by_key(|package| package.identity());
    if let Some(pair) = packages
        .windows(2)
        .find(|pair| pair[0].identity() == pair[1].identity())
    {
        return Err(LoweringError::DuplicatePackage(pair[0].identity().clone()));
    }
    Ok(packages)
}

fn canonical_modules<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    packages: &[&PackageInput],
) -> Result<Vec<&'input ModuleInput<'syntax>>, LoweringError> {
    let package_ids: BTreeSet<_> = packages.iter().map(|package| package.identity()).collect();
    let mut modules: Vec<_> = input.modules().iter().collect();
    modules.sort_unstable_by_key(|module| module.identity());
    for module in &modules {
        if !package_ids.contains(module.identity().package()) {
            return Err(LoweringError::UnknownPackage(
                module.identity().package().clone(),
            ));
        }
        for segment in module.identity().path() {
            if !is_module_segment(segment) {
                return Err(LoweringError::InvalidModuleSegment(segment.clone()));
            }
        }
    }
    if let Some(pair) = modules
        .windows(2)
        .find(|pair| pair[0].identity() == pair[1].identity())
    {
        return Err(LoweringError::DuplicateModule(pair[0].identity().clone()));
    }
    Ok(modules)
}

fn validate_sources(
    input: &CompileUnitInput<'_>,
    packages: &[&PackageInput],
    modules: &[&ModuleInput<'_>],
) -> Result<(), LoweringError> {
    let mut paths = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let package_modes: BTreeMap<_, _> = packages
        .iter()
        .map(|package| (package.identity(), package.mode()))
        .collect();
    for module in modules {
        let mut roots = 0;
        let mut single_files = 0;
        for source in module.sources() {
            if source.syntax().root().kind() != NodeKind::SourceFile {
                return Err(LoweringError::InvalidModuleSource(
                    source.canonical_path().into(),
                ));
            }
            require_source(input, source.syntax().source())?;
            if !source_ids.insert(source.syntax().source()) {
                return Err(LoweringError::DuplicateSource(source.syntax().source()));
            }
            if !paths.insert(source.canonical_path()) {
                return Err(LoweringError::DuplicateSourcePath(
                    source.canonical_path().into(),
                ));
            }
            match source.kind() {
                ModuleSourceKind::Root => roots += 1,
                ModuleSourceKind::Implementation => {}
                ModuleSourceKind::SingleFile => single_files += 1,
            }
        }
        let package_mode = *package_modes
            .get(module.identity().package())
            .ok_or_else(|| LoweringError::UnknownPackage(module.identity().package().clone()))?;
        let valid = match package_mode {
            PackageMode::Declared => roots == 1 && single_files == 0,
            PackageMode::SingleFile => {
                roots == 0 && single_files == 1 && module.sources().len() == 1
            }
        };
        if !valid {
            return Err(LoweringError::InvalidModuleLayout(
                module.identity().clone(),
            ));
        }
    }
    let mut package_declaration_sources = BTreeSet::new();
    for package in packages {
        let package_modules: Vec<_> = modules
            .iter()
            .filter(|module| module.identity().package() == package.identity())
            .collect();
        if package_modules
            .iter()
            .filter(|module| module.identity().path().is_empty())
            .count()
            != 1
        {
            return Err(LoweringError::InvalidPackageModuleSet(
                package.identity().clone(),
            ));
        }
        if package.mode() == PackageMode::Declared {
            let root = package_modules
                .iter()
                .find(|module| module.identity().path().is_empty())
                .expect("validated declared package has one root module");
            let root_source = root
                .sources()
                .iter()
                .find(|source| source.kind() == ModuleSourceKind::Root)
                .expect("validated declared module has one root source");
            package_declaration_sources.insert(root_source.syntax().source());
        }
        if package.mode() == PackageMode::SingleFile
            && (package_modules.len() != 1
                || package_modules[0].sources().len() != 1
                || package_modules[0].sources()[0].kind() != ModuleSourceKind::SingleFile)
        {
            return Err(LoweringError::InvalidSingleFilePackage(
                package.identity().clone(),
            ));
        }
    }
    validate_package_directive_ownership(&package_declaration_sources, modules)?;
    Ok(())
}

fn validate_source_visibility_resolutions<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    modules: &[&'input ModuleInput<'syntax>],
) -> Result<
    BTreeMap<SourceVisibilityResolutionKey, &'input SourceVisibilityResolutionInput>,
    LoweringError,
> {
    let mut authored = BTreeMap::new();
    let mut source_owners = BTreeMap::new();
    let mut path_owners = BTreeMap::new();

    for module in modules {
        for source in module.sources() {
            source_owners.insert(source.syntax().source(), module.identity());
            path_owners.insert(source.canonical_path(), (module.identity(), source));
            collect_source_visibility_nodes(source.syntax(), &mut authored)?;
        }
    }

    let mut resolved = BTreeMap::new();
    let mut input_resolutions: Vec<_> = input.source_visibility_resolutions().iter().collect();
    input_resolutions.sort_unstable_by(|left, right| {
        resolution_key(left.declaration())
            .cmp(&resolution_key(right.declaration()))
            .then_with(|| left.target_source().cmp(right.target_source()))
    });
    for resolution in input_resolutions {
        let declaration = resolution.declaration();
        let key = resolution_key(declaration);
        if resolved.insert(key, resolution).is_some() {
            return Err(LoweringError::DuplicateSourceVisibilityResolution(
                declaration,
            ));
        }
        if !authored.contains_key(&key) {
            return Err(LoweringError::InvalidSourceVisibilityResolution(
                declaration,
            ));
        }
        let importing_module = source_owners.get(&declaration.source()).copied().ok_or(
            LoweringError::InvalidSourceVisibilityResolution(declaration),
        )?;
        let (target_module, _) = path_owners
            .get(resolution.target_source())
            .copied()
            .ok_or_else(|| LoweringError::UnknownSourceVisibilityTarget {
                declaration,
                target: resolution.target_source().into(),
            })?;
        if importing_module != target_module {
            return Err(TopologyViolation::invalid_source_see(declaration).into());
        }
    }
    if let Some((_, declaration)) = authored.iter().find(|(key, _)| !resolved.contains_key(key)) {
        return Err(LoweringError::MissingSourceVisibilityResolution(
            *declaration,
        ));
    }

    Ok(resolved)
}

fn validate_use_resolutions<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    modules: &[&'input ModuleInput<'syntax>],
    target_selection: &TargetSelection,
) -> Result<BTreeMap<UseResolutionKey, PreparedUseResolution<'input>>, LoweringError> {
    let mut authored = BTreeMap::new();
    let mut source_owners = BTreeMap::new();
    let module_indices: BTreeMap<_, _> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| (module.identity(), index))
        .collect();

    for module in modules {
        for source in module.sources() {
            source_owners.insert(source.syntax().source(), module.identity());
            collect_use_nodes(source.syntax(), target_selection, &mut authored)?;
        }
    }

    let mut resolved = BTreeMap::new();
    let mut module_edges = vec![BTreeMap::new(); modules.len()];
    let mut input_resolutions: Vec<_> = input.use_resolutions().iter().collect();
    input_resolutions.sort_unstable_by(|left, right| {
        resolution_key(left.declaration())
            .cmp(&resolution_key(right.declaration()))
            .then_with(|| left.target_module().cmp(right.target_module()))
    });
    for resolution in input_resolutions {
        let declaration = resolution.declaration();
        if !target_selection.use_is_active(declaration) {
            continue;
        }
        let key = resolution_key(declaration);
        let (_, scope) = authored
            .get(&key)
            .copied()
            .ok_or(LoweringError::InvalidUseResolution(declaration))?;
        if resolved
            .insert(
                key,
                PreparedUseResolution {
                    input: resolution,
                    scope,
                },
            )
            .is_some()
        {
            return Err(LoweringError::DuplicateUseResolution(declaration));
        }
        let importing_module = source_owners
            .get(&declaration.source())
            .copied()
            .ok_or(LoweringError::InvalidUseResolution(declaration))?;
        let importing_index = *module_indices
            .get(importing_module)
            .ok_or(LoweringError::InvalidUseResolution(declaration))?;
        let target_index = *module_indices
            .get(resolution.target_module())
            .ok_or(LoweringError::UnknownUseTarget(declaration))?;
        module_edges[importing_index]
            .entry(target_index)
            .or_insert(declaration);
    }
    if let Some((_, (declaration, _))) =
        authored.iter().find(|(key, _)| !resolved.contains_key(key))
    {
        return Err(LoweringError::MissingUseResolution(*declaration));
    }

    validate_acyclic_modules(modules, &module_edges)?;
    Ok(resolved)
}

fn collect_source_visibility_nodes(
    tree: &nocter_syntax::SyntaxTree,
    declarations: &mut BTreeMap<SourceVisibilityResolutionKey, NodeId>,
) -> Result<(), LoweringError> {
    for element in tree.children(tree.root_id()) {
        let SyntaxElement::Node(node) = element else {
            continue;
        };
        let kind = tree
            .node(*node)
            .ok_or(LoweringError::InconsistentSyntax(tree.source()))?
            .kind();
        if kind == NodeKind::SourceVisibilityDeclaration {
            declarations.insert(resolution_key(*node), *node);
        }
    }
    Ok(())
}

fn collect_use_nodes(
    tree: &nocter_syntax::SyntaxTree,
    target_selection: &TargetSelection,
    declarations: &mut BTreeMap<UseResolutionKey, (NodeId, UseScope)>,
) -> Result<(), LoweringError> {
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        let kind = tree
            .node(node)
            .ok_or(LoweringError::InconsistentSyntax(tree.source()))?
            .kind();
        if kind == NodeKind::Item && !target_selection.item_is_active(node) {
            continue;
        }
        if matches!(
            kind,
            NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
        ) {
            let scope = if kind == NodeKind::BlockUseDeclaration {
                UseScope::Block
            } else {
                UseScope::Module
            };
            declarations.insert(resolution_key(node), (node, scope));
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    Ok(())
}

fn validate_acyclic_modules(
    modules: &[&ModuleInput<'_>],
    edges: &[BTreeMap<usize, NodeId>],
) -> Result<(), LoweringError> {
    let mut indegree = vec![0usize; modules.len()];
    for targets in edges {
        for target in targets.keys() {
            indegree[*target] += 1;
        }
    }
    let mut ready: BTreeSet<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop_first() {
        visited += 1;
        for target in edges[index].keys() {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != modules.len() {
        let imports = module_cycle_witness(edges, &indegree)
            .expect("unvisited module graph contains a cycle witness");
        return Err(TopologyViolation::module_import_cycle(imports)
            .expect("module cycle contains at least one import")
            .into());
    }
    Ok(())
}

fn module_cycle_witness(
    edges: &[BTreeMap<usize, NodeId>],
    residual_indegree: &[usize],
) -> Option<Vec<NodeId>> {
    let mut predecessors = vec![BTreeSet::new(); edges.len()];
    for (source, targets) in edges.iter().enumerate() {
        for target in targets.keys() {
            if residual_indegree[source] != 0 && residual_indegree[*target] != 0 {
                predecessors[*target].insert(source);
            }
        }
    }
    let mut current = residual_indegree.iter().position(|degree| *degree != 0)?;
    let mut positions = BTreeMap::new();
    let mut backward = Vec::new();
    let cycle_start = loop {
        if let Some(position) = positions.insert(current, backward.len()) {
            break position;
        }
        backward.push(current);
        current = *predecessors[current].first()?;
    };
    let mut modules = backward[cycle_start..].to_vec();
    modules.reverse();
    let canonical = modules
        .iter()
        .enumerate()
        .min_by_key(|(_, module)| **module)
        .map(|(position, _)| position)?;
    modules.rotate_left(canonical);

    let mut imports = Vec::with_capacity(modules.len());
    for index in 0..modules.len() {
        let source = modules[index];
        let target = modules[(index + 1) % modules.len()];
        imports.push(*edges[source].get(&target)?);
    }
    Some(imports)
}

const fn resolution_key(declaration: NodeId) -> (SourceId, usize) {
    (declaration.source(), declaration.index())
}

fn collect_symbols(
    input: &CompileUnitInput<'_>,
    packages: &[&PackageInput],
    modules: &[&ModuleInput<'_>],
    target_selection: &TargetSelection,
) -> Result<SymbolTable, LoweringError> {
    let mut spellings: Vec<Box<str>> = Vec::new();
    for package in packages {
        spellings.push(package.display_name().into());
    }
    for module in modules {
        spellings.extend(module.identity().path().iter().cloned());
        for source in module.sources() {
            collect_tree_symbols(
                input,
                source.syntax(),
                Some(target_selection),
                &mut spellings,
            )?;
        }
    }
    Ok(SymbolTable::from_spellings(spellings))
}

fn collect_tree_symbols(
    input: &CompileUnitInput<'_>,
    tree: &nocter_syntax::SyntaxTree,
    target_selection: Option<&TargetSelection>,
    spellings: &mut Vec<Box<str>>,
) -> Result<(), LoweringError> {
    let source = require_source(input, tree.source())?;
    let mut pending = vec![SyntaxElement::Node(tree.root_id())];
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => {
                let kind = tree
                    .node(node)
                    .ok_or(LoweringError::InconsistentSyntax(tree.source()))?
                    .kind();
                if kind == NodeKind::Item
                    && target_selection.is_some_and(|selection| !selection.item_is_active(node))
                {
                    continue;
                }
                // Function and method bodies belong to the current checking generation, not to
                // the reusable declaration symbol domain. `current_symbols` appends their
                // spellings to a checking branch without renumbering this declaration prefix.
                if kind == NodeKind::Block {
                    continue;
                }
                if kind == NodeKind::StringLiteral {
                    let decoded = nocter_syntax::decode_string_literal(source, tree, node)
                        .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
                    spellings.push(decoded);
                    continue;
                }
                pending.extend(tree.children(node).iter().rev().copied());
            }
            SyntaxElement::Token(token)
                if matches!(
                    token.kind(),
                    TokenKind::Identifier
                        | TokenKind::Keyword(
                            nocter_syntax::Keyword::Void | nocter_syntax::Keyword::Never
                        )
                ) =>
            {
                let spelling = source
                    .text_at(token.range())
                    .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
                spellings.push(spelling.into());
            }
            SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
        }
    }
    Ok(())
}

fn project_module_sources(
    index: &mut SourceIndexBuilder,
    module: ModuleId,
    input: &ModuleInput<'_>,
) -> Result<(), LoweringError> {
    let mut sources: Vec<_> = input.sources().iter().collect();
    sources.sort_unstable_by_key(|source| source.canonical_path());
    for source in sources {
        let role = match source.kind() {
            ModuleSourceKind::Root | ModuleSourceKind::SingleFile => SourceRole::Declaration,
            ModuleSourceKind::Implementation => SourceRole::Implementation,
        };
        index.insert(
            SemanticEntity::Module(module),
            role,
            SourceOrigin::from_node(source.syntax(), source.syntax().root_id())
                .map_err(|_| LoweringError::InconsistentSyntax(source.syntax().source()))?,
        );
    }
    Ok(())
}

fn require_source<'input>(
    input: &'input CompileUnitInput<'_>,
    source: SourceId,
) -> Result<&'input nocter_source::SourceFile, LoweringError> {
    input
        .sources()
        .get(source)
        .ok_or(LoweringError::MissingSource(source))
}

fn is_module_segment(segment: &str) -> bool {
    nocter_compile_input::is_valid_module_segment(segment)
}

#[cfg(test)]
mod tests;
