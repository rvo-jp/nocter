use std::collections::HashMap;
use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, DeclarationSiteId, ModuleId, PackageId, Symbol, SymbolTable, TypeStore,
};

use crate::{ModulePath, Visibility};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    display_name: Symbol,
}

impl Package {
    #[must_use]
    pub const fn display_name(&self) -> Symbol {
        self.display_name
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    package: PackageId,
    path: ModulePath,
}

impl Module {
    #[must_use]
    pub const fn package(&self) -> PackageId {
        self.package
    }

    #[must_use]
    pub const fn path(&self) -> &ModulePath {
        &self.path
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationSite {
    module: ModuleId,
    visibility: Visibility,
}

impl DeclarationSite {
    #[must_use]
    pub const fn module(self) -> ModuleId {
        self.module
    }

    #[must_use]
    pub const fn visibility(self) -> Visibility {
        self.visibility
    }
}

#[derive(Debug)]
pub struct DeclarationProgram {
    symbols: SymbolTable,
    packages: Arena<PackageId, Package>,
    modules: Arena<ModuleId, Module>,
    declaration_sites: Arena<DeclarationSiteId, DeclarationSite>,
    types: TypeStore,
}

impl DeclarationProgram {
    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    #[must_use]
    pub const fn packages(&self) -> &Arena<PackageId, Package> {
        &self.packages
    }

    #[must_use]
    pub const fn modules(&self) -> &Arena<ModuleId, Module> {
        &self.modules
    }

    #[must_use]
    pub const fn declaration_sites(&self) -> &Arena<DeclarationSiteId, DeclarationSite> {
        &self.declaration_sites
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        &self.types
    }
}

#[derive(Debug)]
pub struct DeclarationProgramBuilder {
    symbols: SymbolTable,
    packages: ArenaBuilder<PackageId, Package>,
    modules: ArenaBuilder<ModuleId, Module>,
    module_ids: HashMap<(PackageId, ModulePath), ModuleId>,
    declaration_sites: ArenaBuilder<DeclarationSiteId, DeclarationSite>,
    types: TypeStore,
}

impl DeclarationProgramBuilder {
    #[must_use]
    pub fn new(symbols: SymbolTable) -> Self {
        Self {
            symbols,
            packages: ArenaBuilder::new(),
            modules: ArenaBuilder::new(),
            module_ids: HashMap::new(),
            declaration_sites: ArenaBuilder::new(),
            types: TypeStore::new(),
        }
    }

    /// Adds a package after its exact package-graph identity has been selected by the caller.
    ///
    /// Package insertion order is required to be the caller's canonical package-graph order.
    /// Display names are presentation metadata and do not define package identity.
    ///
    /// # Errors
    ///
    /// Returns [`ProgramBuildError::UnknownSymbol`] when the display name is absent from the
    /// program's canonical symbol table.
    pub fn add_package(&mut self, display_name: Symbol) -> Result<PackageId, ProgramBuildError> {
        self.require_symbol(display_name)?;
        Ok(self.packages.insert(Package { display_name }))
    }

    /// Adds one normalized module identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown package, an unknown path symbol, or a duplicate exact
    /// package-and-path identity.
    pub fn add_module(
        &mut self,
        package: PackageId,
        path: ModulePath,
    ) -> Result<ModuleId, ProgramBuildError> {
        self.require_package(package)?;
        for segment in path.segments() {
            self.require_symbol(*segment)?;
        }
        let key = (package, path.clone());
        if let Some(existing) = self.module_ids.get(&key) {
            return Err(ProgramBuildError::DuplicateModule(*existing));
        }
        let id = self.modules.insert(Module { package, path });
        self.module_ids.insert(key, id);
        Ok(id)
    }

    /// Adds an authored declaration site after resolving its visibility boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the declaring module or normalized visibility boundary is invalid.
    pub fn add_declaration_site(
        &mut self,
        module: ModuleId,
        visibility: Visibility,
    ) -> Result<DeclarationSiteId, ProgramBuildError> {
        let declaring = self.require_module(module)?;
        match visibility {
            Visibility::Private | Visibility::Public => {}
            Visibility::Package(package) => {
                self.require_package(package)?;
                if package != declaring.package {
                    return Err(ProgramBuildError::VisibilityOutsidePackage);
                }
            }
            Visibility::Descendants(boundary) => {
                let boundary = self.require_module(boundary)?;
                if boundary.package != declaring.package
                    || !boundary.path.is_ancestor_of(&declaring.path)
                {
                    return Err(ProgramBuildError::InvalidVisibilityAncestor);
                }
            }
        }
        Ok(self
            .declaration_sites
            .insert(DeclarationSite { module, visibility }))
    }

    #[must_use]
    pub const fn types_mut(&mut self) -> &mut TypeStore {
        &mut self.types
    }

    #[must_use]
    pub fn finish(self) -> DeclarationProgram {
        DeclarationProgram {
            symbols: self.symbols,
            packages: self.packages.finish(),
            modules: self.modules.finish(),
            declaration_sites: self.declaration_sites.finish(),
            types: self.types,
        }
    }

    fn require_symbol(&self, symbol: Symbol) -> Result<(), ProgramBuildError> {
        self.symbols
            .spelling(symbol)
            .map(|_| ())
            .ok_or(ProgramBuildError::UnknownSymbol)
    }

    fn require_package(&self, package: PackageId) -> Result<&Package, ProgramBuildError> {
        self.packages
            .get(package)
            .ok_or(ProgramBuildError::UnknownPackage)
    }

    fn require_module(&self, module: ModuleId) -> Result<&Module, ProgramBuildError> {
        self.modules
            .get(module)
            .ok_or(ProgramBuildError::UnknownModule)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramBuildError {
    UnknownSymbol,
    UnknownPackage,
    UnknownModule,
    DuplicateModule(ModuleId),
    VisibilityOutsidePackage,
    InvalidVisibilityAncestor,
}

impl fmt::Display for ProgramBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol => formatter.write_str("symbol is not part of the program table"),
            Self::UnknownPackage => formatter.write_str("package is not part of the program"),
            Self::UnknownModule => formatter.write_str("module is not part of the program"),
            Self::DuplicateModule(existing) => {
                write!(formatter, "module identity duplicates {existing:?}")
            }
            Self::VisibilityOutsidePackage => {
                formatter.write_str("package visibility names another package")
            }
            Self::InvalidVisibilityAncestor => {
                formatter.write_str("descendant visibility boundary is not a module ancestor")
            }
        }
    }
}

impl std::error::Error for ProgramBuildError {}

#[cfg(test)]
mod tests {
    use nocter_model::SymbolTable;

    use crate::{DeclarationProgramBuilder, ModulePath, ProgramBuildError, Visibility};

    #[test]
    fn module_identity_is_exact_package_and_normalized_path() {
        let symbols = SymbolTable::from_spellings(["app", "dependency", "parser"]);
        let app_name = symbols.get("app").unwrap();
        let dependency_name = symbols.get("dependency").unwrap();
        let parser = symbols.get("parser").unwrap();
        let mut builder = DeclarationProgramBuilder::new(symbols);
        let app = builder.add_package(app_name).unwrap();
        let dependency = builder.add_package(dependency_name).unwrap();

        let app_parser = builder
            .add_module(app, ModulePath::from_segments([parser]))
            .unwrap();
        let dependency_parser = builder
            .add_module(dependency, ModulePath::from_segments([parser]))
            .unwrap();

        assert_ne!(app_parser, dependency_parser);
        assert_eq!(
            builder
                .add_module(app, ModulePath::from_segments([parser]))
                .unwrap_err(),
            ProgramBuildError::DuplicateModule(app_parser)
        );
    }

    #[test]
    fn visibility_is_resolved_to_semantic_boundaries() {
        let symbols = SymbolTable::from_spellings(["app", "parser", "lexer", "other"]);
        let app_name = symbols.get("app").unwrap();
        let parser_name = symbols.get("parser").unwrap();
        let lexer_name = symbols.get("lexer").unwrap();
        let other_name = symbols.get("other").unwrap();
        let mut builder = DeclarationProgramBuilder::new(symbols);
        let app = builder.add_package(app_name).unwrap();
        let other = builder.add_package(other_name).unwrap();
        let root = builder.add_module(app, ModulePath::root()).unwrap();
        let parser = builder
            .add_module(app, ModulePath::from_segments([parser_name]))
            .unwrap();
        let lexer = builder
            .add_module(app, ModulePath::from_segments([parser_name, lexer_name]))
            .unwrap();

        builder
            .add_declaration_site(lexer, Visibility::Descendants(root))
            .unwrap();
        builder
            .add_declaration_site(lexer, Visibility::Descendants(parser))
            .unwrap();
        builder
            .add_declaration_site(lexer, Visibility::Package(app))
            .unwrap();
        assert_eq!(
            builder
                .add_declaration_site(root, Visibility::Descendants(parser))
                .unwrap_err(),
            ProgramBuildError::InvalidVisibilityAncestor
        );
        assert_eq!(
            builder
                .add_declaration_site(root, Visibility::Package(other))
                .unwrap_err(),
            ProgramBuildError::VisibilityOutsidePackage
        );
    }

    #[test]
    fn program_is_frozen_with_its_type_store() {
        let symbols = SymbolTable::from_spellings(["app"]);
        let app_name = symbols.get("app").unwrap();
        let mut builder = DeclarationProgramBuilder::new(symbols);
        let app = builder.add_package(app_name).unwrap();
        let root = builder.add_module(app, ModulePath::root()).unwrap();
        let site = builder
            .add_declaration_site(root, Visibility::Private)
            .unwrap();
        let program = builder.finish();

        assert_eq!(program.packages().len(), 1);
        assert_eq!(program.modules().len(), 1);
        assert_eq!(
            program.declaration_sites().get(site).unwrap().module(),
            root
        );
    }
}
