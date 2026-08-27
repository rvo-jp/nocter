mod names;

use std::collections::HashMap;
use std::fmt;

use nocter_declarations::{ProgramBuildError, Visibility};
use nocter_model::{AssociatedTypeId, DeclarationSiteId, InterfaceId, Symbol};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::SyntaxOrigin;

use crate::visibility::{VisibilityResolutionError, resolve_authored};
use crate::{
    NamespaceViolation, ReservedDeclarations, ReservedEntity, SurfaceDeclarationId,
    SurfaceDeclarationKind, SurfaceSourceId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HeaderError {
    Namespace(NamespaceViolation),
    Program(ProgramBuildError),
    FrontendBindings(nocter_frontend_bindings::FrontendBindingDefinitionError),
    MissingDeclaration(SurfaceDeclarationId),
    MissingSource(SurfaceSourceId),
    MissingName(SurfaceDeclarationId),
    InconsistentName(SurfaceDeclarationId),
    InvalidVisibility(SurfaceDeclarationId),
    InconsistentSource(SourceId),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(violation) => write!(
                formatter,
                "{}: {}",
                violation.rule().code(),
                violation.rule().message()
            ),
            Self::Program(error) => error.fmt(formatter),
            Self::FrontendBindings(error) => error.fmt(formatter),
            Self::MissingDeclaration(declaration) => {
                write!(formatter, "surface declaration {declaration:?} is missing")
            }
            Self::MissingSource(source) => {
                write!(formatter, "surface source {source:?} is missing")
            }
            Self::MissingName(declaration) => {
                write!(formatter, "declaration {declaration:?} requires a name")
            }
            Self::InconsistentName(declaration) => write!(
                formatter,
                "implementation declaration {declaration:?} changed its contract name"
            ),
            Self::InvalidVisibility(declaration) => {
                write!(
                    formatter,
                    "declaration {declaration:?} has invalid visibility"
                )
            }
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

impl From<nocter_frontend_bindings::FrontendBindingDefinitionError> for HeaderError {
    fn from(error: nocter_frontend_bindings::FrontendBindingDefinitionError) -> Self {
        Self::FrontendBindings(error)
    }
}

impl From<NamespaceViolation> for HeaderError {
    fn from(violation: NamespaceViolation) -> Self {
        Self::Namespace(violation)
    }
}

/// Reserved declarations after names, visibility, and declaration sites are fixed.
#[derive(Debug)]
pub struct PreparedHeaders<'syntax> {
    pub(crate) reserved: ReservedDeclarations<'syntax>,
    pub(crate) names: Box<[Option<Symbol>]>,
    pub(crate) sites: Box<[Option<DeclarationSiteId>]>,
    pub(crate) visibility: Box<[Option<Visibility>]>,
    associated_types: AssociatedTypeIndex,
}

#[derive(Debug, Default)]
struct AssociatedTypeIndex {
    by_owner_and_name: HashMap<(InterfaceId, Symbol), AssociatedTypeId>,
    declarations: HashMap<AssociatedTypeId, SurfaceDeclarationId>,
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

    pub(crate) fn associated_type(
        &self,
        interface: InterfaceId,
        name: Symbol,
    ) -> Option<AssociatedTypeId> {
        self.associated_types
            .by_owner_and_name
            .get(&(interface, name))
            .copied()
    }

    pub(crate) fn associated_types(&self) -> &HashMap<(InterfaceId, Symbol), AssociatedTypeId> {
        &self.associated_types.by_owner_and_name
    }

    pub(crate) fn associated_type_declarations(
        &self,
    ) -> &HashMap<AssociatedTypeId, SurfaceDeclarationId> {
        &self.associated_types.declarations
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
    project_entities(&mut reserved)?;
    let associated_types = index_associated_types(&reserved, &names)?;

    Ok(PreparedHeaders {
        reserved,
        names: names.into_boxed_slice(),
        sites: sites.into_boxed_slice(),
        visibility: resolved_visibility.into_boxed_slice(),
        associated_types,
    })
}

fn index_associated_types(
    reserved: &ReservedDeclarations<'_>,
    names: &[Option<Symbol>],
) -> Result<AssociatedTypeIndex, HeaderError> {
    let mut index = AssociatedTypeIndex::default();
    for (entity, declaration) in reserved.entity_index.representatives() {
        let ReservedEntity::AssociatedType(associated) = *entity else {
            continue;
        };
        let owner = reserved
            .declarations
            .get(declaration.index())
            .and_then(|surface| surface.owner())
            .and_then(|owner| reserved.entity(owner));
        let Some(ReservedEntity::Interface(interface)) = owner else {
            return Err(HeaderError::MissingDeclaration(*declaration));
        };
        let name = names
            .get(declaration.index())
            .copied()
            .flatten()
            .ok_or(HeaderError::MissingName(*declaration))?;
        if index
            .by_owner_and_name
            .insert((interface, name), associated)
            .is_some()
            || index
                .declarations
                .insert(associated, *declaration)
                .is_some()
        {
            return Err(HeaderError::InconsistentName(*declaration));
        }
    }
    Ok(index)
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
            let source = reserved
                .sources
                .get(declaration.source().index())
                .ok_or(HeaderError::MissingSource(declaration.source()))?;
            if source.kind() == crate::ModuleSourceKind::Implementation {
                return Ok(Visibility::Private);
            }
            declaration
                .owner()
                .and_then(|owner| resolved.get(owner.index()))
                .copied()
                .flatten()
                .ok_or(HeaderError::InvalidVisibility(id))
        }
        SurfaceDeclarationKind::Construction
        | SurfaceDeclarationKind::Instance
        | SurfaceDeclarationKind::InterfaceImplementation
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
            VisibilityResolutionError::AbovePackageRoot(node) => {
                NamespaceViolation::visibility_above_package_root(
                    nocter_syntax::SyntaxOrigin::Node(node),
                )
                .into()
            }
        }
    })
}

fn project_site(
    reserved: &mut ReservedDeclarations<'_>,
    declaration: SurfaceDeclarationId,
    site: DeclarationSiteId,
) -> Result<(), HeaderError> {
    let source = reserved
        .sources
        .get(reserved.declarations[declaration.index()].source().index())
        .ok_or(HeaderError::MissingSource(
            reserved.declarations[declaration.index()].source(),
        ))?
        .syntax()
        .source();
    reserved
        .source_index
        .define_declaration_site_source(site, source)?;
    let origin = declaration_site_origin(reserved, declaration)?;
    reserved.source_index.insert(
        SemanticEntity::DeclarationSite(site),
        SourceRole::Declaration,
        origin,
    );
    Ok(())
}

fn project_entities(reserved: &mut ReservedDeclarations<'_>) -> Result<(), HeaderError> {
    for index in 0..reserved.declarations.len() {
        let id = SurfaceDeclarationId::from_index(index);
        let Some(entity) = reserved.entity(id) else {
            continue;
        };
        if let crate::ReservedEntity::NominalType(nominal) = entity
            && reserved.contracts.representative(id) == id
        {
            let private_representation = reserved.contracts.representation(id);
            let representation = private_representation.unwrap_or(id);
            let source = declaration_source(reserved, representation)?;
            reserved.source_index.define_nominal_representation_source(
                nominal,
                source,
                private_representation.is_some(),
            )?;
        }
        let role = if reserved.contracts.is_implementation(id) {
            SourceRole::Implementation
        } else {
            SourceRole::Declaration
        };
        let origin = entity_origin(reserved, id)?;
        let declaration = match entity {
            crate::ReservedEntity::BuiltinType(builtin) => Some(
                nocter_frontend_bindings::FrontendDeclaration::BuiltinType(builtin),
            ),
            crate::ReservedEntity::NominalType(id) => Some(
                nocter_frontend_bindings::FrontendDeclaration::NominalType(id),
            ),
            crate::ReservedEntity::Interface(id) => {
                Some(nocter_frontend_bindings::FrontendDeclaration::Interface(id))
            }
            crate::ReservedEntity::AssociatedType(id) => {
                Some(nocter_frontend_bindings::FrontendDeclaration::AssociatedType(id))
            }
            crate::ReservedEntity::Callable(id) => {
                Some(nocter_frontend_bindings::FrontendDeclaration::Callable(id))
            }
            _ => None,
        };
        if let (Some(declaration), nocter_syntax::SyntaxOrigin::Token(token)) =
            (declaration, origin.syntax())
        {
            reserved
                .source_index
                .insert_declaration(declaration, token, role, origin);
        } else {
            reserved
                .source_index
                .insert(entity.semantic_entity(), role, origin);
        }
    }
    Ok(())
}

fn declaration_source(
    reserved: &ReservedDeclarations<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SourceId, HeaderError> {
    let source = reserved
        .declarations
        .get(declaration.index())
        .ok_or(HeaderError::MissingDeclaration(declaration))?
        .source();
    reserved
        .sources
        .get(source.index())
        .map(|source| source.syntax().source())
        .ok_or(HeaderError::MissingSource(source))
}

fn declaration_site_origin(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
) -> Result<SourceOrigin, HeaderError> {
    let declaration = reserved.declarations[id.index()];
    let source = reserved
        .sources
        .get(declaration.source().index())
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    SourceOrigin::from_node(source.syntax(), declaration.node())
        .map_err(|_| HeaderError::InconsistentSource(source.syntax().source()))
}

fn entity_origin(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
) -> Result<SourceOrigin, HeaderError> {
    let declaration = reserved.declarations[id.index()];
    let source = reserved
        .sources
        .get(declaration.source().index())
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    match declaration.entity_origin() {
        SyntaxOrigin::Node(node) => SourceOrigin::from_node(source.syntax(), node)
            .map_err(|_| HeaderError::InconsistentSource(source.syntax().source())),
        SyntaxOrigin::Token(token) => SourceOrigin::from_token(source.syntax(), token)
            .map_err(|_| HeaderError::InconsistentSource(source.syntax().source())),
    }
}

#[cfg(test)]
mod tests;
