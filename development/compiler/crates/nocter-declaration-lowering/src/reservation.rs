use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::{DeclarationProgramBuilder, ModulePath, ProgramBuildError};
use nocter_frontend_bindings::DuplicateBlockImport;
use nocter_model::{
    AssociatedTypeId, BuiltinType, CallableId, ConstantId, ConstructionId, DropId, InstanceId,
    InterfaceId, InterfaceImplementationId, ModuleId, NominalTypeId, OpaqueTypeId, PackageId,
    TestId, TypeAliasId, VariantId,
};
use nocter_runtime_contract::PrimitiveBinding;
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::NodeId;
use nocter_syntax::SyntaxOrigin;

use crate::package_targets::{reserve_package_targets, reserve_single_file_targets};
use crate::surface::SurfaceParts;
use crate::{
    DeclarationContractError, DeclarationContracts, DeclarationSurface, ModuleIdentity,
    ModuleSourceKind, PackageInput, ReservationError::InconsistentSurface, SurfaceBlockImport,
    SurfaceDeclaration, SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceImport, SurfaceSource,
    SurfaceVisibility,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReservedEntity {
    BuiltinType(BuiltinType),
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Constant(ConstantId),
    Callable(CallableId),
    Construction(ConstructionId),
    Instance(InstanceId),
    InterfaceImplementation(InterfaceImplementationId),
    Drop(DropId),
    Test(TestId),
    Variant(VariantId),
    OpaqueType(OpaqueTypeId),
}

/// The authoritative bidirectional identity mapping produced by reservation.
///
/// Multiple surface declarations may describe one semantic entity when a public contract and its
/// implementation are split across source files. `representatives` always points back to the
/// contract representative selected by [`DeclarationContracts`]; consumers must not recover that
/// relationship by searching `by_surface`.
#[derive(Debug)]
pub(crate) struct ReservedEntityIndex {
    by_surface: Box<[Option<ReservedEntity>]>,
    representatives: BTreeMap<ReservedEntity, SurfaceDeclarationId>,
}

impl ReservedEntityIndex {
    fn new(
        by_surface: Vec<Option<ReservedEntity>>,
        contracts: &DeclarationContracts,
    ) -> Result<Self, ReservationError> {
        let mut representatives = BTreeMap::new();
        for (index, entity) in by_surface.iter().copied().enumerate() {
            let Some(entity) = entity else { continue };
            let declaration = SurfaceDeclarationId::from_index(index);
            if contracts.representative(declaration) != declaration {
                continue;
            }
            if representatives.insert(entity, declaration).is_some() {
                return Err(InconsistentSurface(declaration));
            }
        }
        Ok(Self {
            by_surface: by_surface.into_boxed_slice(),
            representatives,
        })
    }

    pub(crate) fn entity(&self, declaration: SurfaceDeclarationId) -> Option<ReservedEntity> {
        self.by_surface.get(declaration.index()).copied().flatten()
    }

    pub(crate) fn representative(&self, entity: ReservedEntity) -> Option<SurfaceDeclarationId> {
        self.representatives.get(&entity).copied()
    }

    pub(crate) const fn by_surface(&self) -> &[Option<ReservedEntity>] {
        &self.by_surface
    }

    pub(crate) fn representatives(&self) -> &BTreeMap<ReservedEntity, SurfaceDeclarationId> {
        &self.representatives
    }
}

impl ReservedEntity {
    #[must_use]
    pub const fn semantic_entity(self) -> SemanticEntity {
        match self {
            Self::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
            Self::NominalType(id) => SemanticEntity::NominalType(id),
            Self::TypeAlias(id) => SemanticEntity::TypeAlias(id),
            Self::Interface(id) => SemanticEntity::Interface(id),
            Self::AssociatedType(id) => SemanticEntity::AssociatedType(id),
            Self::Constant(id) => SemanticEntity::Constant(id),
            Self::Callable(id) => SemanticEntity::Callable(id),
            Self::Construction(id) => SemanticEntity::Construction(id),
            Self::Instance(id) => SemanticEntity::Instance(id),
            Self::InterfaceImplementation(id) => SemanticEntity::InterfaceImplementation(id),
            Self::Drop(id) => SemanticEntity::Drop(id),
            Self::Test(id) => SemanticEntity::Test(id),
            Self::Variant(id) => SemanticEntity::Variant(id),
            Self::OpaqueType(id) => SemanticEntity::OpaqueType(id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReservationError {
    Contract(DeclarationContractError),
    Program(ProgramBuildError),
    DuplicateBlockImport(DuplicateBlockImport),
    Projection(crate::projection_recipe::ProjectionRecipeError),
    MissingSymbol(Box<str>),
    UnknownPackage(ModuleIdentity),
    UnknownRootPackage(crate::PackageIdentity),
    UnknownModule(ModuleIdentity),
    InvalidOwner(SurfaceDeclarationId),
    InconsistentSurface(SurfaceDeclarationId),
    InconsistentSource(SourceId),
    InvalidPackageTarget(NodeId),
    DuplicatePackageTarget(NodeId),
    Toolchain(crate::ToolchainError),
}

impl fmt::Display for ReservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(formatter),
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateBlockImport(error) => error.fmt(formatter),
            Self::Projection(error) => error.fmt(formatter),
            Self::MissingSymbol(spelling) => {
                write!(formatter, "canonical symbol table is missing {spelling}")
            }
            Self::UnknownPackage(module) => {
                write!(formatter, "module {module:?} belongs to an unknown package")
            }
            Self::UnknownRootPackage(package) => {
                write!(formatter, "compile root names unknown package {package:?}")
            }
            Self::UnknownModule(module) => {
                write!(formatter, "source belongs to unknown module {module:?}")
            }
            Self::InvalidOwner(declaration) => {
                write!(
                    formatter,
                    "surface declaration {declaration:?} has an invalid owner"
                )
            }
            Self::InconsistentSurface(declaration) => write!(
                formatter,
                "surface declaration {declaration:?} has inconsistent source topology"
            ),
            Self::InconsistentSource(source) => {
                write!(formatter, "{source} has an inconsistent syntax origin")
            }
            Self::InvalidPackageTarget(declaration) => {
                write!(
                    formatter,
                    "package target {declaration:?} is inconsistent with discovery"
                )
            }
            Self::DuplicatePackageTarget(declaration) => {
                write!(
                    formatter,
                    "package target {declaration:?} repeats a selected target name"
                )
            }
            Self::Toolchain(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReservationError {}

impl From<DeclarationContractError> for ReservationError {
    fn from(error: DeclarationContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<ProgramBuildError> for ReservationError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DuplicateBlockImport> for ReservationError {
    fn from(error: DuplicateBlockImport) -> Self {
        Self::DuplicateBlockImport(error)
    }
}

impl From<crate::projection_recipe::ProjectionRecipeError> for ReservationError {
    fn from(error: crate::projection_recipe::ProjectionRecipeError) -> Self {
        Self::Projection(error)
    }
}

impl From<crate::ToolchainError> for ReservationError {
    fn from(error: crate::ToolchainError) -> Self {
        Self::Toolchain(error)
    }
}

/// A complete semantic-identity reservation awaiting header definition.
///
/// The syntax-owned fields remain private to the lowering crate and are consumed by the header
/// resolver. External users can inspect only the stable topology and typed reservation mapping.
#[derive(Debug)]
pub struct ReservedDeclarations<'syntax> {
    pub(crate) program: DeclarationProgramBuilder,
    pub(crate) source_index: crate::frontend_projection::FrontendProjectionBuilder,
    pub(crate) source_map: &'syntax SourceMap,
    pub(crate) packages: Box<[PackageInput]>,
    pub(crate) package_ids: Box<[PackageId]>,
    pub(crate) modules: Box<[ModuleIdentity]>,
    pub(crate) module_ids: Box<[ModuleId]>,
    pub(crate) sources: Box<[SurfaceSource<'syntax>]>,
    pub(crate) source_modules: Box<[ModuleId]>,
    pub(crate) source_visibilities: Box<[SurfaceVisibility]>,
    pub(crate) imports: Box<[SurfaceImport]>,
    pub(crate) declarations: Box<[SurfaceDeclaration]>,
    pub(crate) contracts: DeclarationContracts,
    pub(crate) entity_index: ReservedEntityIndex,
    pub(crate) toolchain: crate::toolchain::ResolvedToolchainInput,
    pub(crate) primitive_bindings: Box<[PrimitiveBinding]>,
}

impl ReservedDeclarations<'_> {
    #[must_use]
    pub fn symbols(&self) -> &nocter_model::SymbolTable {
        self.program.symbols()
    }

    #[must_use]
    pub const fn source_map(&self) -> &SourceMap {
        self.source_map
    }

    #[must_use]
    pub const fn packages(&self) -> &[PackageInput] {
        &self.packages
    }

    #[must_use]
    pub const fn modules(&self) -> &[ModuleIdentity] {
        &self.modules
    }

    #[must_use]
    pub const fn package_ids(&self) -> &[PackageId] {
        &self.package_ids
    }

    #[must_use]
    pub const fn module_ids(&self) -> &[ModuleId] {
        &self.module_ids
    }

    #[must_use]
    pub const fn sources(&self) -> &[SurfaceSource<'_>] {
        &self.sources
    }

    #[must_use]
    pub const fn imports(&self) -> &[SurfaceImport] {
        &self.imports
    }

    #[must_use]
    pub const fn source_visibilities(&self) -> &[SurfaceVisibility] {
        &self.source_visibilities
    }

    #[must_use]
    pub const fn contracts(&self) -> &DeclarationContracts {
        &self.contracts
    }

    #[must_use]
    pub fn source_binding_count(&self) -> usize {
        self.source_index.len()
    }

    #[must_use]
    pub fn entity(&self, declaration: SurfaceDeclarationId) -> Option<ReservedEntity> {
        self.entity_index.entity(declaration)
    }

    #[must_use]
    pub fn declaration_for_entity(&self, entity: ReservedEntity) -> Option<SurfaceDeclarationId> {
        self.entity_index.representative(entity)
    }

    #[must_use]
    pub fn module_for_source(&self, source: crate::SurfaceSourceId) -> Option<ModuleId> {
        self.source_modules.get(source.index()).copied()
    }

    #[must_use]
    pub fn declarations(&self) -> &[SurfaceDeclaration] {
        &self.declarations
    }

    #[must_use]
    pub const fn entities(&self) -> &[Option<ReservedEntity>] {
        self.entity_index.by_surface()
    }
}

/// Reserves every recursively referenceable declaration identity in canonical surface order.
///
/// Fields, generic parameters, ordinary parameters, requirements, and bodies are added during
/// header definition after their already-reserved owner is known. Associated types are reserved
/// here because callable headers can refer to them recursively.
///
/// # Errors
///
/// Returns [`ReservationError`] for invalid callable contracts, inconsistent surface ownership,
/// missing topology symbols, invalid program topology, or duplicate source projections.
#[cfg(test)]
pub(crate) fn reserve_declaration_identities<'syntax>(
    surface: DeclarationSurface<'syntax>,
    toolchain: &crate::ToolchainInput,
) -> Result<ReservedDeclarations<'syntax>, ReservationError> {
    let resolved = crate::toolchain::resolve_toolchain_surface(&surface, toolchain)?;
    let contracts = crate::analyze_declaration_contracts(&surface)?;
    reserve_with_contracts(surface, contracts, resolved)
}

pub(crate) fn reserve_with_contracts(
    surface: DeclarationSurface<'_>,
    contracts: DeclarationContracts,
    toolchain: crate::toolchain::ResolvedToolchainInput,
) -> Result<ReservedDeclarations<'_>, ReservationError> {
    let SurfaceParts {
        target,
        source_map,
        symbols,
        packages,
        root_packages,
        modules,
        sources,
        source_visibilities,
        imports,
        block_imports,
        package_target_resolutions,
        declarations,
    } = surface.into_parts();
    let mut program = DeclarationProgramBuilder::new(target, symbols);
    let mut source_index =
        crate::frontend_projection::FrontendProjectionBuilder::new(source_map, &sources)?;
    let package_ids = reserve_packages(&packages, &sources, &mut program, &mut source_index)?;
    let semantic_roots = root_packages
        .iter()
        .map(|identity| {
            package_ids
                .get(identity)
                .copied()
                .ok_or_else(|| ReservationError::UnknownRootPackage(identity.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    program.set_root_packages(semantic_roots)?;
    let module_ids = reserve_modules(&modules, &package_ids, &mut program)?;
    project_block_imports(&block_imports, &module_ids, &mut source_index)?;
    reserve_single_file_targets(
        &packages,
        &sources,
        &package_ids,
        &module_ids,
        &mut program,
        &mut source_index,
    )?;
    reserve_package_targets(
        &packages,
        &sources,
        &package_target_resolutions,
        &package_ids,
        &module_ids,
        &mut program,
        &mut source_index,
    )?;
    let source_modules = project_sources(&sources, &module_ids, &mut source_index)?;
    let entity_index = reserve_surface_entities(
        &declarations,
        &contracts,
        toolchain.builtin_types(),
        &mut program,
    )?;
    let primitive_bindings =
        resolve_primitive_bindings(&declarations, &entity_index, toolchain.primitive_roles())?;
    project_declaration_documentation(
        &sources,
        &declarations,
        &contracts,
        entity_index.by_surface(),
        &mut source_index,
    )?;
    let semantic_packages = packages
        .iter()
        .map(|package| {
            package_ids
                .get(package.identity())
                .copied()
                .ok_or_else(|| ReservationError::MissingSymbol(package.display_name().into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let semantic_modules = modules
        .iter()
        .map(|module| {
            module_ids
                .get(module)
                .copied()
                .ok_or_else(|| ReservationError::UnknownModule(module.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ReservedDeclarations {
        program,
        source_index,
        source_map,
        packages,
        package_ids: semantic_packages.into_boxed_slice(),
        modules,
        module_ids: semantic_modules.into_boxed_slice(),
        sources,
        source_modules: source_modules.into_boxed_slice(),
        source_visibilities,
        imports,
        declarations,
        contracts,
        entity_index,
        toolchain,
        primitive_bindings: primitive_bindings.into_boxed_slice(),
    })
}

fn resolve_primitive_bindings(
    declarations: &[SurfaceDeclaration],
    entities: &ReservedEntityIndex,
    roles: &[crate::toolchain::ResolvedPrimitiveRole],
) -> Result<Vec<PrimitiveBinding>, ReservationError> {
    roles
        .iter()
        .copied()
        .map(|role| {
            let index = role.declaration().index();
            let declaration = declarations
                .get(index)
                .ok_or(InconsistentSurface(role.declaration()))?;
            let Some(ReservedEntity::Callable(callable)) = entities.entity(role.declaration())
            else {
                return Err(InconsistentSurface(role.declaration()));
            };
            if declaration.kind() != SurfaceDeclarationKind::PrimitiveFunction {
                return Err(InconsistentSurface(role.declaration()));
            }
            Ok(PrimitiveBinding::new(role.role(), callable))
        })
        .collect()
}

fn reserve_packages(
    packages: &[PackageInput],
    sources: &[SurfaceSource<'_>],
    program: &mut DeclarationProgramBuilder,
    source_index: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<BTreeMap<crate::PackageIdentity, PackageId>, ReservationError> {
    let mut ids = BTreeMap::new();
    for package in packages {
        let name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| ReservationError::MissingSymbol(package.display_name().into()))?;
        let id = program.add_package(package.identity().clone(), name)?;
        ids.insert(package.identity().clone(), id);
        if package.mode() == crate::PackageMode::Declared {
            let module = ModuleIdentity::new(package.identity().clone(), Vec::<&str>::new());
            let tree = sources
                .iter()
                .find(|source| {
                    source.module() == &module && source.kind() == ModuleSourceKind::Root
                })
                .map(SurfaceSource::syntax)
                .ok_or_else(|| ReservationError::UnknownModule(module.clone()))?;
            source_index.insert_documentation(
                SemanticEntity::Package(id),
                crate::projection_recipe::DocumentationSite::File(tree.source()),
            );
            source_index.insert(
                SemanticEntity::Package(id),
                SourceRole::Declaration,
                SourceOrigin::from_node(tree, tree.root_id())
                    .map_err(|_| ReservationError::InconsistentSource(tree.source()))?,
            );
        }
    }
    Ok(ids)
}

fn reserve_modules(
    modules: &[ModuleIdentity],
    packages: &BTreeMap<crate::PackageIdentity, PackageId>,
    program: &mut DeclarationProgramBuilder,
) -> Result<BTreeMap<ModuleIdentity, ModuleId>, ReservationError> {
    let mut ids = BTreeMap::new();
    for module in modules {
        let package = *packages
            .get(module.package())
            .ok_or_else(|| ReservationError::UnknownPackage(module.clone()))?;
        let path = module
            .path()
            .iter()
            .map(|segment| {
                program
                    .symbols()
                    .get(segment)
                    .ok_or_else(|| ReservationError::MissingSymbol(segment.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        ids.insert(
            module.clone(),
            program.add_module(package, ModulePath::from_segments(path))?,
        );
    }
    Ok(ids)
}

fn project_block_imports(
    imports: &[SurfaceBlockImport],
    modules: &BTreeMap<ModuleIdentity, ModuleId>,
    projection: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<(), ReservationError> {
    for import in imports {
        let target = *modules
            .get(import.target())
            .ok_or_else(|| ReservationError::UnknownModule(import.target().clone()))?;
        projection.insert_block_import(import.node(), target)?;
    }
    Ok(())
}

fn project_sources(
    sources: &[SurfaceSource<'_>],
    modules: &BTreeMap<ModuleIdentity, ModuleId>,
    source_index: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<Vec<ModuleId>, ReservationError> {
    sources
        .iter()
        .map(|source| {
            let module = *modules
                .get(source.module())
                .ok_or_else(|| ReservationError::UnknownModule(source.module().clone()))?;
            let role = match source.kind() {
                ModuleSourceKind::Root | ModuleSourceKind::SingleFile => SourceRole::Declaration,
                ModuleSourceKind::Implementation => SourceRole::Implementation,
            };
            let owns_module_documentation = source.kind() == ModuleSourceKind::SingleFile
                || source.kind() == ModuleSourceKind::Root && !source.module().path().is_empty();
            if owns_module_documentation {
                source_index.insert_documentation(
                    SemanticEntity::Module(module),
                    crate::projection_recipe::DocumentationSite::File(source.syntax().source()),
                );
            }
            source_index.insert_module_source(
                module,
                source.syntax().source(),
                role,
                SourceOrigin::from_node(source.syntax(), source.syntax().root_id())
                    .map_err(|_| ReservationError::InconsistentSource(source.syntax().source()))?,
            );
            Ok(module)
        })
        .collect()
}

fn project_declaration_documentation(
    sources: &[SurfaceSource<'_>],
    declarations: &[SurfaceDeclaration],
    contracts: &DeclarationContracts,
    entities: &[Option<ReservedEntity>],
    source_index: &mut crate::frontend_projection::FrontendProjectionBuilder,
) -> Result<(), ReservationError> {
    for (index, declaration) in declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let Some(entity) = entities[index] else {
            continue;
        };
        let source = sources
            .get(declaration.source().index())
            .ok_or(InconsistentSurface(id))?;
        if contracts.representative(id) == id {
            source_index.insert_documentation(
                entity.semantic_entity(),
                crate::projection_recipe::DocumentationSite::Node(declaration.node()),
            );
        } else {
            let origin = match declaration.entity_origin() {
                SyntaxOrigin::Node(node) => SourceOrigin::from_node(source.syntax(), node)
                    .map_err(|_| ReservationError::InconsistentSource(node.source()))?,
                SyntaxOrigin::Token(token) => SourceOrigin::from_token(source.syntax(), token)
                    .map_err(|_| ReservationError::InconsistentSource(token.source()))?,
            };
            source_index.insert_occurrence_documentation(
                entity.semantic_entity(),
                origin,
                declaration.node(),
            );
        }
    }
    Ok(())
}

fn reserve_surface_entities(
    declarations: &[SurfaceDeclaration],
    contracts: &DeclarationContracts,
    builtin_types: &[crate::toolchain::ResolvedBuiltinType],
    program: &mut DeclarationProgramBuilder,
) -> Result<ReservedEntityIndex, ReservationError> {
    let mut entities = vec![None; declarations.len()];
    for (index, declaration) in declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let representative = contracts.representative(id);
        if representative != id {
            continue;
        }
        validate_owner(declarations, id, declaration)?;
        entities[index] = if declaration.kind() == SurfaceDeclarationKind::PrimitiveType {
            builtin_types
                .iter()
                .copied()
                .find(|builtin| builtin.declaration() == id)
                .map(|builtin| ReservedEntity::BuiltinType(builtin.builtin()))
        } else {
            reserve_entity(program, declaration.kind())
        };
        if declaration.kind() == SurfaceDeclarationKind::PrimitiveType && entities[index].is_none()
        {
            return Err(InconsistentSurface(id));
        }
    }
    for index in 0..declarations.len() {
        let id = SurfaceDeclarationId::from_index(index);
        let representative = contracts.representative(id);
        if representative != id {
            entities[index] = entities.get(representative.index()).copied().flatten();
            if entities[index].is_none() {
                return Err(InconsistentSurface(id));
            }
        }
    }
    ReservedEntityIndex::new(entities, contracts)
}

fn reserve_entity(
    program: &mut DeclarationProgramBuilder,
    kind: SurfaceDeclarationKind,
) -> Option<ReservedEntity> {
    let declarations = program.declarations_mut();
    match kind {
        SurfaceDeclarationKind::Struct | SurfaceDeclarationKind::Enum => Some(
            ReservedEntity::NominalType(declarations.reserve_nominal_type()),
        ),
        SurfaceDeclarationKind::TypeAlias => {
            Some(ReservedEntity::TypeAlias(declarations.reserve_type_alias()))
        }
        SurfaceDeclarationKind::Interface => {
            Some(ReservedEntity::Interface(declarations.reserve_interface()))
        }
        SurfaceDeclarationKind::AssociatedType => Some(ReservedEntity::AssociatedType(
            declarations.reserve_associated_type(),
        )),
        SurfaceDeclarationKind::Constant => {
            Some(ReservedEntity::Constant(declarations.reserve_constant()))
        }
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::PrimitiveFunction
        | SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::ConstructionFunction
        | SurfaceDeclarationKind::Literal
        | SurfaceDeclarationKind::InherentMethod
        | SurfaceDeclarationKind::Coercion
        | SurfaceDeclarationKind::Equality
        | SurfaceDeclarationKind::Ordering
        | SurfaceDeclarationKind::Index
        | SurfaceDeclarationKind::Expansion => {
            Some(ReservedEntity::Callable(declarations.reserve_callable()))
        }
        SurfaceDeclarationKind::Construction => Some(ReservedEntity::Construction(
            declarations.reserve_construction(),
        )),
        SurfaceDeclarationKind::Instance => {
            Some(ReservedEntity::Instance(declarations.reserve_instance()))
        }
        SurfaceDeclarationKind::InterfaceImplementation => {
            Some(ReservedEntity::InterfaceImplementation(
                declarations.reserve_interface_implementation(),
            ))
        }
        SurfaceDeclarationKind::Drop => Some(ReservedEntity::Drop(declarations.reserve_drop())),
        SurfaceDeclarationKind::Test => Some(ReservedEntity::Test(declarations.reserve_test())),
        SurfaceDeclarationKind::Variant => {
            Some(ReservedEntity::Variant(declarations.reserve_variant()))
        }
        SurfaceDeclarationKind::OpaqueType => Some(ReservedEntity::OpaqueType(
            declarations.reserve_opaque_type(),
        )),
        SurfaceDeclarationKind::Field | SurfaceDeclarationKind::PrimitiveType => None,
    }
}

fn validate_owner(
    declarations: &[SurfaceDeclaration],
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<(), ReservationError> {
    let actual = declaration
        .owner()
        .map(|owner| declarations.get(owner.index()).map(|owner| owner.kind()));
    let valid = match declaration.kind() {
        SurfaceDeclarationKind::Field => actual == Some(Some(SurfaceDeclarationKind::Struct)),
        SurfaceDeclarationKind::Variant => actual == Some(Some(SurfaceDeclarationKind::Enum)),
        SurfaceDeclarationKind::AssociatedType | SurfaceDeclarationKind::InterfaceMethod => {
            actual == Some(Some(SurfaceDeclarationKind::Interface))
        }
        SurfaceDeclarationKind::ConstructionFunction | SurfaceDeclarationKind::Literal => {
            actual == Some(Some(SurfaceDeclarationKind::Construction))
        }
        SurfaceDeclarationKind::InherentMethod
        | SurfaceDeclarationKind::Coercion
        | SurfaceDeclarationKind::Equality
        | SurfaceDeclarationKind::Ordering
        | SurfaceDeclarationKind::Index
        | SurfaceDeclarationKind::Expansion
        | SurfaceDeclarationKind::InterfaceImplementation => {
            actual == Some(Some(SurfaceDeclarationKind::Instance))
        }
        SurfaceDeclarationKind::OpaqueType => actual.is_some_and(|kind| {
            kind.is_some_and(|kind| {
                matches!(
                    kind,
                    SurfaceDeclarationKind::Function
                        | SurfaceDeclarationKind::PrimitiveFunction
                        | SurfaceDeclarationKind::InterfaceMethod
                        | SurfaceDeclarationKind::ConstructionFunction
                        | SurfaceDeclarationKind::Literal
                        | SurfaceDeclarationKind::InherentMethod
                        | SurfaceDeclarationKind::Coercion
                        | SurfaceDeclarationKind::Equality
                        | SurfaceDeclarationKind::Ordering
                        | SurfaceDeclarationKind::Index
                        | SurfaceDeclarationKind::Expansion
                )
            })
        }),
        SurfaceDeclarationKind::Constant
        | SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::PrimitiveFunction
        | SurfaceDeclarationKind::TypeAlias
        | SurfaceDeclarationKind::Struct
        | SurfaceDeclarationKind::Enum
        | SurfaceDeclarationKind::Interface
        | SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::Drop
        | SurfaceDeclarationKind::Test
        | SurfaceDeclarationKind::PrimitiveType => actual.is_none(),
    };
    if valid {
        Ok(())
    } else {
        Err(ReservationError::InvalidOwner(id))
    }
}

#[cfg(test)]
mod tests;
