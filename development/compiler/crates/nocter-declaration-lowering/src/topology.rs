use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    DeclarationProgram, DeclarationProgramBuilder, ModulePath, ProgramBuildError,
};
use nocter_model::{ModuleId, SymbolTable};
use nocter_source::SourceId;
use nocter_source_index::{
    DuplicateSourceBinding, SemanticEntity, SourceIndex, SourceIndexBuilder, SourceOrigin,
    SourceRole,
};
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, SyntaxElement, TokenKind};

use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceKind, PackageIdentity, PackageInput,
    PackageMode, UseResolutionInput, UseTargetInput,
};

pub(crate) type UseResolutionKey = (SourceId, usize);

#[derive(Debug)]
pub struct LoweredDeclarations {
    program: DeclarationProgram,
    source_index: SourceIndex,
}

impl LoweredDeclarations {
    pub(crate) const fn new(program: DeclarationProgram, source_index: SourceIndex) -> Self {
        Self {
            program,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoweringError {
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
    MissingUseResolution(NodeId),
    DuplicateUseResolution(NodeId),
    InvalidUseResolution(NodeId),
    UnknownUseTarget(NodeId),
    InvalidSourceUse(NodeId),
    UnreachableImplementationSource(Box<str>),
    ModuleImportCycle(ModuleIdentity),
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
}

impl fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Self::InvalidSourceUse(declaration) => write!(
                formatter,
                "resolved source use {declaration:?} violates same-module composition rules"
            ),
            Self::UnreachableImplementationSource(path) => {
                write!(
                    formatter,
                    "implementation source {path} is not reachable from its module root"
                )
            }
            Self::ModuleImportCycle(module) => {
                write!(
                    formatter,
                    "module import graph contains a cycle through {module:?}"
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
    let mut program = DeclarationProgramBuilder::new(prepared.symbols);
    let mut source_index = SourceIndexBuilder::new();
    let mut package_ids = BTreeMap::new();

    for package in &prepared.packages {
        let display_name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| LoweringError::MissingCollectedSymbol(package.display_name().into()))?;
        let id = program.add_package(display_name)?;
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
        project_module_sources(&mut source_index, id, module)?;
    }

    Ok(LoweredDeclarations::new(
        program.finish()?,
        source_index.finish(),
    ))
}

pub(crate) struct PreparedCompileUnit<'input, 'syntax> {
    pub(crate) symbols: SymbolTable,
    pub(crate) packages: Vec<&'input PackageInput<'syntax>>,
    pub(crate) modules: Vec<&'input ModuleInput<'syntax>>,
    pub(crate) use_resolutions: BTreeMap<UseResolutionKey, &'input UseResolutionInput>,
}

pub(crate) fn prepare_compile_unit<'input, 'syntax>(
    input: &'input CompileUnitInput<'syntax>,
) -> Result<PreparedCompileUnit<'input, 'syntax>, LoweringError> {
    let packages = canonical_packages(input)?;
    let modules = canonical_modules(input, &packages)?;
    validate_sources(input, &packages, &modules)?;
    let use_resolutions = validate_use_resolutions(input, &modules)?;
    let symbols = collect_symbols(input, &packages, &modules)?;
    Ok(PreparedCompileUnit {
        symbols,
        packages,
        modules,
        use_resolutions,
    })
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
            collect_use_nodes(source.syntax(), &mut authored)?;
        }
    }

    let mut resolved = BTreeMap::new();
    let mut source_edges: BTreeMap<SourceId, Vec<SourceId>> = BTreeMap::new();
    let mut module_edges = vec![BTreeSet::new(); modules.len()];
    let mut input_resolutions: Vec<_> = input.use_resolutions().iter().collect();
    input_resolutions.sort_unstable_by(|left, right| {
        use_key(left.declaration())
            .cmp(&use_key(right.declaration()))
            .then_with(|| left.target().cmp(right.target()))
    });
    for resolution in input_resolutions {
        let declaration = resolution.declaration();
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
                    return Err(LoweringError::InvalidSourceUse(declaration));
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
                module_edges[importing_index].insert(target_index);
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
    declarations: &mut BTreeMap<UseResolutionKey, NodeId>,
) -> Result<(), LoweringError> {
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        let kind = tree
            .node(node)
            .ok_or(LoweringError::InconsistentSyntax(tree.source()))?
            .kind();
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
    edges: &[BTreeSet<usize>],
) -> Result<(), LoweringError> {
    let mut indegree = vec![0usize; modules.len()];
    for targets in edges {
        for target in targets {
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
        for target in &edges[index] {
            indegree[*target] -= 1;
            if indegree[*target] == 0 {
                ready.insert(*target);
            }
        }
    }
    if visited != modules.len() {
        let index = indegree
            .iter()
            .position(|degree| *degree != 0)
            .expect("unvisited module has a positive indegree");
        return Err(LoweringError::ModuleImportCycle(
            modules[index].identity().clone(),
        ));
    }
    Ok(())
}

const fn use_key(declaration: NodeId) -> UseResolutionKey {
    (declaration.source(), declaration.index())
}

fn collect_symbols(
    input: &CompileUnitInput<'_>,
    packages: &[&PackageInput<'_>],
    modules: &[&ModuleInput<'_>],
) -> Result<SymbolTable, LoweringError> {
    let mut spellings: Vec<Box<str>> = Vec::new();
    for package in packages {
        spellings.push(package.display_name().into());
        if let Some(declaration) = package.declaration() {
            collect_tree_symbols(input, declaration.syntax(), &mut spellings)?;
        }
    }
    for module in modules {
        spellings.extend(module.identity().path().iter().cloned());
        for source in module.sources() {
            collect_tree_symbols(input, source.syntax(), &mut spellings)?;
        }
    }
    Ok(SymbolTable::from_spellings(spellings))
}

fn collect_tree_symbols(
    input: &CompileUnitInput<'_>,
    tree: &nocter_syntax::SyntaxTree,
    spellings: &mut Vec<Box<str>>,
) -> Result<(), LoweringError> {
    let source = require_source(input, tree.source())?;
    for token in tree.lexed().tokens() {
        if token.kind() == TokenKind::Identifier {
            let spelling = source
                .text_at(token.span().range())
                .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
            spellings.push(spelling.into());
        }
    }
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|syntax| syntax.kind() == NodeKind::StringLiteral)
        {
            let decoded = crate::text::decode_string_literal(source, tree, node)
                .ok_or(LoweringError::InconsistentSyntax(tree.source()))?;
            spellings.push(decoded);
            continue;
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
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
    let bytes = segment.as_bytes();
    !bytes.is_empty()
        && segment != "_"
        && !bytes[0].is_ascii_digit()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
        && Keyword::from_spelling(segment).is_none()
}

#[cfg(test)]
mod tests;
