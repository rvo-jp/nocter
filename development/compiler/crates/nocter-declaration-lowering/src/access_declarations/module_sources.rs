//! Selected physical sources for the value-access pipeline, without released semantic conversion.
use super::{Failure, validate_source_role};
use crate::{
    CompileUnitInput, DeclarationContractDiagnostic, DeclarationContractError,
    DeclarationContracts, DeclarationSurface, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceError, SurfaceImport, SurfaceSource, SurfaceSourceId, SurfaceVisibility,
};
use nocter_source::SourceMap;
use nocter_syntax::BoundSyntax;

/// Declarations that can introduce a name in the source's ordinary lookup scope.
pub(super) fn has_source_name(declaration: SurfaceDeclaration) -> bool {
    declaration.owner().is_none()
        && !matches!(
            declaration.kind(),
            crate::SurfaceDeclarationKind::Test
                | crate::SurfaceDeclarationKind::TypeBehavior
                | crate::SurfaceDeclarationKind::PrimitiveType
        )
}

/// Physical source lookup consists of the current file and its directly seen files.
pub(super) fn visible_sources<'a>(
    paired: &'a PairedModuleSources<'_>,
    source: SurfaceSourceId,
) -> impl Iterator<Item = SurfaceSourceId> + 'a {
    std::iter::once(source).chain(
        paired
            .inventory
            .source_visibilities()
            .iter()
            .filter(move |edge| edge.source() == source)
            .map(|edge| edge.target()),
    )
}

/// Source topology and role evidence only: no normalized types, name bindings or accepted Unit.
/// The private surface cannot be passed into released declaration reservation.
#[derive(Debug)]
pub struct ModuleSources<'syntax> {
    pub(super) surface: DeclarationSurface<'syntax>,
    pub(super) audiences: super::module_audiences::ModuleAudiences,
    pub(super) primitive_roles: Vec<crate::toolchain::ResolvedPrimitiveRole>,
    pub(super) builtin_types: Vec<crate::toolchain::ResolvedBuiltinType>,
}
#[derive(Debug)]
pub enum ModuleSourcesFailure {
    Surface(SurfaceError),
    PrimitiveAuthority(super::PrimitiveAuthorityError),
    Toolchain(crate::ToolchainError),
    Access(Failure),
    Contract(DeclarationContractDiagnostic),
    InconsistentContract(DeclarationContractError),
}
/// Source-structural pairs only. Resolved header equality and value/type namespace closure
/// remain required before one semantic declaration authority can be published.
#[derive(Debug)]
pub struct PairedModuleSources<'syntax> {
    pub(super) inventory: ModuleSources<'syntax>,
    pub(super) contracts: DeclarationContracts,
}
impl<'syntax> PairedModuleSources<'syntax> {
    #[must_use]
    pub const fn inventory(&self) -> &ModuleSources<'syntax> {
        &self.inventory
    }
    #[must_use]
    pub const fn contracts(&self) -> &DeclarationContracts {
        &self.contracts
    }
}

impl<'syntax> ModuleSources<'syntax> {
    /// Consume discovery's closed inventory and exact physical resolutions for v2 sources.
    /// # Errors
    /// Rejects mixed/wrong grammar, inconsistent topology, syntax errors and source-role violations.
    pub fn prepare(input: &CompileUnitInput<'syntax>) -> Result<Self, ModuleSourcesFailure> {
        let surface = crate::surface::collect_access_declaration_surface(input)
            .map_err(ModuleSourcesFailure::Surface)?;
        for source in surface.sources() {
            let syntax = BoundSyntax::new(
                surface
                    .source_map()
                    .get(source.syntax().source())
                    .ok_or(ModuleSourcesFailure::Access(Failure::InconsistentSyntax))?,
                source.syntax(),
            )
            .ok_or(ModuleSourcesFailure::Access(Failure::InconsistentSyntax))?;
            validate_source_role(syntax, source.kind()).map_err(ModuleSourcesFailure::Access)?;
        }
        let audiences = super::module_audiences::ModuleAudiences::prepare(&surface)
            .map_err(ModuleSourcesFailure::Access)?;
        let (primitive_roles, builtin_types) = if let Some(toolchain) = input.toolchain() {
            super::primitives::validate_catalog(toolchain)
                .map_err(ModuleSourcesFailure::PrimitiveAuthority)?;
            crate::toolchain::validate_locator_modules(toolchain)
                .map_err(ModuleSourcesFailure::Toolchain)?;
            (
                crate::toolchain::resolve_primitive_roles(&surface, toolchain)
                    .map_err(ModuleSourcesFailure::Toolchain)?,
                crate::toolchain::resolve_builtin_types(&surface, toolchain)
                    .map_err(ModuleSourcesFailure::Toolchain)?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        crate::surface::validate_primitive_type_authority(&surface, &builtin_types, &[])
            .map_err(ModuleSourcesFailure::Surface)?;
        Ok(Self {
            surface,
            audiences,
            primitive_roles,
            builtin_types,
        })
    }
    /// Pair root contracts with directly reciprocal complete definitions, without resolving types.
    /// # Errors
    /// Reports missing, duplicate or structurally mismatching definitions at their real sources.
    pub fn pair(self) -> Result<PairedModuleSources<'syntax>, ModuleSourcesFailure> {
        let contracts = crate::analyze_declaration_contracts(&self.surface)
            .map_err(|error| contract_failure(error, &self.surface))?;
        Ok(PairedModuleSources {
            inventory: self,
            contracts,
        })
    }
    #[must_use]
    pub fn audience(&self, declaration: SurfaceDeclarationId) -> Option<&super::ModuleAudience> {
        self.audiences.declaration(declaration)
    }
    #[must_use]
    pub fn import_audience(&self, node: nocter_syntax::NodeId) -> Option<&super::ModuleAudience> {
        self.audiences.import(node)
    }
    #[must_use]
    pub fn source_entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (crate::SurfaceSourceId, &SurfaceSource<'syntax>)> {
        self.sources()
            .iter()
            .enumerate()
            .map(|(index, source)| (crate::SurfaceSourceId::from_index(index), source))
    }
    #[must_use]
    pub fn declaration_entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (SurfaceDeclarationId, SurfaceDeclaration)> + '_ {
        self.declarations()
            .iter()
            .copied()
            .enumerate()
            .map(|(index, declaration)| (SurfaceDeclarationId::from_index(index), declaration))
    }
    #[must_use]
    pub fn source_map(&self) -> &SourceMap {
        self.surface.source_map()
    }
    #[must_use]
    pub const fn sources(&self) -> &[SurfaceSource<'syntax>] {
        self.surface.sources()
    }
    #[must_use]
    pub const fn declarations(&self) -> &[SurfaceDeclaration] {
        self.surface.declarations()
    }
    #[must_use]
    pub const fn source_visibilities(&self) -> &[SurfaceVisibility] {
        self.surface.source_visibilities()
    }
    #[must_use]
    pub const fn block_imports(&self) -> &[crate::SurfaceBlockImport] {
        self.surface.block_imports()
    }
    /// The topology owner's dependency-first order; includes every selected import scope.
    #[must_use]
    pub fn modules_in_dependency_order(
        &self,
    ) -> impl ExactSizeIterator<Item = &nocter_compile_input::ModuleIdentity> {
        self.surface.modules_in_dependency_order()
    }
    #[must_use]
    pub const fn imports(&self) -> &[SurfaceImport] {
        self.surface.imports()
    }
}

pub(super) fn contract_failure(
    error: DeclarationContractError,
    surface: &DeclarationSurface<'_>,
) -> ModuleSourcesFailure {
    match DeclarationContractDiagnostic::project(error, surface) {
        Ok(diagnostic) => ModuleSourcesFailure::Contract(diagnostic),
        Err(error) => ModuleSourcesFailure::InconsistentContract(error),
    }
}
