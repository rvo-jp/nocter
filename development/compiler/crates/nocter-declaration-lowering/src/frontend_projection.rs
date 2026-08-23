use std::collections::HashMap;

use nocter_compile_input::CompileUnitInput;
use nocter_frontend_bindings::{FrontendBindings, FrontendBindingsBuilder, FrontendDeclaration};
use nocter_model::{BodyId, ModuleId, ParameterId};
use nocter_source::SourceId;
use nocter_source_index::{
    DuplicateDocumentation, DuplicateSourceBinding, SemanticEntity, SourceIndex,
    SourceIndexBuilder, SourceOrigin, SourceRole,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxToken};

/// One lowering-owned write path that independently emits semantic checking bindings and the
/// presentation index. Neither completed product is reconstructed from the other.
#[derive(Debug, Default)]
pub(crate) struct FrontendProjectionBuilder {
    source_index: SourceIndexBuilder,
    bindings: FrontendBindingsBuilder,
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
    ) -> Result<(), DuplicateSourceBinding> {
        self.bindings.add_module_source(module, source);
        self.source_index
            .insert(SemanticEntity::Module(module), role, origin)
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
            FrontendDeclaration::NominalType(id) => SemanticEntity::NominalType(id),
            FrontendDeclaration::Interface(id) => SemanticEntity::Interface(id),
            FrontendDeclaration::AssociatedType(id) => SemanticEntity::AssociatedType(id),
            FrontendDeclaration::Callable(id) => SemanticEntity::Callable(id),
        };
        self.source_index.insert(entity, role, origin)
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

    pub(crate) fn finish(self) -> (SourceIndex, FrontendBindings) {
        (self.source_index.finish(), self.bindings.finish())
    }
}

const fn source_entity(entity: nocter_declarations::ExportedEntity) -> SemanticEntity {
    match entity {
        nocter_declarations::ExportedEntity::Module(id) => SemanticEntity::Module(id),
        nocter_declarations::ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        nocter_declarations::ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        nocter_declarations::ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        nocter_declarations::ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}

pub(crate) fn add_block_imports(
    input: &CompileUnitInput<'_>,
    bindings: FrontendBindings,
) -> FrontendBindings {
    let modules_by_identity = input
        .modules()
        .iter()
        .filter_map(|module| {
            module.sources().iter().find_map(|source| {
                let source = source.syntax().source();
                bindings
                    .module_for_source(source)
                    .map(|id| (module.identity(), id))
            })
        })
        .collect::<HashMap<_, _>>();
    let imports = input.use_resolutions().iter().filter_map(|resolution| {
        let node = resolution.declaration();
        let is_block = input.modules().iter().any(|module| {
            module.sources().iter().any(|source| {
                let tree = source.syntax();
                tree.source() == node.source()
                    && tree.node(node).map(nocter_syntax::SyntaxNode::kind)
                        == Some(NodeKind::BlockUseDeclaration)
            })
        });
        let identity = resolution.target_module();
        is_block
            .then(|| {
                modules_by_identity
                    .get(identity)
                    .copied()
                    .map(|module| (node, module))
            })
            .flatten()
    });
    bindings.with_block_imports(imports)
}
