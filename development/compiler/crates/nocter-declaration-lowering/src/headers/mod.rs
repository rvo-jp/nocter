mod names;

use std::fmt;

use nocter_declarations::{ProgramBuildError, Visibility};
use nocter_model::{DeclarationSiteId, Symbol};
use nocter_source::SourceId;
use nocter_source_index::{DuplicateSourceBinding, SemanticEntity, SourceOrigin, SourceRole};

use crate::visibility::{VisibilityResolutionError, resolve_authored};
use crate::{ReservedDeclarations, SurfaceDeclarationId, SurfaceDeclarationKind, SurfaceSourceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderError {
    Program(ProgramBuildError),
    DuplicateSourceBinding(DuplicateSourceBinding),
    MissingSource(SurfaceSourceId),
    MissingName(SurfaceDeclarationId),
    InvalidName(SurfaceDeclarationId),
    InconsistentName(SurfaceDeclarationId),
    DuplicateModuleName {
        first: SurfaceDeclarationId,
        second: SurfaceDeclarationId,
    },
    DuplicateMemberName {
        first: SurfaceDeclarationId,
        second: SurfaceDeclarationId,
    },
    DuplicateTestName {
        first: SurfaceDeclarationId,
        second: SurfaceDeclarationId,
    },
    InvalidVisibility(SurfaceDeclarationId),
    VisibilityAbovePackageRoot(SurfaceDeclarationId),
    InconsistentSource(SourceId),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Program(error) => error.fmt(formatter),
            Self::DuplicateSourceBinding(error) => error.fmt(formatter),
            Self::MissingSource(source) => {
                write!(formatter, "surface source {source:?} is missing")
            }
            Self::MissingName(declaration) => {
                write!(formatter, "declaration {declaration:?} requires a name")
            }
            Self::InvalidName(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} uses a reserved name"
                )
            }
            Self::InconsistentName(declaration) => write!(
                formatter,
                "implementation declaration {declaration:?} changed its contract name"
            ),
            Self::DuplicateModuleName { first, second } => write!(
                formatter,
                "module declarations {first:?} and {second:?} introduce the same name"
            ),
            Self::DuplicateMemberName { first, second } => write!(
                formatter,
                "member declarations {first:?} and {second:?} introduce the same name"
            ),
            Self::DuplicateTestName { first, second } => write!(
                formatter,
                "tests {first:?} and {second:?} introduce the same module-local test name"
            ),
            Self::InvalidVisibility(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has invalid visibility"
                )
            }
            Self::VisibilityAbovePackageRoot(declaration) => write!(
                formatter,
                "declaration {declaration:?} moves visibility above its package root"
            ),
            Self::InconsistentSource(source) => {
                write!(formatter, "{source} has an inconsistent declaration origin")
            }
        }
    }
}

impl std::error::Error for HeaderError {}

impl From<ProgramBuildError> for HeaderError {
    fn from(error: ProgramBuildError) -> Self {
        Self::Program(error)
    }
}

impl From<DuplicateSourceBinding> for HeaderError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateSourceBinding(error)
    }
}

/// Reserved declarations after names, visibility, and declaration sites are fixed.
#[derive(Debug)]
pub struct PreparedHeaders<'syntax> {
    pub(crate) reserved: ReservedDeclarations<'syntax>,
    pub(crate) names: Box<[Option<Symbol>]>,
    pub(crate) sites: Box<[Option<DeclarationSiteId>]>,
    pub(crate) visibility: Box<[Option<Visibility>]>,
}

impl PreparedHeaders<'_> {
    #[must_use]
    pub const fn reserved(&self) -> &ReservedDeclarations<'_> {
        &self.reserved
    }

    #[must_use]
    pub fn name(&self, declaration: SurfaceDeclarationId) -> Option<Symbol> {
        self.names.get(declaration.index()).copied().flatten()
    }

    #[must_use]
    pub fn site(&self, declaration: SurfaceDeclarationId) -> Option<DeclarationSiteId> {
        self.sites.get(declaration.index()).copied().flatten()
    }

    #[must_use]
    pub fn visibility(&self, declaration: SurfaceDeclarationId) -> Option<Visibility> {
        self.visibility.get(declaration.index()).copied().flatten()
    }
}

/// Resolves declaration spellings and visibility without resolving type names.
///
/// # Errors
///
/// Returns [`HeaderError`] for invalid or duplicate names, invalid visibility boundaries,
/// inconsistent source origins, or declaration-program construction failures.
pub fn prepare_declaration_headers(
    mut reserved: ReservedDeclarations<'_>,
) -> Result<PreparedHeaders<'_>, HeaderError> {
    let names = names::resolve(&reserved)?;
    let mut sites = vec![None; reserved.declarations.len()];
    let mut resolved_visibility = vec![None; reserved.declarations.len()];

    for index in 0..names.len() {
        let id = SurfaceDeclarationId::from_index(index);
        let declaration = reserved.declarations[index];
        let representative = reserved.contracts.representative(id);
        if representative != id {
            sites[index] = sites[representative.index()];
            resolved_visibility[index] = resolved_visibility[representative.index()];
            continue;
        }
        if declaration.kind() == SurfaceDeclarationKind::OpaqueType {
            continue;
        }
        let module = reserved
            .module_for_source(declaration.source())
            .ok_or(HeaderError::MissingSource(declaration.source()))?;
        let visibility = resolve_visibility(&reserved, id, declaration, &resolved_visibility)?;
        let site = reserved.program.add_declaration_site(module, visibility)?;
        sites[index] = Some(site);
        resolved_visibility[index] = Some(visibility);
        project_site(&mut reserved, id, site)?;
    }
    project_entities(&mut reserved, &names)?;

    Ok(PreparedHeaders {
        reserved,
        names: names.into_boxed_slice(),
        sites: sites.into_boxed_slice(),
        visibility: resolved_visibility.into_boxed_slice(),
    })
}

fn resolve_visibility(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    declaration: crate::SurfaceDeclaration,
    resolved: &[Option<Visibility>],
) -> Result<Visibility, HeaderError> {
    match declaration.kind() {
        SurfaceDeclarationKind::Variant => {
            if declaration.visibility().is_some() {
                return Err(HeaderError::InvalidVisibility(id));
            }
            declaration
                .owner()
                .and_then(|owner| resolved.get(owner.index()))
                .copied()
                .flatten()
                .ok_or(HeaderError::InvalidVisibility(id))
        }
        SurfaceDeclarationKind::ConformanceMethod
        | SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::Conformance
        | SurfaceDeclarationKind::Drop
        | SurfaceDeclarationKind::Test => {
            if declaration.visibility().is_some() {
                Err(HeaderError::InvalidVisibility(id))
            } else {
                Ok(Visibility::Private)
            }
        }
        SurfaceDeclarationKind::InterfaceMethod | SurfaceDeclarationKind::AssociatedType => {
            let visibility = authored_visibility(reserved, id, declaration)?;
            if visibility == Visibility::Public {
                Ok(visibility)
            } else {
                Err(HeaderError::InvalidVisibility(id))
            }
        }
        SurfaceDeclarationKind::OpaqueType => Err(HeaderError::InvalidVisibility(id)),
        _ => authored_visibility(reserved, id, declaration),
    }
}

fn authored_visibility(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    declaration: crate::SurfaceDeclaration,
) -> Result<Visibility, HeaderError> {
    resolve_authored(reserved, declaration.source(), declaration.visibility()).map_err(|error| {
        match error {
            VisibilityResolutionError::MissingSource(source) => HeaderError::MissingSource(source),
            VisibilityResolutionError::Invalid(_) => HeaderError::InvalidVisibility(id),
            VisibilityResolutionError::AbovePackageRoot(_) => {
                HeaderError::VisibilityAbovePackageRoot(id)
            }
        }
    })
}

fn project_site(
    reserved: &mut ReservedDeclarations<'_>,
    declaration: SurfaceDeclarationId,
    site: DeclarationSiteId,
) -> Result<(), HeaderError> {
    let origin = declaration_origin(reserved, declaration, false)?;
    reserved.source_index.insert(
        SemanticEntity::DeclarationSite(site),
        SourceRole::Declaration,
        origin,
    )?;
    Ok(())
}

fn project_entities(
    reserved: &mut ReservedDeclarations<'_>,
    names: &[Option<Symbol>],
) -> Result<(), HeaderError> {
    for (index, name) in names.iter().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let Some(entity) = reserved.entities[index] else {
            continue;
        };
        let role = if reserved.contracts.is_implementation(id) {
            SourceRole::Implementation
        } else {
            SourceRole::Declaration
        };
        let origin = declaration_origin(reserved, id, name.is_some())?;
        reserved
            .source_index
            .insert(entity.semantic_entity(), role, origin)?;
    }
    Ok(())
}

fn declaration_origin(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    named: bool,
) -> Result<SourceOrigin, HeaderError> {
    let declaration = reserved.declarations[id.index()];
    let source = reserved
        .sources
        .get(declaration.source().index())
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    if named {
        let token = declaration.name().ok_or(HeaderError::MissingName(id))?;
        SourceOrigin::from_token(source.syntax(), token)
            .map_err(|_| HeaderError::InconsistentSource(source.syntax().source()))
    } else {
        SourceOrigin::from_node(source.syntax(), declaration.node())
            .map_err(|_| HeaderError::InconsistentSource(source.syntax().source()))
    }
}

#[cfg(test)]
mod tests;
