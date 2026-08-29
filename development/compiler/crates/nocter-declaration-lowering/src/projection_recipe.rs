use std::collections::HashMap;
use std::fmt;

use nocter_frontend_bindings::{
    AssociatedProjectionUse, FrontendBindingDefinitionError, FrontendBindings,
    FrontendBindingsBuilder, FrontendDeclaration, SourceOwnershipError,
};
use nocter_model::{
    AssociatedTypeId, BodyId, DeclarationSiteId, ModuleId, NominalTypeId, ParameterId, Symbol,
    TypeId,
};
use nocter_source::{SourceId, SourceMap};
use nocter_source_index::{
    SemanticEntity, SourceIndex, SourceIndexBuilder, SourceOrigin, SourceRole,
};
use nocter_syntax::{
    DeclarationSyntaxLocator, DeclarationSyntaxProjection, SyntaxOrigin, project_declaration_syntax,
};

use crate::{ModuleIdentity, ModuleSourceKind, SurfaceSource, SurfaceSourceId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SurfaceOrigin {
    source: SurfaceSourceId,
    syntax: DeclarationSyntaxLocator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DocumentationSite {
    File(SourceId),
    Node(nocter_syntax::NodeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StableDocumentationSite {
    File(SurfaceSourceId),
    Node(SurfaceOrigin),
}

#[derive(Clone, Debug)]
enum ProjectionOperation {
    Binding {
        entity: SemanticEntity,
        role: SourceRole,
        origin: SurfaceOrigin,
    },
    ModuleSource {
        module: ModuleId,
        source: SurfaceSourceId,
        role: SourceRole,
        origin: SurfaceOrigin,
    },
    Body {
        body: BodyId,
        block: SurfaceOrigin,
        role: SourceRole,
        origin: SurfaceOrigin,
    },
    Parameter {
        parameter: ParameterId,
        declaration: SurfaceOrigin,
        role: SourceRole,
        origin: SurfaceOrigin,
    },
    Declaration {
        declaration: FrontendDeclaration,
        token: SurfaceOrigin,
        role: SourceRole,
        origin: SurfaceOrigin,
    },
    AssociatedProjection {
        base: TypeId,
        associated: AssociatedTypeId,
        syntax: SurfaceOrigin,
        origin: SurfaceOrigin,
    },
    DeclarationSiteSource {
        site: DeclarationSiteId,
        source: SurfaceSourceId,
    },
    NominalRepresentationSource {
        nominal: NominalTypeId,
        source: SurfaceSourceId,
        contract_private: bool,
    },
    Documentation {
        entity: SemanticEntity,
        site: StableDocumentationSite,
    },
    OccurrenceDocumentation {
        entity: SemanticEntity,
        occurrence: SurfaceOrigin,
        site: SurfaceOrigin,
    },
    SourceNamespace {
        source: SurfaceSourceId,
        authored: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
        fallback: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
    },
    SourceAccess {
        source: SurfaceSourceId,
        directly_visible: Box<[SurfaceSourceId]>,
    },
}

/// Source-neutral recipe emitted with a declaration program.
///
/// Semantic identities and stable syntax locators are retained together. Physical source and
/// syntax-arena identities enter only while materializing this recipe for one current generation.
#[derive(Clone, Debug)]
pub struct FrontendProjectionRecipe {
    sources: Box<[ProjectionSourceKey]>,
    operations: Box<[ProjectionOperation]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectionSourceKey {
    module: ModuleIdentity,
    canonical_path: Box<str>,
    kind: ModuleSourceKind,
}

impl ProjectionSourceKey {
    fn new(source: &SurfaceSource<'_>) -> Self {
        Self {
            module: source.module().clone(),
            canonical_path: source.canonical_path().into(),
            kind: source.kind(),
        }
    }
}

impl FrontendProjectionRecipe {
    // Keeping the closed operation interpreter together makes omissions auditable when a new
    // projection fact is added. Splitting this match would create parallel partial interpreters.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn materialize(
        &self,
        source_map: &SourceMap,
        sources: &[SurfaceSource<'_>],
        block_imports: &HashMap<nocter_syntax::NodeId, ModuleId>,
    ) -> Result<(SourceIndex, FrontendBindings), ProjectionRecipeError> {
        if self.sources.len() != sources.len()
            || self
                .sources
                .iter()
                .zip(sources)
                .any(|(expected, current)| *expected != ProjectionSourceKey::new(current))
        {
            return Err(ProjectionRecipeError::SourceDomainMismatch);
        }
        let domain = ProjectionSyntaxDomain::new(source_map, sources)?;
        let mut index = SourceIndexBuilder::new();
        let mut bindings = FrontendBindingsBuilder::new();
        for operation in &self.operations {
            match operation {
                ProjectionOperation::Binding {
                    entity,
                    role,
                    origin,
                } => index.insert(*entity, *role, domain.source_origin(*origin)?),
                ProjectionOperation::ModuleSource {
                    module,
                    source,
                    role,
                    origin,
                } => {
                    bindings.add_module_source(*module, domain.source_id(*source)?)?;
                    index.insert(
                        SemanticEntity::Module(*module),
                        *role,
                        domain.source_origin(*origin)?,
                    );
                }
                ProjectionOperation::Body {
                    body,
                    block,
                    role,
                    origin,
                } => {
                    let block = match domain.syntax(*block)? {
                        SyntaxOrigin::Node(node) => node,
                        SyntaxOrigin::Token(_) => {
                            return Err(ProjectionRecipeError::ExpectedNode);
                        }
                    };
                    bindings.add_body_block(*body, block);
                    index.insert(
                        SemanticEntity::Body(*body),
                        *role,
                        domain.source_origin(*origin)?,
                    );
                }
                ProjectionOperation::Parameter {
                    parameter,
                    declaration,
                    role,
                    origin,
                } => {
                    let declaration = match domain.syntax(*declaration)? {
                        SyntaxOrigin::Token(token) => token,
                        SyntaxOrigin::Node(_) => {
                            return Err(ProjectionRecipeError::ExpectedToken);
                        }
                    };
                    bindings.add_parameter_declaration(*parameter, declaration);
                    index.insert(
                        SemanticEntity::Parameter(*parameter),
                        *role,
                        domain.source_origin(*origin)?,
                    );
                }
                ProjectionOperation::Declaration {
                    declaration,
                    token,
                    role,
                    origin,
                } => {
                    let token = match domain.syntax(*token)? {
                        SyntaxOrigin::Token(token) => token,
                        SyntaxOrigin::Node(_) => {
                            return Err(ProjectionRecipeError::ExpectedToken);
                        }
                    };
                    bindings.add_declaration(token, *declaration);
                    index.insert(
                        declaration_entity(*declaration),
                        *role,
                        domain.source_origin(*origin)?,
                    );
                }
                ProjectionOperation::AssociatedProjection {
                    base,
                    associated,
                    syntax,
                    origin,
                } => {
                    bindings.add_associated_projection_use(AssociatedProjectionUse::new(
                        *base,
                        *associated,
                        domain.syntax(*syntax)?,
                    ));
                    index.insert(
                        SemanticEntity::AssociatedType(*associated),
                        SourceRole::Reference,
                        domain.source_origin(*origin)?,
                    );
                }
                ProjectionOperation::DeclarationSiteSource { site, source } => {
                    bindings.define_declaration_site_source(*site, domain.source_id(*source)?)?;
                }
                ProjectionOperation::NominalRepresentationSource {
                    nominal,
                    source,
                    contract_private,
                } => bindings.define_nominal_representation_source(
                    *nominal,
                    domain.source_id(*source)?,
                    *contract_private,
                )?,
                ProjectionOperation::Documentation { entity, site } => {
                    if let Some(markdown) = domain.documentation(*site)? {
                        index.insert_documentation(*entity, markdown);
                    }
                }
                ProjectionOperation::OccurrenceDocumentation {
                    entity,
                    occurrence,
                    site,
                } => {
                    if let Some(markdown) =
                        domain.documentation(StableDocumentationSite::Node(*site))?
                    {
                        index.insert_occurrence_documentation(
                            *entity,
                            domain.source_origin(*occurrence)?,
                            markdown,
                        );
                    }
                }
                ProjectionOperation::SourceNamespace {
                    source,
                    authored,
                    fallback,
                } => {
                    let source = domain.source_id(*source)?;
                    bindings.define_source_namespace(
                        source,
                        authored.iter().copied(),
                        fallback.iter().copied(),
                    )?;
                    index.define_visible_names(
                        source,
                        authored
                            .iter()
                            .chain(fallback)
                            .map(|(name, entity)| (*name, source_entity(*entity))),
                    );
                }
                ProjectionOperation::SourceAccess {
                    source,
                    directly_visible,
                } => bindings.define_source_access(
                    domain.source_id(*source)?,
                    directly_visible
                        .iter()
                        .map(|source| domain.source_id(*source))
                        .collect::<Result<Vec<_>, _>>()?,
                )?,
            }
        }
        for (declaration, target) in block_imports {
            bindings
                .add_block_import(*declaration, *target)
                .map_err(|_| ProjectionRecipeError::DuplicateBlockImport(*declaration))?;
        }
        Ok((index.finish(), bindings.finish()))
    }
}

#[derive(Debug)]
pub(crate) struct ProjectionRecipeBuilder {
    sources: Box<[ProjectionSourceKey]>,
    locators: HashMap<SourceId, (SurfaceSourceId, DeclarationSyntaxProjection)>,
    operations: Vec<ProjectionOperation>,
}

impl ProjectionRecipeBuilder {
    pub(crate) fn new(
        source_map: &SourceMap,
        sources: &[SurfaceSource<'_>],
    ) -> Result<Self, ProjectionRecipeError> {
        let domain = ProjectionSyntaxDomain::new(source_map, sources)?;
        Ok(Self {
            sources: sources
                .iter()
                .map(ProjectionSourceKey::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            locators: domain
                .entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| {
                    (
                        entry.source,
                        (SurfaceSourceId::from_index(index), entry.projection),
                    )
                })
                .collect(),
            operations: Vec::new(),
        })
    }

    fn source(&self, source: SourceId) -> Result<SurfaceSourceId, ProjectionRecipeError> {
        self.locators
            .get(&source)
            .map(|entry| entry.0)
            .ok_or(ProjectionRecipeError::UnknownSource(source))
    }

    fn origin(&self, origin: SyntaxOrigin) -> Result<SurfaceOrigin, ProjectionRecipeError> {
        let source = match origin {
            SyntaxOrigin::Node(node) => node.source(),
            SyntaxOrigin::Token(token) => token.source(),
        };
        let (source, projection) = self
            .locators
            .get(&source)
            .ok_or(ProjectionRecipeError::UnknownSource(source))?;
        let syntax = projection
            .locate(origin)
            .ok_or(ProjectionRecipeError::OutsideDeclarationSurface(origin))?;
        Ok(SurfaceOrigin {
            source: *source,
            syntax,
        })
    }

    fn source_origin(&self, origin: SourceOrigin) -> Result<SurfaceOrigin, ProjectionRecipeError> {
        self.origin(origin.syntax())
    }

    pub(crate) fn binding(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let origin = self.source_origin(origin)?;
        self.operations.push(ProjectionOperation::Binding {
            entity,
            role,
            origin,
        });
        Ok(())
    }

    pub(crate) fn module_source(
        &mut self,
        module: ModuleId,
        source: SourceId,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let source = self.source(source)?;
        let origin = self.source_origin(origin)?;
        self.operations.push(ProjectionOperation::ModuleSource {
            module,
            source,
            role,
            origin,
        });
        Ok(())
    }

    pub(crate) fn body(
        &mut self,
        body: BodyId,
        block: nocter_syntax::NodeId,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let block = self.origin(SyntaxOrigin::Node(block))?;
        let origin = self.source_origin(origin)?;
        self.operations.push(ProjectionOperation::Body {
            body,
            block,
            role,
            origin,
        });
        Ok(())
    }

    pub(crate) fn parameter(
        &mut self,
        parameter: ParameterId,
        declaration: nocter_syntax::SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let declaration = self.origin(SyntaxOrigin::Token(declaration))?;
        let origin = self.source_origin(origin)?;
        self.operations.push(ProjectionOperation::Parameter {
            parameter,
            declaration,
            role,
            origin,
        });
        Ok(())
    }

    pub(crate) fn declaration(
        &mut self,
        declaration: FrontendDeclaration,
        token: nocter_syntax::SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let token = self.origin(SyntaxOrigin::Token(token))?;
        let origin = self.source_origin(origin)?;
        self.operations.push(ProjectionOperation::Declaration {
            declaration,
            token,
            role,
            origin,
        });
        Ok(())
    }

    pub(crate) fn associated_projection(
        &mut self,
        base: TypeId,
        associated: AssociatedTypeId,
        syntax: SyntaxOrigin,
        origin: SourceOrigin,
    ) -> Result<(), ProjectionRecipeError> {
        let syntax = self.origin(syntax)?;
        let origin = self.source_origin(origin)?;
        self.operations
            .push(ProjectionOperation::AssociatedProjection {
                base,
                associated,
                syntax,
                origin,
            });
        Ok(())
    }

    pub(crate) fn declaration_site_source(
        &mut self,
        site: DeclarationSiteId,
        source: SourceId,
    ) -> Result<(), ProjectionRecipeError> {
        let source = self.source(source)?;
        self.operations
            .push(ProjectionOperation::DeclarationSiteSource { site, source });
        Ok(())
    }

    pub(crate) fn nominal_representation_source(
        &mut self,
        nominal: NominalTypeId,
        source: SourceId,
        contract_private: bool,
    ) -> Result<(), ProjectionRecipeError> {
        let source = self.source(source)?;
        self.operations
            .push(ProjectionOperation::NominalRepresentationSource {
                nominal,
                source,
                contract_private,
            });
        Ok(())
    }

    pub(crate) fn documentation(
        &mut self,
        entity: SemanticEntity,
        site: DocumentationSite,
    ) -> Result<(), ProjectionRecipeError> {
        let site = match site {
            DocumentationSite::File(source) => StableDocumentationSite::File(self.source(source)?),
            DocumentationSite::Node(node) => {
                StableDocumentationSite::Node(self.origin(SyntaxOrigin::Node(node))?)
            }
        };
        self.operations
            .push(ProjectionOperation::Documentation { entity, site });
        Ok(())
    }

    pub(crate) fn occurrence_documentation(
        &mut self,
        entity: SemanticEntity,
        occurrence: SourceOrigin,
        documented_node: nocter_syntax::NodeId,
    ) -> Result<(), ProjectionRecipeError> {
        let occurrence = self.source_origin(occurrence)?;
        let site = self.origin(SyntaxOrigin::Node(documented_node))?;
        self.operations
            .push(ProjectionOperation::OccurrenceDocumentation {
                entity,
                occurrence,
                site,
            });
        Ok(())
    }

    pub(crate) fn source_namespace(
        &mut self,
        source: SourceId,
        authored: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
        fallback: Box<[(Symbol, nocter_declarations::ExportedEntity)]>,
    ) -> Result<(), ProjectionRecipeError> {
        let source = self.source(source)?;
        self.operations.push(ProjectionOperation::SourceNamespace {
            source,
            authored,
            fallback,
        });
        Ok(())
    }

    pub(crate) fn source_access(
        &mut self,
        source: SourceId,
        directly_visible: &[SourceId],
    ) -> Result<(), ProjectionRecipeError> {
        let source = self.source(source)?;
        let directly_visible = directly_visible
            .iter()
            .map(|source| self.source(*source))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice();
        self.operations.push(ProjectionOperation::SourceAccess {
            source,
            directly_visible,
        });
        Ok(())
    }

    pub(crate) fn finish(self) -> FrontendProjectionRecipe {
        FrontendProjectionRecipe {
            sources: self.sources,
            operations: self.operations.into_boxed_slice(),
        }
    }
}

struct ProjectionSyntaxEntry<'syntax> {
    source: SourceId,
    syntax: &'syntax nocter_syntax::SyntaxTree,
    projection: DeclarationSyntaxProjection,
}

struct ProjectionSyntaxDomain<'syntax> {
    entries: Vec<ProjectionSyntaxEntry<'syntax>>,
}

impl<'syntax> ProjectionSyntaxDomain<'syntax> {
    fn new(
        source_map: &'syntax SourceMap,
        sources: &'syntax [SurfaceSource<'syntax>],
    ) -> Result<Self, ProjectionRecipeError> {
        let entries = sources
            .iter()
            .map(|source| {
                let syntax = source.syntax();
                let file = source_map
                    .get(syntax.source())
                    .ok_or(ProjectionRecipeError::UnknownSource(syntax.source()))?;
                let projection = project_declaration_syntax(syntax, file)
                    .ok_or(ProjectionRecipeError::MismatchedSource(syntax.source()))?;
                Ok::<_, ProjectionRecipeError>(ProjectionSyntaxEntry {
                    source: syntax.source(),
                    syntax,
                    projection,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { entries })
    }

    fn entry(
        &self,
        source: SurfaceSourceId,
    ) -> Result<&ProjectionSyntaxEntry<'syntax>, ProjectionRecipeError> {
        self.entries
            .get(source.index())
            .ok_or(ProjectionRecipeError::UnknownSurfaceSource(source))
    }

    fn source_id(&self, source: SurfaceSourceId) -> Result<SourceId, ProjectionRecipeError> {
        Ok(self.entry(source)?.source)
    }

    fn syntax(&self, origin: SurfaceOrigin) -> Result<SyntaxOrigin, ProjectionRecipeError> {
        self.entry(origin.source)?
            .projection
            .resolve(origin.syntax)
            .ok_or(ProjectionRecipeError::UnresolvedLocator)
    }

    fn source_origin(&self, origin: SurfaceOrigin) -> Result<SourceOrigin, ProjectionRecipeError> {
        let entry = self.entry(origin.source)?;
        match self.syntax(origin)? {
            SyntaxOrigin::Node(node) => SourceOrigin::from_node(entry.syntax, node)
                .map_err(|_| ProjectionRecipeError::UnresolvedLocator),
            SyntaxOrigin::Token(token) => SourceOrigin::from_token(entry.syntax, token)
                .map_err(|_| ProjectionRecipeError::UnresolvedLocator),
        }
    }

    fn documentation(
        &self,
        site: StableDocumentationSite,
    ) -> Result<Option<&'syntax str>, ProjectionRecipeError> {
        match site {
            StableDocumentationSite::File(source) => {
                Ok(self.entry(source)?.syntax.file_documentation())
            }
            StableDocumentationSite::Node(origin) => {
                let entry = self.entry(origin.source)?;
                let node = match self.syntax(origin)? {
                    SyntaxOrigin::Node(node) => node,
                    SyntaxOrigin::Token(_) => {
                        return Err(ProjectionRecipeError::ExpectedNode);
                    }
                };
                Ok(entry.syntax.documentation(node))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionRecipeError {
    SourceDomainMismatch,
    UnknownSource(SourceId),
    UnknownSurfaceSource(SurfaceSourceId),
    MismatchedSource(SourceId),
    OutsideDeclarationSurface(SyntaxOrigin),
    UnresolvedLocator,
    ExpectedNode,
    ExpectedToken,
    DuplicateBlockImport(nocter_syntax::NodeId),
    SourceOwnership(SourceOwnershipError),
    FrontendBinding(FrontendBindingDefinitionError),
}

impl fmt::Display for ProjectionRecipeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid frontend projection recipe: {self:?}")
    }
}

impl std::error::Error for ProjectionRecipeError {}

impl From<SourceOwnershipError> for ProjectionRecipeError {
    fn from(error: SourceOwnershipError) -> Self {
        Self::SourceOwnership(error)
    }
}

impl From<FrontendBindingDefinitionError> for ProjectionRecipeError {
    fn from(error: FrontendBindingDefinitionError) -> Self {
        Self::FrontendBinding(error)
    }
}

const fn declaration_entity(declaration: FrontendDeclaration) -> SemanticEntity {
    match declaration {
        FrontendDeclaration::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
        FrontendDeclaration::NominalType(id) => SemanticEntity::NominalType(id),
        FrontendDeclaration::Interface(id) => SemanticEntity::Interface(id),
        FrontendDeclaration::AssociatedType(id) => SemanticEntity::AssociatedType(id),
        FrontendDeclaration::Callable(id) => SemanticEntity::Callable(id),
    }
}

const fn source_entity(entity: nocter_declarations::ExportedEntity) -> SemanticEntity {
    match entity {
        nocter_declarations::ExportedEntity::BuiltinType(builtin) => {
            SemanticEntity::BuiltinType(builtin)
        }
        nocter_declarations::ExportedEntity::Module(id) => SemanticEntity::Module(id),
        nocter_declarations::ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        nocter_declarations::ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        nocter_declarations::ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        nocter_declarations::ExportedEntity::Constant(id) => SemanticEntity::Constant(id),
        nocter_declarations::ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}
