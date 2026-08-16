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
use nocter_syntax::{Keyword, NodeKind, TokenKind};

use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceKind, PackageIdentity, PackageInput,
    PackageMode,
};

#[derive(Debug)]
pub struct LoweredDeclarations {
    program: DeclarationProgram,
    source_index: SourceIndex,
}

impl LoweredDeclarations {
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
    let packages = canonical_packages(input)?;
    let modules = canonical_modules(input, &packages)?;
    validate_sources(input, &packages, &modules)?;
    let symbols = collect_symbols(input, &packages, &modules)?;
    let mut program = DeclarationProgramBuilder::new(symbols);
    let mut source_index = SourceIndexBuilder::new();
    let mut package_ids = BTreeMap::new();

    for package in &packages {
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

    for module in &modules {
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

    Ok(LoweredDeclarations {
        program: program.finish()?,
        source_index: source_index.finish(),
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
