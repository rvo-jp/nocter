use std::collections::HashMap;
use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, CompilationTarget, DeclarationSiteId, ImportId, ModuleId, PackageId,
    PackageIdentity, PackageTargetId, Symbol, SymbolTable, TypeAuthority, TypeStore,
    TypeTransaction,
};
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

use crate::{
    DeclarationAnalysisAdmission, DeclarationArenaBuilder, DeclarationArenas, ExportedEntity,
    ImportDeclaration, IncompleteDefinition, ModuleNamespace, ModulePath, PackageTarget,
    ProgramValidationError, StandardLibrary, Visibility,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Package {
    identity: PackageIdentity,
    display_name: Symbol,
}

impl Package {
    #[must_use]
    pub const fn identity(&self) -> &PackageIdentity {
        &self.identity
    }

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
pub struct DeclarationGraph {
    target: CompilationTarget,
    symbols: SymbolTable,
    packages: Arena<PackageId, Package>,
    package_ids: HashMap<PackageIdentity, PackageId>,
    root_packages: Box<[PackageId]>,
    standard_library: Option<StandardLibrary>,
    modules: Arena<ModuleId, Module>,
    module_ids: HashMap<(PackageId, ModulePath), ModuleId>,
    module_namespaces: Arena<ModuleId, ModuleNamespace>,
    declaration_sites: Arena<DeclarationSiteId, DeclarationSite>,
    imports: Arena<ImportId, ImportDeclaration>,
    package_targets: Arena<PackageTargetId, PackageTarget>,
    declarations: DeclarationArenas,
}

/// Immutable Phase 2 declaration graph and its canonical header-type store.
#[derive(Debug)]
pub struct DeclarationProgram {
    graph: DeclarationGraph,
    types: TypeAuthority,
}

/// A declaration program whose complete integrity and authored-language validation succeeded.
///
/// Only this type can cross into production checking. Read-only declaration queries dereference
/// to the underlying immutable program, while the analysis admission authority remains available
/// only through the consuming phase transition.
#[derive(Debug)]
pub struct AcceptedDeclarationProgram {
    program: DeclarationProgram,
    admission: DeclarationAnalysisAdmission,
}

impl DeclarationGraph {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    #[must_use]
    pub const fn packages(&self) -> &Arena<PackageId, Package> {
        &self.packages
    }

    /// Resolves one exact package-graph identity without scanning display metadata.
    #[must_use]
    pub fn package_by_identity(&self, identity: &PackageIdentity) -> Option<PackageId> {
        self.package_ids.get(identity).copied()
    }

    /// Returns the exact packages selected before dependency traversal.
    #[must_use]
    pub const fn root_packages(&self) -> &[PackageId] {
        &self.root_packages
    }

    /// Returns the exact package selected to provide compiler-owned standard declarations.
    ///
    /// Standalone declaration graphs may omit a standard package. Built-in attachments and
    /// primitive declarations are invalid unless this identity is present.
    #[must_use]
    pub const fn standard_package(&self) -> Option<PackageId> {
        match &self.standard_library {
            Some(standard) => Some(standard.package()),
            None => None,
        }
    }

    #[must_use]
    pub const fn standard_library(&self) -> Option<&StandardLibrary> {
        self.standard_library.as_ref()
    }

    #[must_use]
    pub const fn modules(&self) -> &Arena<ModuleId, Module> {
        &self.modules
    }

    /// Resolves one normalized package-local module path through the graph's canonical index.
    #[must_use]
    pub fn module_by_path(&self, package: PackageId, path: &ModulePath) -> Option<ModuleId> {
        self.module_ids.get(&(package, path.clone())).copied()
    }

    #[must_use]
    pub const fn module_namespaces(&self) -> &Arena<ModuleId, ModuleNamespace> {
        &self.module_namespaces
    }

    /// Resolves one exact module-local authored or compiler-selected fallback name.
    #[must_use]
    pub fn lookup_local(&self, module: ModuleId, name: Symbol) -> Option<ExportedEntity> {
        self.module_namespaces.get(module)?.lookup_local(name)
    }

    /// Resolves an authored export visible from another module.
    ///
    /// Compiler-selected prelude fallback names never participate in this lookup.
    #[must_use]
    pub fn lookup_export(
        &self,
        from: ModuleId,
        module: ModuleId,
        name: Symbol,
    ) -> Option<ExportedEntity> {
        let entry = self.module_namespaces.get(module)?.lookup_authored(name)?;
        self.is_visible_from(entry.visibility(), from, module)
            .then_some(entry.target())
    }

    /// Tests one normalized visibility boundary without reinterpreting source path syntax.
    #[must_use]
    pub fn is_visible_from(
        &self,
        visibility: Visibility,
        from: ModuleId,
        declaring_module: ModuleId,
    ) -> bool {
        if self.modules.get(declaring_module).is_none() {
            return false;
        }
        match visibility {
            Visibility::Private => from == declaring_module,
            Visibility::Public => self.modules.get(from).is_some(),
            Visibility::Package(package) => self
                .modules
                .get(from)
                .is_some_and(|module| module.package() == package),
            Visibility::Descendants(boundary) => {
                let Some(boundary) = self.modules.get(boundary) else {
                    return false;
                };
                let Some(from) = self.modules.get(from) else {
                    return false;
                };
                boundary.package() == from.package() && boundary.path().is_ancestor_of(from.path())
            }
        }
    }

    #[must_use]
    pub const fn declaration_sites(&self) -> &Arena<DeclarationSiteId, DeclarationSite> {
        &self.declaration_sites
    }

    #[must_use]
    pub const fn imports(&self) -> &Arena<ImportId, ImportDeclaration> {
        &self.imports
    }

    #[must_use]
    pub const fn package_targets(&self) -> &Arena<PackageTargetId, PackageTarget> {
        &self.package_targets
    }

    #[must_use]
    pub const fn declarations(&self) -> &DeclarationArenas {
        &self.declarations
    }
}

impl DeclarationProgram {
    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.graph.target()
    }

    #[must_use]
    pub const fn graph(&self) -> &DeclarationGraph {
        &self.graph
    }

    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        self.graph.symbols()
    }

    #[must_use]
    pub const fn packages(&self) -> &Arena<PackageId, Package> {
        self.graph.packages()
    }

    #[must_use]
    pub fn package_by_identity(&self, identity: &PackageIdentity) -> Option<PackageId> {
        self.graph.package_by_identity(identity)
    }

    #[must_use]
    pub const fn root_packages(&self) -> &[PackageId] {
        self.graph.root_packages()
    }

    #[must_use]
    pub const fn standard_package(&self) -> Option<PackageId> {
        self.graph.standard_package()
    }

    #[must_use]
    pub const fn standard_library(&self) -> Option<&StandardLibrary> {
        self.graph.standard_library()
    }

    #[must_use]
    pub const fn modules(&self) -> &Arena<ModuleId, Module> {
        self.graph.modules()
    }

    #[must_use]
    pub fn module_by_path(&self, package: PackageId, path: &ModulePath) -> Option<ModuleId> {
        self.graph.module_by_path(package, path)
    }

    #[must_use]
    pub const fn module_namespaces(&self) -> &Arena<ModuleId, ModuleNamespace> {
        self.graph.module_namespaces()
    }

    #[must_use]
    pub fn lookup_local(&self, module: ModuleId, name: Symbol) -> Option<ExportedEntity> {
        self.graph.lookup_local(module, name)
    }

    #[must_use]
    pub fn lookup_export(
        &self,
        from: ModuleId,
        module: ModuleId,
        name: Symbol,
    ) -> Option<ExportedEntity> {
        self.graph.lookup_export(from, module, name)
    }

    #[must_use]
    pub fn is_visible_from(
        &self,
        visibility: Visibility,
        from: ModuleId,
        declaring_module: ModuleId,
    ) -> bool {
        self.graph
            .is_visible_from(visibility, from, declaring_module)
    }

    #[must_use]
    pub const fn declaration_sites(&self) -> &Arena<DeclarationSiteId, DeclarationSite> {
        self.graph.declaration_sites()
    }

    #[must_use]
    pub const fn imports(&self) -> &Arena<ImportId, ImportDeclaration> {
        self.graph.imports()
    }

    #[must_use]
    pub const fn package_targets(&self) -> &Arena<PackageTargetId, PackageTarget> {
        self.graph.package_targets()
    }

    #[must_use]
    pub const fn declarations(&self) -> &DeclarationArenas {
        self.graph.declarations()
    }

    #[must_use]
    pub const fn types(&self) -> &TypeStore {
        self.types.store()
    }

    fn into_unvalidated_parts(self) -> (DeclarationGraph, TypeAuthority) {
        (self.graph, self.types)
    }
}

impl std::ops::Deref for AcceptedDeclarationProgram {
    type Target = DeclarationProgram;

    fn deref(&self) -> &Self::Target {
        &self.program
    }
}

impl AcceptedDeclarationProgram {
    #[must_use]
    pub const fn program(&self) -> &DeclarationProgram {
        &self.program
    }

    /// Opens the sole Phase 2-to-Phase 3 ownership boundary.
    ///
    /// The returned type authority keeps every declaration `TypeId` as an immutable prefix.
    /// Phase 3 may open branch-local body and specialization transactions before publishing a
    /// read-only checked snapshot; no second store or ID translation is created.
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DeclarationGraph,
        TypeAuthority,
        DeclarationAnalysisAdmission,
    ) {
        let (graph, types) = self.program.into_unvalidated_parts();
        (graph, types, self.admission)
    }
}

#[derive(Debug)]
pub struct DeclarationProgramBuilder {
    target: CompilationTarget,
    symbols: SymbolTable,
    packages: ArenaBuilder<PackageId, Package>,
    package_ids: HashMap<PackageIdentity, PackageId>,
    root_packages: Vec<PackageId>,
    standard_library: Option<StandardLibrary>,
    modules: ArenaBuilder<ModuleId, Module>,
    module_namespaces: ArenaBuilder<ModuleId, Option<ModuleNamespace>>,
    module_ids: HashMap<(PackageId, ModulePath), ModuleId>,
    declaration_sites: ArenaBuilder<DeclarationSiteId, DeclarationSite>,
    imports: ArenaBuilder<ImportId, ImportDeclaration>,
    package_targets: ArenaBuilder<PackageTargetId, PackageTarget>,
    declarations: DeclarationArenaBuilder,
    types: TypeTransaction,
}

impl DeclarationProgramBuilder {
    #[must_use]
    pub fn new(target: CompilationTarget, symbols: SymbolTable) -> Self {
        Self {
            target,
            symbols,
            packages: ArenaBuilder::new(),
            package_ids: HashMap::new(),
            root_packages: Vec::new(),
            standard_library: None,
            modules: ArenaBuilder::new(),
            module_namespaces: ArenaBuilder::new(),
            module_ids: HashMap::new(),
            declaration_sites: ArenaBuilder::new(),
            imports: ArenaBuilder::new(),
            package_targets: ArenaBuilder::new(),
            declarations: DeclarationArenaBuilder::new(),
            types: TypeAuthority::new().transaction(),
        }
    }

    #[must_use]
    pub const fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// Adds a package after its exact package-graph identity has been selected by the caller.
    ///
    /// Package insertion order is required to be the caller's canonical package-graph order.
    /// Display names are presentation metadata and do not define package identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the display name is absent from the program's canonical symbol table
    /// or the resolved package identity has already been reserved.
    pub fn add_package(
        &mut self,
        identity: PackageIdentity,
        display_name: Symbol,
    ) -> Result<PackageId, ProgramBuildError> {
        self.require_symbol(display_name)?;
        if self.package_ids.contains_key(&identity) {
            return Err(ProgramBuildError::DuplicatePackageIdentity(identity));
        }
        let id = self.packages.insert(Package {
            identity: identity.clone(),
            display_name,
        });
        self.package_ids.insert(identity, id);
        Ok(id)
    }

    /// Records packages selected as compile roots before dependency traversal.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown package or a repeated root identity.
    pub fn set_root_packages(
        &mut self,
        packages: impl IntoIterator<Item = PackageId>,
    ) -> Result<(), ProgramBuildError> {
        for package in packages {
            self.require_package(package)?;
            if self.root_packages.contains(&package) {
                return Err(ProgramBuildError::DuplicateRootPackage(package));
            }
            self.root_packages.push(package);
        }
        Ok(())
    }

    /// Records the exact package selected by compilation setup as the standard library.
    ///
    /// This semantic identity is stored in the immutable program so later validation never has to
    /// infer compiler authority from a package display name.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown package or an attempt to select a different standard
    /// package after one has already been recorded.
    pub fn set_standard_package(&mut self, package: PackageId) -> Result<(), ProgramBuildError> {
        self.require_package(package)?;
        match &self.standard_library {
            None => {
                self.standard_library = Some(StandardLibrary::new(package));
                Ok(())
            }
            Some(existing) if existing.package() == package => Ok(()),
            Some(_) => Err(ProgramBuildError::ConflictingStandardPackage),
        }
    }

    /// Grants one built-in surface to an exact module selected by compilation setup.
    ///
    /// # Errors
    ///
    /// Returns an error when no standard package is selected, the module is unknown or outside the
    /// selected package, or a different module already owns this surface.
    pub fn set_structural_attachment_module(
        &mut self,
        attachment: StructuralAttachment,
        module: ModuleId,
    ) -> Result<(), ProgramBuildError> {
        let module_package = self.require_module(module)?.package();
        let standard = self
            .standard_library
            .as_mut()
            .ok_or(ProgramBuildError::StandardPackageNotSelected)?;
        if module_package != standard.package() {
            return Err(ProgramBuildError::StandardModuleOutsidePackage);
        }
        standard
            .set_structural_attachment_module(attachment, module)
            .map_err(|_| ProgramBuildError::ConflictingStructuralAttachmentModule(attachment))
    }

    /// Records the source module owning one named compiler-represented type declaration.
    ///
    /// # Errors
    ///
    /// Returns an error when no standard package is selected, the module is unknown or outside the
    /// selected package, or a different module already owns this builtin type.
    pub fn set_builtin_type_module(
        &mut self,
        builtin: nocter_model::BuiltinType,
        module: ModuleId,
    ) -> Result<(), ProgramBuildError> {
        let module_package = self.require_module(module)?.package();
        let standard = self
            .standard_library
            .as_mut()
            .ok_or(ProgramBuildError::StandardPackageNotSelected)?;
        if module_package != standard.package() {
            return Err(ProgramBuildError::StandardModuleOutsidePackage);
        }
        standard
            .set_builtin_type_module(builtin, module)
            .map_err(|_| ProgramBuildError::ConflictingBuiltinTypeModule(builtin))
    }

    /// Records one exact standard declaration selected by toolchain discovery.
    ///
    /// # Errors
    ///
    /// Returns an error when no standard package is selected or the role was already assigned to
    /// a different declaration.
    pub fn set_standard_declaration(
        &mut self,
        role: StandardDeclarationRole,
        declaration: crate::StandardDeclaration,
    ) -> Result<(), ProgramBuildError> {
        self.standard_library
            .as_mut()
            .ok_or(ProgramBuildError::StandardPackageNotSelected)?
            .set_declaration(role, declaration)
            .map_err(|_| ProgramBuildError::ConflictingStandardDeclaration(role))
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
        let namespace = self.module_namespaces.insert(None);
        debug_assert_eq!(id, namespace);
        self.module_ids.insert(key, id);
        Ok(id)
    }

    /// Defines the canonical authored and prelude-fallback namespace for one module.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown module or a second definition of the same namespace.
    pub fn define_module_namespace(
        &mut self,
        module: ModuleId,
        namespace: ModuleNamespace,
    ) -> Result<(), ProgramBuildError> {
        self.require_module(module)?;
        let slot = self
            .module_namespaces
            .get_mut(module)
            .ok_or(ProgramBuildError::UnknownModule)?;
        if slot.is_some() {
            return Err(ProgramBuildError::ModuleNamespaceAlreadyDefined(module));
        }
        *slot = Some(namespace);
        Ok(())
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
    pub fn types(&self) -> &TypeStore {
        &self.types
    }

    #[must_use]
    pub const fn types_mut(&mut self) -> &mut TypeTransaction {
        &mut self.types
    }

    #[must_use]
    pub const fn declarations_mut(&mut self) -> &mut DeclarationArenaBuilder {
        &mut self.declarations
    }

    #[must_use]
    pub const fn declarations(&self) -> &DeclarationArenaBuilder {
        &self.declarations
    }

    #[must_use]
    pub fn module_package(&self, module: ModuleId) -> Option<PackageId> {
        self.modules.get(module).map(Module::package)
    }

    pub fn add_import(&mut self, import: ImportDeclaration) -> ImportId {
        self.imports.insert(import)
    }

    /// Adds a package target after checking that its selected module belongs to that package.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown identity, unknown name, or cross-package target module.
    pub fn add_package_target(
        &mut self,
        target: PackageTarget,
    ) -> Result<PackageTargetId, ProgramBuildError> {
        self.require_package(target.package())?;
        self.require_symbol(target.name())?;
        let module = self.require_module(target.module())?;
        if module.package != target.package() {
            return Err(ProgramBuildError::TargetOutsidePackage);
        }
        Ok(self.package_targets.insert(target))
    }

    /// Freezes and validates the complete declaration program.
    ///
    /// # Errors
    ///
    /// Returns an error when an identity reservation was not completed.
    pub fn finish(self) -> Result<AcceptedDeclarationProgram, ProgramBuildError> {
        self.finish_recovering()
            .map_err(ProgramBuildFailure::into_error)
    }

    /// Freezes and validates the complete declaration program while retaining a structurally
    /// valid graph when an authored language rule rejects it.
    ///
    /// This is the declaration authority for editor recovery. Integrity failures never expose a
    /// program, and callers must continue to report the returned error rather than treating the
    /// recovery graph as accepted source.
    ///
    /// # Errors
    ///
    /// Returns the exact production build error and, only when the complete declaration report is
    /// nonempty, the structurally valid declaration program plus its frozen analysis facts.
    pub fn finish_recovering(self) -> Result<AcceptedDeclarationProgram, ProgramBuildFailure> {
        let module_namespaces = self
            .module_namespaces
            .try_finish_with(|module, namespace| {
                namespace.ok_or(ProgramBuildError::MissingModuleNamespace(module))
            })
            .map_err(ProgramBuildFailure::Error)?;
        let program = DeclarationProgram {
            graph: DeclarationGraph {
                target: self.target,
                symbols: self.symbols,
                packages: self.packages.finish(),
                package_ids: self.package_ids,
                root_packages: self.root_packages.into_boxed_slice(),
                standard_library: self.standard_library,
                modules: self.modules.finish(),
                module_ids: self.module_ids,
                module_namespaces,
                declaration_sites: self.declaration_sites.finish(),
                imports: self.imports.finish(),
                package_targets: self.package_targets.finish(),
                declarations: self
                    .declarations
                    .finish()
                    .map_err(ProgramBuildError::from)
                    .map_err(ProgramBuildFailure::Error)?,
            },
            types: self.types.freeze(),
        };
        crate::validate::validate_integrity(&program)
            .map_err(ProgramValidationError::from)
            .map_err(ProgramBuildError::from)
            .map_err(ProgramBuildFailure::Error)?;
        let validation = crate::validate::validate_language_rules(&program)
            .map_err(ProgramValidationError::from)
            .map_err(ProgramBuildError::from)
            .map_err(ProgramBuildFailure::Error)?;
        let (report, admission, body_analysis) = validation.into_parts();
        if !report.is_empty() {
            return Err(ProgramBuildFailure::Rejected(Box::new(
                RejectedDeclarationProgram::new(program, report, admission, body_analysis),
            )));
        }
        Ok(AcceptedDeclarationProgram { program, admission })
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

/// A structurally valid declaration graph rejected by an authored language rule.
///
/// This value deliberately exposes only destructive projection into declaration facts. It cannot
/// enter checking or any other production transition that requires an accepted
/// [`DeclarationProgram`].
#[derive(Debug)]
pub struct RejectedDeclarationProgram {
    program: DeclarationProgram,
    report: crate::validate::DeclarationValidationReport,
    admission: DeclarationAnalysisAdmission,
    body_analysis: crate::validate::BodyAnalysisCapability,
}

impl RejectedDeclarationProgram {
    const fn new(
        program: DeclarationProgram,
        report: crate::validate::DeclarationValidationReport,
        admission: DeclarationAnalysisAdmission,
        body_analysis: crate::validate::BodyAnalysisCapability,
    ) -> Self {
        Self {
            program,
            report,
            admission,
            body_analysis,
        }
    }

    /// Separates the authored validation report from the deepest editor analysis input that the
    /// rejected graph may safely enter.
    ///
    /// The returned variant is the capability boundary. A declaration-only rejection never
    /// constructs a [`BodyAnalysisDeclarationProgram`], so checking cannot depend on callers to
    /// inspect a boolean before opening its editor-only entry point.
    #[must_use]
    pub fn into_analysis(
        self,
    ) -> (
        crate::validate::DeclarationValidationReport,
        RejectedDeclarationAnalysis,
    ) {
        let (graph, types) = self.program.into_unvalidated_parts();
        let analysis = match self.body_analysis {
            crate::validate::BodyAnalysisCapability::DeclarationsOnly => {
                RejectedDeclarationAnalysis::Declarations(DeclarationAnalysisProgram {
                    graph,
                    types,
                })
            }
            crate::validate::BodyAnalysisCapability::AdmittedBodies => {
                RejectedDeclarationAnalysis::Bodies(BodyAnalysisDeclarationProgram {
                    graph,
                    types,
                    admission: self.admission,
                })
            }
        };
        (self.report, analysis)
    }
}

/// The exact editor analysis capability retained after declaration rejection.
#[derive(Debug)]
pub enum RejectedDeclarationAnalysis {
    Declarations(DeclarationAnalysisProgram),
    Bodies(BodyAnalysisDeclarationProgram),
}

/// Structurally valid declaration facts that cannot enter body analysis.
#[derive(Debug)]
pub struct DeclarationAnalysisProgram {
    graph: DeclarationGraph,
    types: TypeAuthority,
}

impl DeclarationAnalysisProgram {
    #[must_use]
    pub fn into_parts(self) -> (DeclarationGraph, TypeAuthority) {
        (self.graph, self.types)
    }
}

/// Structurally valid declaration facts admitted to editor body analysis but never to production
/// compilation.
#[derive(Debug)]
pub struct BodyAnalysisDeclarationProgram {
    graph: DeclarationGraph,
    types: TypeAuthority,
    admission: DeclarationAnalysisAdmission,
}

impl BodyAnalysisDeclarationProgram {
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        DeclarationGraph,
        TypeAuthority,
        DeclarationAnalysisAdmission,
    ) {
        (self.graph, self.types, self.admission)
    }
}

/// A failed declaration-program freeze.
///
/// Authored rejection owns its report and rejected graph in one variant. Structural build and
/// integrity failures cannot accidentally carry a rejected analysis program.
#[derive(Debug)]
pub enum ProgramBuildFailure {
    Error(ProgramBuildError),
    Rejected(Box<RejectedDeclarationProgram>),
}

impl ProgramBuildFailure {
    #[must_use]
    pub fn into_error(self) -> ProgramBuildError {
        match self {
            Self::Error(error) => error,
            Self::Rejected(rejected) => ProgramBuildError::InvalidProgram(
                ProgramValidationError::Declaration(rejected.report),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgramBuildError {
    UnknownSymbol,
    UnknownPackage,
    DuplicatePackageIdentity(PackageIdentity),
    UnknownModule,
    DuplicateModule(ModuleId),
    DuplicateRootPackage(PackageId),
    MissingModuleNamespace(ModuleId),
    ModuleNamespaceAlreadyDefined(ModuleId),
    DuplicateModuleNamespaceName(Symbol),
    ConflictingStandardPackage,
    StandardPackageNotSelected,
    StandardModuleOutsidePackage,
    ConflictingStructuralAttachmentModule(StructuralAttachment),
    ConflictingBuiltinTypeModule(nocter_model::BuiltinType),
    ConflictingStandardDeclaration(StandardDeclarationRole),
    VisibilityOutsidePackage,
    InvalidVisibilityAncestor,
    TargetOutsidePackage,
    IncompleteDefinition(IncompleteDefinition),
    InvalidProgram(ProgramValidationError),
}

impl fmt::Display for ProgramBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol => formatter.write_str("symbol is not part of the program table"),
            Self::UnknownPackage => formatter.write_str("package is not part of the program"),
            Self::DuplicatePackageIdentity(identity) => {
                write!(
                    formatter,
                    "package identity {identity:?} is already reserved"
                )
            }
            Self::UnknownModule => formatter.write_str("module is not part of the program"),
            Self::DuplicateModule(existing) => {
                write!(formatter, "module identity duplicates {existing:?}")
            }
            Self::DuplicateRootPackage(package) => {
                write!(formatter, "compile root repeats package {package:?}")
            }
            Self::MissingModuleNamespace(module) => {
                write!(formatter, "module {module:?} has no canonical namespace")
            }
            Self::ModuleNamespaceAlreadyDefined(module) => {
                write!(formatter, "module {module:?} namespace was already defined")
            }
            Self::DuplicateModuleNamespaceName(name) => {
                write!(formatter, "module namespace repeats symbol {name:?}")
            }
            Self::ConflictingStandardPackage => {
                formatter.write_str("a different standard package is already selected")
            }
            Self::StandardPackageNotSelected => {
                formatter.write_str("the standard package has not been selected")
            }
            Self::StandardModuleOutsidePackage => {
                formatter.write_str("standard built-in module belongs to another package")
            }
            Self::ConflictingStructuralAttachmentModule(attachment) => {
                write!(
                    formatter,
                    "a different module already owns the {attachment:?} built-in surface"
                )
            }
            Self::ConflictingBuiltinTypeModule(builtin) => {
                write!(formatter, "{builtin:?} has conflicting source modules")
            }
            Self::ConflictingStandardDeclaration(role) => {
                write!(
                    formatter,
                    "standard role {role:?} has conflicting declarations"
                )
            }
            Self::VisibilityOutsidePackage => {
                formatter.write_str("package visibility names another package")
            }
            Self::InvalidVisibilityAncestor => {
                formatter.write_str("descendant visibility boundary is not a module ancestor")
            }
            Self::TargetOutsidePackage => {
                formatter.write_str("package target module belongs to another package")
            }
            Self::IncompleteDefinition(error) => error.fmt(formatter),
            Self::InvalidProgram(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProgramBuildError {}

impl From<IncompleteDefinition> for ProgramBuildError {
    fn from(error: IncompleteDefinition) -> Self {
        Self::IncompleteDefinition(error)
    }
}

impl From<ProgramValidationError> for ProgramBuildError {
    fn from(error: ProgramValidationError) -> Self {
        Self::InvalidProgram(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{BuiltinType, PackageIdentity, SymbolTable, TypeKind};

    use crate::{
        DeclarationProgramBuilder, ModuleNamespace, ModulePath, ProgramBuildError, Visibility,
    };

    #[test]
    fn module_identity_is_exact_package_and_normalized_path() {
        let symbols = SymbolTable::from_spellings(["app", "dependency", "parser"]);
        let app_name = symbols.get("app").unwrap();
        let dependency_name = symbols.get("dependency").unwrap();
        let parser = symbols.get("parser").unwrap();
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let app = builder
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        assert_eq!(
            builder
                .add_package(PackageIdentity::new("workspace:app"), dependency_name)
                .unwrap_err(),
            ProgramBuildError::DuplicatePackageIdentity(PackageIdentity::new("workspace:app"))
        );
        let dependency = builder
            .add_package(PackageIdentity::new("resolved:dependency"), dependency_name)
            .unwrap();

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
    fn compile_roots_are_exact_validated_package_identities() {
        let symbols = SymbolTable::from_spellings(["app", "dependency"]);
        let app_name = symbols.get("app").unwrap();
        let dependency_name = symbols.get("dependency").unwrap();
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let app = builder
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let dependency = builder
            .add_package(PackageIdentity::new("resolved:dependency"), dependency_name)
            .unwrap();

        builder.set_root_packages([app]).unwrap();
        assert_eq!(
            builder.set_root_packages([app]).unwrap_err(),
            ProgramBuildError::DuplicateRootPackage(app)
        );

        let app_root = builder.add_module(app, ModulePath::root()).unwrap();
        let dependency_root = builder.add_module(dependency, ModulePath::root()).unwrap();
        builder
            .define_module_namespace(app_root, ModuleNamespace::default())
            .unwrap();
        builder
            .define_module_namespace(dependency_root, ModuleNamespace::default())
            .unwrap();
        let program = builder.finish().unwrap();

        assert_eq!(program.root_packages(), &[app]);
        assert_eq!(
            program.packages().get(app).unwrap().identity(),
            &PackageIdentity::new("workspace:app")
        );
        assert_eq!(
            program.package_by_identity(&PackageIdentity::new("workspace:app")),
            Some(app)
        );
        assert_eq!(
            program.module_by_path(app, &ModulePath::root()),
            Some(app_root)
        );
        assert_eq!(
            program.module_by_path(dependency, &ModulePath::root()),
            Some(dependency_root)
        );
    }

    #[test]
    fn visibility_is_resolved_to_semantic_boundaries() {
        let symbols = SymbolTable::from_spellings(["app", "parser", "lexer", "other"]);
        let app_name = symbols.get("app").unwrap();
        let parser_name = symbols.get("parser").unwrap();
        let lexer_name = symbols.get("lexer").unwrap();
        let other_name = symbols.get("other").unwrap();
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let app = builder
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let other = builder
            .add_package(PackageIdentity::new("workspace:other"), other_name)
            .unwrap();
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
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let app = builder
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let root = builder.add_module(app, ModulePath::root()).unwrap();
        builder
            .define_module_namespace(root, ModuleNamespace::default())
            .unwrap();
        let site = builder
            .add_declaration_site(root, Visibility::Private)
            .unwrap();
        let program = builder.finish().unwrap();

        assert_eq!(program.packages().len(), 1);
        assert_eq!(program.modules().len(), 1);
        assert_eq!(
            program.declaration_sites().get(site).unwrap().module(),
            root
        );
    }

    #[test]
    fn phase_three_extends_the_single_type_store_without_translating_ids() {
        let symbols = SymbolTable::from_spellings(["app"]);
        let app_name = symbols.get("app").unwrap();
        let mut builder =
            DeclarationProgramBuilder::new(nocter_model::CompilationTarget::Arm64Darwin, symbols);
        let i32_type = builder.types().builtin(BuiltinType::I32);
        let app = builder
            .add_package(PackageIdentity::new("workspace:app"), app_name)
            .unwrap();
        let root = builder.add_module(app, ModulePath::root()).unwrap();
        builder
            .define_module_namespace(root, ModuleNamespace::default())
            .unwrap();
        let program = builder.finish().unwrap();
        let prefix_len = program.types().type_count();

        let (graph, types, _admission) = program.into_parts();
        let mut transaction = types.transaction();
        let optional = transaction.intern(TypeKind::Optional(i32_type)).unwrap();
        let types = transaction.commit(&types).unwrap();

        assert_eq!(graph.modules().len(), 1);
        assert_eq!(
            types.store().get(i32_type),
            Some(&TypeKind::Builtin(BuiltinType::I32))
        );
        assert_eq!(
            types.store().get(optional),
            Some(&TypeKind::Optional(i32_type))
        );
        assert_eq!(types.store().type_count(), prefix_len + 1);
    }
}
