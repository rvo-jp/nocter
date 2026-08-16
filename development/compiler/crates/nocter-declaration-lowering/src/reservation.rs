use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::{DeclarationProgramBuilder, ModulePath, ProgramBuildError};
use nocter_model::{
    AssociatedTypeId, CallableId, ConformanceId, ConstructionId, DropId, InstanceId, InterfaceId,
    ModuleId, NominalTypeId, OpaqueTypeId, PackageId, TestId, TypeAliasId, VariantId,
};
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::{
    DuplicateSourceBinding, SemanticEntity, SourceIndexBuilder, SourceOrigin, SourceRole,
};

use crate::surface::SurfaceParts;
use crate::{
    CallableContractError, CallableContracts, DeclarationSurface, ModuleIdentity, ModuleSourceKind,
    PackageInput, ReservationError::InconsistentSurface, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceDeclarationKind, SurfaceImport, SurfaceSource, analyze_callable_contracts,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ReservedEntity {
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Callable(CallableId),
    Construction(ConstructionId),
    Instance(InstanceId),
    Conformance(ConformanceId),
    Drop(DropId),
    Test(TestId),
    Variant(VariantId),
    OpaqueType(OpaqueTypeId),
}

impl ReservedEntity {
    #[must_use]
    pub const fn semantic_entity(self) -> SemanticEntity {
        match self {
            Self::NominalType(id) => SemanticEntity::NominalType(id),
            Self::TypeAlias(id) => SemanticEntity::TypeAlias(id),
            Self::Interface(id) => SemanticEntity::Interface(id),
            Self::AssociatedType(id) => SemanticEntity::AssociatedType(id),
            Self::Callable(id) => SemanticEntity::Callable(id),
            Self::Construction(id) => SemanticEntity::Construction(id),
            Self::Instance(id) => SemanticEntity::Instance(id),
            Self::Conformance(id) => SemanticEntity::Conformance(id),
            Self::Drop(id) => SemanticEntity::Drop(id),
            Self::Test(id) => SemanticEntity::Test(id),
            Self::Variant(id) => SemanticEntity::Variant(id),
            Self::OpaqueType(id) => SemanticEntity::OpaqueType(id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReservationError {
    Contract(CallableContractError),
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
    MissingSymbol(Box<str>),
    UnknownPackage(ModuleIdentity),
    UnknownModule(ModuleIdentity),
    InvalidOwner(SurfaceDeclarationId),
    InconsistentSurface(SurfaceDeclarationId),
    InconsistentSource(SourceId),
}

impl fmt::Display for ReservationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => error.fmt(formatter),
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
            Self::MissingSymbol(spelling) => {
                write!(formatter, "canonical symbol table is missing {spelling}")
            }
            Self::UnknownPackage(module) => {
                write!(formatter, "module {module:?} belongs to an unknown package")
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
        }
    }
}

impl std::error::Error for ReservationError {}

impl From<CallableContractError> for ReservationError {
    fn from(error: CallableContractError) -> Self {
        Self::Contract(error)
    }
}

impl From<ProgramBuildError> for ReservationError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DuplicateSourceBinding> for ReservationError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

/// A complete semantic-identity reservation awaiting header definition.
///
/// The syntax-owned fields remain private to the lowering crate and are consumed by the header
/// resolver. External users can inspect only the stable topology and typed reservation mapping.
#[derive(Debug)]
pub struct ReservedDeclarations<'syntax> {
    pub(crate) program: DeclarationProgramBuilder,
    pub(crate) source_index: SourceIndexBuilder,
    pub(crate) source_map: &'syntax SourceMap,
    pub(crate) packages: Box<[PackageInput<'syntax>]>,
    pub(crate) package_ids: Box<[PackageId]>,
    pub(crate) modules: Box<[ModuleIdentity]>,
    pub(crate) module_ids: Box<[ModuleId]>,
    pub(crate) sources: Box<[SurfaceSource<'syntax>]>,
    pub(crate) source_modules: Box<[ModuleId]>,
    pub(crate) imports: Box<[SurfaceImport]>,
    pub(crate) declarations: Box<[SurfaceDeclaration]>,
    pub(crate) contracts: CallableContracts,
    pub(crate) entities: Box<[Option<ReservedEntity>]>,
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
    pub const fn packages(&self) -> &[PackageInput<'_>] {
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
    pub const fn contracts(&self) -> &CallableContracts {
        &self.contracts
    }

    #[must_use]
    pub fn source_binding_count(&self) -> usize {
        self.source_index.len()
    }

    #[must_use]
    pub fn entity(&self, declaration: SurfaceDeclarationId) -> Option<ReservedEntity> {
        self.entities.get(declaration.index()).copied().flatten()
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
        &self.entities
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
pub fn reserve_declaration_identities(
    surface: DeclarationSurface<'_>,
) -> Result<ReservedDeclarations<'_>, ReservationError> {
    let contracts = analyze_callable_contracts(&surface)?;
    reserve_with_contracts(surface, contracts)
}

pub(crate) fn reserve_with_contracts(
    surface: DeclarationSurface<'_>,
    contracts: CallableContracts,
) -> Result<ReservedDeclarations<'_>, ReservationError> {
    let SurfaceParts {
        source_map,
        symbols,
        packages,
        modules,
        sources,
        imports,
        declarations,
    } = surface.into_parts();
    let mut program = DeclarationProgramBuilder::new(symbols);
    let mut source_index = SourceIndexBuilder::new();
    let package_ids = reserve_packages(&packages, &mut program, &mut source_index)?;
    let module_ids = reserve_modules(&modules, &package_ids, &mut program)?;
    let source_modules = project_sources(&sources, &module_ids, &mut source_index)?;
    let entities = reserve_surface_entities(&declarations, &contracts, &mut program)?;
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
        imports,
        declarations,
        contracts,
        entities: entities.into_boxed_slice(),
    })
}

fn reserve_packages(
    packages: &[PackageInput<'_>],
    program: &mut DeclarationProgramBuilder,
    source_index: &mut SourceIndexBuilder,
) -> Result<BTreeMap<crate::PackageIdentity, PackageId>, ReservationError> {
    let mut ids = BTreeMap::new();
    for package in packages {
        let name = program
            .symbols()
            .get(package.display_name())
            .ok_or_else(|| ReservationError::MissingSymbol(package.display_name().into()))?;
        let id = program.add_package(name)?;
        ids.insert(package.identity().clone(), id);
        if let Some(declaration) = package.declaration() {
            let tree = declaration.syntax();
            source_index.insert(
                SemanticEntity::Package(id),
                SourceRole::Declaration,
                SourceOrigin::from_node(tree, tree.root_id())
                    .map_err(|_| ReservationError::InconsistentSource(tree.source()))?,
            )?;
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

fn project_sources(
    sources: &[SurfaceSource<'_>],
    modules: &BTreeMap<ModuleIdentity, ModuleId>,
    source_index: &mut SourceIndexBuilder,
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
            source_index.insert(
                SemanticEntity::Module(module),
                role,
                SourceOrigin::from_node(source.syntax(), source.syntax().root_id())
                    .map_err(|_| ReservationError::InconsistentSource(source.syntax().source()))?,
            )?;
            Ok(module)
        })
        .collect()
}

fn reserve_surface_entities(
    declarations: &[SurfaceDeclaration],
    contracts: &CallableContracts,
    program: &mut DeclarationProgramBuilder,
) -> Result<Vec<Option<ReservedEntity>>, ReservationError> {
    let mut entities = vec![None; declarations.len()];
    for (index, declaration) in declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let representative = contracts.representative(id);
        if representative != id {
            continue;
        }
        validate_owner(declarations, id, declaration)?;
        entities[index] = reserve_entity(program, declaration.kind());
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
    Ok(entities)
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
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::Primitive
        | SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::ConstructionFunction
        | SurfaceDeclarationKind::Literal
        | SurfaceDeclarationKind::InherentMethod
        | SurfaceDeclarationKind::Coercion
        | SurfaceDeclarationKind::Equality
        | SurfaceDeclarationKind::Ordering
        | SurfaceDeclarationKind::Index
        | SurfaceDeclarationKind::Expansion
        | SurfaceDeclarationKind::ConformanceMethod => {
            Some(ReservedEntity::Callable(declarations.reserve_callable()))
        }
        SurfaceDeclarationKind::Construction => Some(ReservedEntity::Construction(
            declarations.reserve_construction(),
        )),
        SurfaceDeclarationKind::Instance => {
            Some(ReservedEntity::Instance(declarations.reserve_instance()))
        }
        SurfaceDeclarationKind::Conformance => Some(ReservedEntity::Conformance(
            declarations.reserve_conformance(),
        )),
        SurfaceDeclarationKind::Drop => Some(ReservedEntity::Drop(declarations.reserve_drop())),
        SurfaceDeclarationKind::Test => Some(ReservedEntity::Test(declarations.reserve_test())),
        SurfaceDeclarationKind::Variant => {
            Some(ReservedEntity::Variant(declarations.reserve_variant()))
        }
        SurfaceDeclarationKind::OpaqueType => Some(ReservedEntity::OpaqueType(
            declarations.reserve_opaque_type(),
        )),
        SurfaceDeclarationKind::Field => None,
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
        | SurfaceDeclarationKind::Expansion => {
            actual == Some(Some(SurfaceDeclarationKind::Instance))
        }
        SurfaceDeclarationKind::ConformanceMethod => {
            actual == Some(Some(SurfaceDeclarationKind::Conformance))
        }
        SurfaceDeclarationKind::OpaqueType => actual.is_some_and(|kind| {
            kind.is_some_and(|kind| {
                matches!(
                    kind,
                    SurfaceDeclarationKind::Function
                        | SurfaceDeclarationKind::Primitive
                        | SurfaceDeclarationKind::InterfaceMethod
                        | SurfaceDeclarationKind::ConstructionFunction
                        | SurfaceDeclarationKind::Literal
                        | SurfaceDeclarationKind::InherentMethod
                        | SurfaceDeclarationKind::Coercion
                        | SurfaceDeclarationKind::Equality
                        | SurfaceDeclarationKind::Ordering
                        | SurfaceDeclarationKind::Index
                        | SurfaceDeclarationKind::Expansion
                        | SurfaceDeclarationKind::ConformanceMethod
                )
            })
        }),
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::Primitive
        | SurfaceDeclarationKind::TypeAlias
        | SurfaceDeclarationKind::Struct
        | SurfaceDeclarationKind::Enum
        | SurfaceDeclarationKind::Interface
        | SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::Conformance
        | SurfaceDeclarationKind::Drop
        | SurfaceDeclarationKind::Test => actual.is_none(),
    };
    if valid {
        Ok(())
    } else {
        Err(ReservationError::InvalidOwner(id))
    }
}

#[cfg(test)]
mod tests;
