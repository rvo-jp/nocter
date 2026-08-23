use std::collections::HashMap;

use nocter_compile_input::{CompileUnitInput, UseTargetInput};
use nocter_frontend_bindings::{FrontendBindings, FrontendBindingsBuilder, FrontendDeclaration};
use nocter_source_index::{
    DuplicateDocumentation, DuplicateSourceBinding, SemanticEntity, SourceIndex,
    SourceIndexBuilder, SourceOrigin, SourceRole, SyntaxOrigin,
};
use nocter_syntax::NodeKind;

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
        self.record_binding(entity, role, origin);
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

    pub(crate) fn finish(self) -> (SourceIndex, FrontendBindings) {
        (self.source_index.finish(), self.bindings.finish())
    }

    fn record_binding(&mut self, entity: SemanticEntity, role: SourceRole, origin: SourceOrigin) {
        if !matches!(role, SourceRole::Declaration | SourceRole::Implementation) {
            return;
        }
        match entity {
            SemanticEntity::Module(module) => {
                self.bindings.add_module_source(module, origin.source());
            }
            SemanticEntity::Body(body) => {
                if let Some(block) = origin.node() {
                    self.bindings.add_body_block(body, block);
                }
            }
            SemanticEntity::Parameter(parameter) => {
                if let SyntaxOrigin::Token(token) = origin.syntax() {
                    self.bindings.add_parameter_declaration(parameter, token);
                }
            }
            SemanticEntity::NominalType(id) => {
                self.record_declaration(origin, FrontendDeclaration::NominalType(id));
            }
            SemanticEntity::Interface(id) => {
                self.record_declaration(origin, FrontendDeclaration::Interface(id));
            }
            SemanticEntity::AssociatedType(id) => {
                self.record_declaration(origin, FrontendDeclaration::AssociatedType(id));
            }
            SemanticEntity::Callable(id) => {
                self.record_declaration(origin, FrontendDeclaration::Callable(id));
            }
            _ => {}
        }
    }

    fn record_declaration(&mut self, origin: SourceOrigin, declaration: FrontendDeclaration) {
        if let SyntaxOrigin::Token(token) = origin.syntax() {
            self.bindings.add_declaration(token, declaration);
        }
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
        let UseTargetInput::Module(identity) = resolution.target() else {
            return None;
        };
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
