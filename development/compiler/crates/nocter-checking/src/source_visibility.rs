use nocter_declarations::{DeclarationGraph, Visibility};
use nocter_frontend_bindings::{SourceAccessError, SourceAccessTable};
use nocter_model::{DeclarationSiteId, ModuleId, NominalTypeId};
use nocter_source::SourceId;

/// Exact source and module authority for semantic visibility decisions.
///
/// This value is the checking layer's read-only contract for source-private declaration and
/// representation access. Editor analysis may consume the contract, but cannot inspect or
/// reconstruct the underlying source-visibility graph.
#[derive(Clone, Copy)]
pub struct SourceAccessContext<'program> {
    access: &'program SourceAccessTable,
    source: SourceId,
    module: ModuleId,
}

impl<'program> SourceAccessContext<'program> {
    /// Creates the visibility context for one exact source in the prepared compile unit.
    ///
    /// # Errors
    ///
    /// Returns an error when declaration lowering did not publish the source's semantic module.
    pub(crate) fn for_source(
        access: &'program SourceAccessTable,
        source: SourceId,
    ) -> Result<Self, SourceAccessError> {
        Ok(Self {
            access,
            source,
            module: access.module_for_source(source)?,
        })
    }

    #[must_use]
    pub const fn module(self) -> ModuleId {
        self.module
    }

    /// Determines whether one declaration site is visible from this exact source.
    ///
    /// # Errors
    ///
    /// Returns an error when the declaration graph and source-access authority disagree.
    pub fn site_is_visible(
        self,
        graph: &DeclarationGraph,
        site: DeclarationSiteId,
    ) -> Result<bool, SourceVisibilityError> {
        let declaration = graph
            .declaration_sites()
            .get(site)
            .copied()
            .ok_or(SourceVisibilityError::MissingSite(site))?;
        if declaration.visibility() == Visibility::Private {
            return self
                .access
                .can_access_private(self.source, site)
                .map_err(SourceVisibilityError::Access);
        }
        Ok(graph.is_visible_from(declaration.visibility(), self.module, declaration.module()))
    }

    /// Determines whether this source may inspect one complete nominal representation.
    ///
    /// Direct source visibility grants private representation access. Every other source may
    /// inspect only a representation that is not sealed by a bodyless public nominal contract.
    ///
    /// # Errors
    ///
    /// Returns an error when lowering did not publish the nominal representation relation.
    pub fn representation_is_visible(
        self,
        nominal: NominalTypeId,
    ) -> Result<bool, SourceVisibilityError> {
        if self
            .access
            .can_access_representation(self.source, nominal)
            .map_err(SourceVisibilityError::Access)?
        {
            return Ok(true);
        }
        self.access
            .representation_is_contract_private(nominal)
            .map(|contract_private| !contract_private)
            .map_err(SourceVisibilityError::Access)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceVisibilityError {
    MissingSite(DeclarationSiteId),
    Access(SourceAccessError),
}

impl std::fmt::Display for SourceVisibilityError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingSite(site) => write!(formatter, "missing declaration site {site:?}"),
            Self::Access(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SourceVisibilityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MissingSite(_) => None,
            Self::Access(error) => Some(error),
        }
    }
}
