use nocter_declarations::{DeclarationGraph, Visibility};
use nocter_frontend_bindings::{SourceAccessError, SourceAccessTable};
use nocter_model::{DeclarationSiteId, ModuleId, NominalTypeId};
use nocter_source::SourceId;

/// Exact source and module authority for one lexical visibility decision.
#[derive(Clone, Copy)]
pub(crate) struct SourceAccessContext<'program> {
    access: &'program SourceAccessTable,
    source: SourceId,
    module: ModuleId,
}

impl<'program> SourceAccessContext<'program> {
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
    pub(crate) const fn module(self) -> ModuleId {
        self.module
    }
}

pub(crate) fn site_is_visible(
    graph: &DeclarationGraph,
    site: DeclarationSiteId,
    from: SourceAccessContext<'_>,
) -> Result<bool, SourceVisibilityError> {
    let declaration = graph
        .declaration_sites()
        .get(site)
        .copied()
        .ok_or(SourceVisibilityError::MissingSite(site))?;
    if declaration.visibility() == Visibility::Private {
        return from
            .access
            .can_access_private(from.source, site)
            .map_err(SourceVisibilityError::Access);
    }
    Ok(graph.is_visible_from(declaration.visibility(), from.module, declaration.module()))
}

pub(crate) fn representation_is_visible(
    nominal: NominalTypeId,
    from: SourceAccessContext<'_>,
) -> Result<bool, SourceVisibilityError> {
    from.access
        .can_access_representation(from.source, nominal)
        .map_err(SourceVisibilityError::Access)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceVisibilityError {
    MissingSite(DeclarationSiteId),
    Access(SourceAccessError),
}
