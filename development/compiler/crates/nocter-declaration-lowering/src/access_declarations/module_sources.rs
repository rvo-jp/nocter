//! Selected physical sources for the value-access pipeline, without released semantic conversion.
use super::{Failure, validate_source_role};
use crate::{
    CompileUnitInput, DeclarationContractDiagnostic, DeclarationContractError,
    DeclarationContracts, DeclarationSurface, SurfaceDeclaration, SurfaceDeclarationId,
    SurfaceError, SurfaceImport, SurfaceSource, SurfaceVisibility,
};
use nocter_source::SourceMap;
use nocter_syntax::BoundSyntax;

/// Source topology and role evidence only: no normalized types, name bindings or accepted Unit.
/// The private surface cannot be passed into released declaration reservation.
#[derive(Debug)]
pub struct ModuleSources<'syntax> {
    surface: DeclarationSurface<'syntax>,
}
#[derive(Debug)]
pub enum ModuleSourcesFailure {
    Surface(SurfaceError),
    Role(Failure),
    Contract(DeclarationContractDiagnostic),
    InconsistentContract(DeclarationContractError),
}
/// Source-structural pairs only. Resolved header equality and value/type namespace closure
/// remain required before one semantic declaration authority can be published.
#[derive(Debug)]
pub struct PairedModuleSources<'syntax> {
    inventory: ModuleSources<'syntax>,
    contracts: DeclarationContracts,
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
                    .ok_or(ModuleSourcesFailure::Role(Failure::InconsistentSyntax))?,
                source.syntax(),
            )
            .ok_or(ModuleSourcesFailure::Role(Failure::InconsistentSyntax))?;
            validate_source_role(syntax, source.kind()).map_err(ModuleSourcesFailure::Role)?;
        }
        Ok(Self { surface })
    }
    /// Pair root contracts with directly reciprocal complete definitions, without resolving types.
    /// # Errors
    /// Reports missing, duplicate or structurally mismatching definitions at their real sources.
    pub fn pair(self) -> Result<PairedModuleSources<'syntax>, ModuleSourcesFailure> {
        let contracts = crate::analyze_declaration_contracts(&self.surface).map_err(|error| {
            match DeclarationContractDiagnostic::project(error, &self.surface) {
                Ok(diagnostic) => ModuleSourcesFailure::Contract(diagnostic),
                Err(error) => ModuleSourcesFailure::InconsistentContract(error),
            }
        })?;
        Ok(PairedModuleSources {
            inventory: self,
            contracts,
        })
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
    pub const fn imports(&self) -> &[SurfaceImport] {
        self.surface.imports()
    }
}
