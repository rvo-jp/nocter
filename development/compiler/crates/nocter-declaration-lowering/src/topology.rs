use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    DeclarationProgram, DeclarationProgramBuilder, ModuleNamespace, ModulePath, ProgramBuildError,
};
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::{ModuleId, SymbolTable};
use nocter_source::SourceId;
use nocter_source_index::{
    DuplicateSourceBinding, SemanticEntity, SourceIndex, SourceOrigin, SourceRole,
};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageIdentity, PackageInput, PackageMode, TopologyViolation, UseResolutionInput,
    UseTargetInput,
};
use nocter_target_selection::{TargetSelection, TargetSelectionError};

pub(crate) type UseResolutionKey = (SourceId, usize);

#[derive(Debug)]
pub struct LoweredDeclarations {
    program: DeclarationProgram,
    frontend_bindings: FrontendBindings,
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
        program: DeclarationProgram,
        frontend_bindings: FrontendBindings,
        source_index: SourceIndex,
    ) -> Self {
        Self {
            program,
            frontend_bindings,
            source_index,
        }
    }

    #[must_use]
    pub const fn program(&self) -> &DeclarationProgram {
        &self.program
    }

    #[must_use]
    pub const fn source_index(&self) -> &SourceIndex {
        &self.source_index
    }

    #[must_use]
    pub fn into_parts(self) -> (DeclarationProgram, SourceIndex) {
        (self.program, self.source_index)
    }

    /// Separates semantic checking input from the independently retained presentation index.
    #[must_use]
    pub fn into_checking_parts(
        self,
        input: &CompileUnitInput<'_>,
    ) -> (DeclarationProgram, FrontendBindings, SourceIndex) {
        let bindings = crate::frontend_projection::add_block_imports(input, self.frontend_bindings);
        (self.program, bindings, self.source_index)
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
    InvalidPackageDeclaration(PackageIdentity),
    InvalidModuleSource(Box<str>),
    InconsistentSyntax(SourceId),
    MissingCollectedSymbol(Box<str>),
    InvalidModuleSegment(Box<str>),
    InvalidModuleLayout(ModuleIdentity),
    InvalidPackageModuleSet(PackageIdentity),
    InvalidSingleFilePackage(PackageIdentity),
    PackageTargetResolution(PackageTargetResolutionError),
    MissingUseResolution(NodeId),
    UnknownTargetGate(NodeId),
    DuplicateUseResolution(NodeId),
    InvalidUseResolution(NodeId),
    UnknownUseTarget(NodeId),
    UnreachableImplementationSource(Box<str>),
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            Self::InvalidPackageDeclaration(package) => write!(
                formatter,
                "package {} has an invalid declaration-source shape",
                package.as_str()
            ),
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
            Self::MissingUseResolution(declaration) => {
                write!(
                    formatter,
                    "use declaration {declaration:?} has no resolved target"
                )
            }
            Self::UnknownTargetGate(literal) => {
                write!(formatter, "unknown compilation target in {literal:?}")
            }
            Self::DuplicateUseResolution(declaration) => write!(
                formatter,
                "use declaration {declaration:?} has more than one resolved target"
            ),
            Self::InvalidUseResolution(declaration) => write!(
                formatter,
                "resolved use {declaration:?} does not identify an authored use declaration"
            ),
            Self::UnknownUseTarget(declaration) => write!(
                formatter,
                "resolved use {declaration:?} names a target outside the compile unit"
            ),
            Self::UnreachableImplementationSource(path) => {
                write!(
                    formatter,
                    "implementation source {path} is not reachable from its module root"
                )
            }
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LoweringError {}

impl From<ProgramBuildError> for LoweringError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DuplicateSourceBinding> for LoweringError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
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
) -> Result<LoweredDeclarations, LoweringError> {
    let prepared = prepare_compile_unit(input)?;
    let mut program = DeclarationProgramBuilder::new(input.target(), prepared.symbols);
    let mut source_index = crate::frontend_projection::FrontendProjectionBuilder::new();
    let mut package_ids = BTreeMap::new();

    for package in &prepared.packages {
        let display_name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| LoweringError::MissingCollectedSymbol(package.display_name().into()))?;
        let id = program.add_package(package.identity().clone(), display_name)?;
        package_ids.insert(package.identity().clone(), id);
        if let Some(declaration) = package.declaration() {
            let tree = declaration.syntax();
            source_index.insert(
                SemanticEntity::Package(id),
                SourceRole::Declaration,
                SourceOrigin::from_node(tree, tree.root_id())
                    .map_err(|_| LoweringError::InconsistentSyntax(tree.source()))?,
            )?;
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

    let (source_index, frontend_bindings) = source_index.finish();
    Ok(LoweredDeclarations::new(
        program.finish()?,
        frontend_bindings,
        source_index,
    ))
}

pub(crate) struct PreparedCompileUnit<'input, 'syntax> {
    pub(crate) symbols: SymbolTable,
    pub(crate) packages: Vec<&'input PackageInput<'syntax>>,
    pub(crate) modules: Vec<&'input ModuleInput<'syntax>>,
    pub(crate) use_resolutions: BTreeMap<UseResolutionKey, &'input UseResolutionInput>,
    pub(crate) package_target_resolutions: Vec<&'input crate::PackageTargetResolutionInput>,
    pub(crate) target_selection: TargetSelection,
}

pub(crate) fn prepare_compile_unit<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
) -> Result<PreparedCompileUnit<'input, 'syntax>, LoweringError> {
    let packages = canonical_packages(input)?;
    let modules = canonical_modules(input, &packages)?;
    validate_sources(input, &packages, &modules)?;
    let target_selection = TargetSelection::prepare(
        input.target(),
        input.sources(),
        modules
            .iter()
            .flat_map(|module| module.sources().iter().map(ModuleSourceInput::syntax)),
    )
    .map_err(|error| match error {
        TargetSelectionError::MissingSource(source) => LoweringError::MissingSource(source),
        TargetSelectionError::InconsistentSyntax(source) => {
            LoweringError::InconsistentSyntax(source)
        }
        TargetSelectionError::UnknownTarget(literal) => LoweringError::UnknownTargetGate(literal),
    })?;
    let package_target_resolutions =
        validate_package_target_resolutions(input, &packages, &modules)?;
    let use_resolutions = validate_use_resolutions(input, &modules, &target_selection)?;
    let symbols = collect_symbols(input, &packages, &modules, &target_selection)?;
    Ok(PreparedCompileUnit {
        symbols,
        packages,
        modules,
        use_resolutions,
        package_target_resolutions,
        target_selection,
    })
}

fn validate_package_target_resolutions<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    packages: &[&'input PackageInput<'syntax>],
    modules: &[&'input ModuleInput<'syntax>],
) -> Result<Vec<&'input crate::PackageTargetResolutionInput>, LoweringError> {
    let package_sources: BTreeMap<_, _> = packages
        .iter()
        .filter_map(|package| {
            package
                .declaration()
                .map(|declaration| (declaration.syntax().source(), package.identity()))
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
        let tree = packages
            .iter()
            .find_map(|candidate| {
                candidate
                    .declaration()
                    .filter(|input| input.syntax().source() == declaration.source())
                    .map(crate::PackageDeclarationInput::syntax)
            })
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

fn canonical_packages<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
) -> Result<Vec<&'input PackageInput<'syntax>>, LoweringError> {
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
    packages: &[&PackageInput<'_>],
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
    packages: &[&PackageInput<'_>],
    modules: &[&ModuleInput<'_>],
) -> Result<(), LoweringError> {
    let mut paths = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let package_modes: BTreeMap<_, _> = packages
        .iter()
        .map(|package| (package.identity(), package.mode()))
        .collect();
    for package in packages {
        match (package.mode(), package.declaration()) {
            (PackageMode::Declared, Some(declaration))
                if declaration.syntax().root().kind() == NodeKind::PackageFile =>
            {
                require_source(input, declaration.syntax().source())?;
                if !source_ids.insert(declaration.syntax().source()) {
                    return Err(LoweringError::DuplicateSource(
                        declaration.syntax().source(),
                    ));
                }
                if !paths.insert(declaration.canonical_path()) {
                    return Err(LoweringError::DuplicateSourcePath(
                        declaration.canonical_path().into(),
                    ));
                }
            }
            (PackageMode::SingleFile, None) => {}
            (PackageMode::Declared, _) | (PackageMode::SingleFile, Some(_)) => {
                return Err(LoweringError::InvalidPackageDeclaration(
                    package.identity().clone(),
                ));
            }
        }
    }
    for module in modules {
        let mut roots = 0;
        let mut single_files = 0;
        for source in module.sources() {
            if source.syntax().root().kind() != NodeKind::ModuleSource {
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
    Ok(())
}

fn validate_use_resolutions<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
    modules: &[&'input ModuleInput<'syntax>],
    target_selection: &TargetSelection,
) -> Result<BTreeMap<UseResolutionKey, &'input UseResolutionInput>, LoweringError> {
    let mut authored = BTreeMap::new();
    let mut source_owners = BTreeMap::new();
    let mut path_owners = BTreeMap::new();
    let module_indices: BTreeMap<_, _> = modules
        .iter()
        .enumerate()
        .map(|(index, module)| (module.identity(), index))
        .collect();

    for module in modules {
        for source in module.sources() {
            source_owners.insert(source.syntax().source(), (module.identity(), source));
            path_owners.insert(source.canonical_path(), (module.identity(), source));
            collect_use_nodes(source.syntax(), target_selection, &mut authored)?;
        }
    }

    let mut resolved = BTreeMap::new();
    let mut source_edges: BTreeMap<SourceId, Vec<SourceId>> = BTreeMap::new();
    let mut module_edges = vec![BTreeMap::new(); modules.len()];
    let mut input_resolutions: Vec<_> = input.use_resolutions().iter().collect();
    input_resolutions.sort_unstable_by(|left, right| {
        use_key(left.declaration())
            .cmp(&use_key(right.declaration()))
            .then_with(|| left.target().cmp(right.target()))
    });
    for resolution in input_resolutions {
        let declaration = resolution.declaration();
        if !target_selection.use_is_active(declaration) {
            continue;
        }
        let key = use_key(declaration);
        if resolved.insert(key, resolution).is_some() {
            return Err(LoweringError::DuplicateUseResolution(declaration));
        }
        if !authored.contains_key(&key) {
            return Err(LoweringError::InvalidUseResolution(declaration));
        }
        let (importing_module, importing_source) = source_owners
            .get(&declaration.source())
            .copied()
            .ok_or(LoweringError::InvalidUseResolution(declaration))?;
        match resolution.target() {
            UseTargetInput::Source(path) => {
                let (target_module, target_source) = path_owners
                    .get(path.as_ref())
                    .copied()
                    .ok_or(LoweringError::UnknownUseTarget(declaration))?;
                if importing_module != target_module
                    || target_source.kind() != ModuleSourceKind::Implementation
                    || !is_source_use(importing_source.syntax(), declaration)
                {
                    return Err(TopologyViolation::invalid_source_import(declaration).into());
                }
                source_edges
                    .entry(importing_source.syntax().source())
                    .or_default()
                    .push(target_source.syntax().source());
            }
            UseTargetInput::Module(target) => {
                let importing_index = *module_indices
                    .get(importing_module)
                    .ok_or(LoweringError::InvalidUseResolution(declaration))?;
                let target_index = *module_indices
                    .get(target)
                    .ok_or(LoweringError::UnknownUseTarget(declaration))?;
                module_edges[importing_index]
                    .entry(target_index)
                    .or_insert(declaration);
            }
        }
    }
    if let Some((_, declaration)) = authored.iter().find(|(key, _)| !resolved.contains_key(key)) {
        return Err(LoweringError::MissingUseResolution(*declaration));
    }

    validate_source_reachability(modules, &source_edges)?;
    validate_acyclic_modules(modules, &module_edges)?;
    Ok(resolved)
}

fn collect_use_nodes(
    tree: &nocter_syntax::SyntaxTree,
    target_selection: &TargetSelection,
    declarations: &mut BTreeMap<UseResolutionKey, NodeId>,
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
            declarations.insert(use_key(node), node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    Ok(())
}

fn is_source_use(tree: &nocter_syntax::SyntaxTree, declaration: NodeId) -> bool {
    if tree.node(declaration).map(nocter_syntax::SyntaxNode::kind) != Some(NodeKind::UseDeclaration)
        || direct_child(tree, declaration, NodeKind::Visibility).is_some()
        || direct_child(tree, declaration, NodeKind::ImportSelection).is_some()
    {
        return false;
    }
    let Some(path) = direct_child(tree, declaration, NodeKind::ModulePath) else {
        return false;
    };
    tree.children(path)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(token.kind()),
            SyntaxElement::Node(_) | SyntaxElement::Missing(_) => None,
        })
        == Some(TokenKind::Punctuation(Punctuation::Dot))
}

fn direct_child(
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
    expected: NodeKind,
) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree
                    .node(*child)
                    .is_some_and(|node| node.kind() == expected) =>
            {
                Some(*child)
            }
            SyntaxElement::Node(_) | SyntaxElement::Token(_) | SyntaxElement::Missing(_) => None,
        })
}

fn validate_source_reachability(
    modules: &[&ModuleInput<'_>],
    edges: &BTreeMap<SourceId, Vec<SourceId>>,
) -> Result<(), LoweringError> {
    for module in modules {
        let root = module
            .sources()
            .iter()
            .find(|source| {
                matches!(
                    source.kind(),
                    ModuleSourceKind::Root | ModuleSourceKind::SingleFile
                )
            })
            .expect("validated module has one root source");
        let mut reached = BTreeSet::from([root.syntax().source()]);
        let mut pending = vec![root.syntax().source()];
        while let Some(source) = pending.pop() {
            if let Some(targets) = edges.get(&source) {
                for target in targets {
                    if reached.insert(*target) {
                        pending.push(*target);
                    }
                }
            }
        }
        if let Some(source) = module
            .sources()
            .iter()
            .filter(|source| {
                source.kind() == ModuleSourceKind::Implementation
                    && !reached.contains(&source.syntax().source())
            })
            .min_by_key(|source| source.canonical_path())
        {
            return Err(LoweringError::UnreachableImplementationSource(
                source.canonical_path().into(),
            ));
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

const fn use_key(declaration: NodeId) -> UseResolutionKey {
    (declaration.source(), declaration.index())
}

fn collect_symbols(
    input: &CompileUnitInput<'_>,
    packages: &[&PackageInput<'_>],
    modules: &[&ModuleInput<'_>],
    target_selection: &TargetSelection,
) -> Result<SymbolTable, LoweringError> {
    let mut spellings: Vec<Box<str>> = Vec::new();
    for package in packages {
        spellings.push(package.display_name().into());
        if let Some(declaration) = package.declaration() {
            collect_tree_symbols(input, declaration.syntax(), None, &mut spellings)?;
        }
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
                if kind == NodeKind::StringLiteral {
                    let decoded = nocter_syntax::decode_string_literal(source, tree, node)
                        .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
                    spellings.push(decoded);
                    continue;
                }
                pending.extend(tree.children(node).iter().rev().copied());
            }
            SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => {
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
    index: &mut crate::frontend_projection::FrontendProjectionBuilder,
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
        index.insert_module_source(
            module,
            source.syntax().source(),
            role,
            SourceOrigin::from_node(source.syntax(), source.syntax().root_id())
                .map_err(|_| LoweringError::InconsistentSyntax(source.syntax().source()))?,
        )?;
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
