use std::collections::HashSet;

use nocter_frontend_bindings::{
    AssociatedProjectionUse, DuplicateBlockImport, FrontendBindings, FrontendBindingsBuilder,
    FrontendDeclaration, SourceOwnershipError,
};
use nocter_model::{
    AssociatedTypeId, BodyId, DeclarationSiteId, ModuleId, NominalTypeId, ParameterId, TypeId,
};
use nocter_source::SourceId;
use nocter_source_index::{
    DuplicateDocumentation, DuplicateSourceBinding, SemanticEntity, SourceIndex,
    SourceIndexBuilder, SourceOrigin, SourceRole,
};
use nocter_syntax::{NodeId, SyntaxToken};

/// One lowering-owned write path that independently emits semantic checking bindings and the
/// presentation index. Neither completed product is reconstructed from the other.
#[derive(Debug, Default)]
pub(crate) struct FrontendProjectionBuilder {
    source_index: SourceIndexBuilder,
    bindings: FrontendBindingsBuilder,
    associated_references: HashSet<(AssociatedTypeId, nocter_syntax::SyntaxOrigin)>,
}

impl FrontendProjectionBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) const fn len(&self) -> usize {
        self.source_index.len()
    }

    pub(crate) fn insert(
        &mut self,
        entity: SemanticEntity,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.source_index.insert(entity, role, origin)
    }

    pub(crate) fn insert_module_source(
        &mut self,
        module: ModuleId,
        source: SourceId,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), ModuleSourceProjectionError> {
        self.bindings.add_module_source(module, source)?;
        self.source_index
            .insert(SemanticEntity::Module(module), role, origin)?;
        Ok(())
    }

    pub(crate) fn insert_body(
        &mut self,
        body: BodyId,
        block: NodeId,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.bindings.add_body_block(body, block);
        self.source_index
            .insert(SemanticEntity::Body(body), role, origin)
    }

    pub(crate) fn insert_parameter(
        &mut self,
        parameter: ParameterId,
        declaration: SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.bindings
            .add_parameter_declaration(parameter, declaration);
        self.source_index
            .insert(SemanticEntity::Parameter(parameter), role, origin)
    }

    pub(crate) fn insert_declaration(
        &mut self,
        declaration: FrontendDeclaration,
        token: SyntaxToken,
        role: SourceRole,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.bindings.add_declaration(token, declaration);
        let entity = match declaration {
            FrontendDeclaration::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
            FrontendDeclaration::NominalType(id) => SemanticEntity::NominalType(id),
            FrontendDeclaration::Interface(id) => SemanticEntity::Interface(id),
            FrontendDeclaration::AssociatedType(id) => SemanticEntity::AssociatedType(id),
            FrontendDeclaration::Callable(id) => SemanticEntity::Callable(id),
        };
        self.source_index.insert(entity, role, origin)
    }

    pub(crate) fn insert_associated_projection_use(
        &mut self,
        base: TypeId,
        associated: AssociatedTypeId,
        syntax: nocter_syntax::SyntaxOrigin,
        origin: SourceOrigin,
    ) -> Result<(), DuplicateSourceBinding> {
        self.bindings
            .add_associated_projection_use(AssociatedProjectionUse::new(base, associated, syntax));
        if self.associated_references.insert((associated, syntax)) {
            self.source_index.insert(
                SemanticEntity::AssociatedType(associated),
                SourceRole::Reference,
                origin,
            )?;
        }
        Ok(())
    }

    pub(crate) fn insert_block_import(
        &mut self,
        declaration: NodeId,
        target: ModuleId,
    ) -> Result<(), DuplicateBlockImport> {
        self.bindings.add_block_import(declaration, target)
    }

    pub(crate) fn define_declaration_site_source(
        &mut self,
        site: DeclarationSiteId,
        source: SourceId,
    ) {
        self.bindings.define_declaration_site_source(site, source);
    }

    pub(crate) fn define_nominal_representation_source(
        &mut self,
        nominal: NominalTypeId,
        source: SourceId,
        contract_private: bool,
    ) {
        self.bindings
            .define_nominal_representation_source(nominal, source, contract_private);
    }

    pub(crate) fn insert_documentation(
        &mut self,
        entity: SemanticEntity,
        markdown: impl Into<Box<str>>,
    ) -> Result<(), DuplicateDocumentation> {
        self.source_index.insert_documentation(entity, markdown)
    }

    pub(crate) fn insert_occurrence_documentation(
        &mut self,
        entity: SemanticEntity,
        origin: SourceOrigin,
        markdown: impl Into<Box<str>>,
    ) -> Result<(), DuplicateDocumentation> {
        self.source_index
            .insert_occurrence_documentation(entity, origin, markdown)
    }

    pub(crate) fn define_source_namespace(
        &mut self,
        source: SourceId,
        authored: impl IntoIterator<Item = (nocter_model::Symbol, nocter_declarations::ExportedEntity)>,
        fallback: impl IntoIterator<Item = (nocter_model::Symbol, nocter_declarations::ExportedEntity)>,
    ) {
        let authored = authored.into_iter().collect::<Vec<_>>();
        let fallback = fallback.into_iter().collect::<Vec<_>>();
        self.bindings.define_source_namespace(
            source,
            authored.iter().copied(),
            fallback.iter().copied(),
        );
        self.source_index.define_visible_names(
            source,
            authored
                .into_iter()
                .chain(fallback)
                .map(|(name, entity)| (name, source_entity(entity))),
        );
    }

    pub(crate) fn define_source_access(
        &mut self,
        source: SourceId,
        directly_visible: impl IntoIterator<Item = SourceId>,
    ) {
        self.bindings.define_source_access(source, directly_visible);
    }

    pub(crate) fn finish(self) -> (SourceIndex, FrontendBindings) {
        (self.source_index.finish(), self.bindings.finish())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModuleSourceProjectionError {
    DuplicateBinding(DuplicateSourceBinding),
    SourceOwnership(SourceOwnershipError),
}

impl From<DuplicateSourceBinding> for ModuleSourceProjectionError {
    fn from(error: DuplicateSourceBinding) -> Self {
        Self::DuplicateBinding(error)
    }
}

impl From<SourceOwnershipError> for ModuleSourceProjectionError {
    fn from(error: SourceOwnershipError) -> Self {
        Self::SourceOwnership(error)
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
