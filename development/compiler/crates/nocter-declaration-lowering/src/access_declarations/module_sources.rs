//! Selected physical sources for the value-access pipeline, without released semantic conversion.
use super::{Failure, validate_source_role};
use crate::{
    CompileUnitInput, DeclarationSurface, SurfaceDeclaration, SurfaceError, SurfaceImport,
    SurfaceSource, SurfaceVisibility,
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
