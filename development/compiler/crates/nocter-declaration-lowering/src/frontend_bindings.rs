use std::collections::HashMap;

use nocter_compile_input::{CompileUnitInput, UseTargetInput};
use nocter_declarations::DeclarationGraph;
use nocter_frontend_bindings::{FrontendBindings, FrontendBindingsBuilder, FrontendDeclaration};
use nocter_model::ModuleId;
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole, SyntaxOrigin};
use nocter_syntax::NodeKind;

pub(crate) fn build(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    source_index: &SourceIndex,
) -> FrontendBindings {
    let mut result = FrontendBindingsBuilder::new();
    let mut modules_by_source = HashMap::<SourceId, ModuleId>::new();
    for (module, _) in graph.modules().iter() {
        for binding in source_index.bindings_for(SemanticEntity::Module(module)) {
            if matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            ) {
                let source = binding.origin().source();
                result.add_module_source(module, source);
                modules_by_source.insert(source, module);
            }
        }
    }
    for (body, _) in graph.declarations().bodies().iter() {
        for binding in source_index.bindings_for(SemanticEntity::Body(body)) {
            if matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            ) && let Some(block) = binding.origin().node()
            {
                result.add_body_block(body, block);
            }
        }
    }
    for (parameter, _) in graph.declarations().parameters().iter() {
        for binding in source_index.bindings_for(SemanticEntity::Parameter(parameter)) {
            if binding.role() == SourceRole::Declaration
                && let SyntaxOrigin::Token(token) = binding.origin().syntax()
            {
                result.add_parameter_declaration(parameter, token);
            }
        }
    }
    add_declarations(graph, source_index, &mut result);

    let modules_by_identity = input
        .modules()
        .iter()
        .filter_map(|module| {
            module
                .sources()
                .first()
                .and_then(|source| modules_by_source.get(&source.syntax().source()))
                .copied()
                .map(|id| (module.identity(), id))
        })
        .collect::<HashMap<_, _>>();
    for resolution in input.use_resolutions() {
        let node = resolution.declaration();
        let is_block_import = input.modules().iter().any(|module| {
            module.sources().iter().any(|source| {
                let tree = source.syntax();
                tree.source() == node.source()
                    && tree.node(node).map(nocter_syntax::SyntaxNode::kind)
                        == Some(NodeKind::BlockUseDeclaration)
            })
        });
        if !is_block_import {
            continue;
        }
        if let UseTargetInput::Module(identity) = resolution.target()
            && let Some(module) = modules_by_identity.get(identity)
        {
            result.set_block_import(node, *module);
        }
    }
    result.finish()
}

fn add_declarations(
    graph: &DeclarationGraph,
    source_index: &SourceIndex,
    result: &mut FrontendBindingsBuilder,
) {
    for (id, _) in graph.declarations().nominal_types().iter() {
        add_declaration(
            source_index,
            SemanticEntity::NominalType(id),
            FrontendDeclaration::NominalType(id),
            result,
        );
    }
    for (id, _) in graph.declarations().interfaces().iter() {
        add_declaration(
            source_index,
            SemanticEntity::Interface(id),
            FrontendDeclaration::Interface(id),
            result,
        );
    }
    for (id, _) in graph.declarations().associated_types().iter() {
        add_declaration(
            source_index,
            SemanticEntity::AssociatedType(id),
            FrontendDeclaration::AssociatedType(id),
            result,
        );
    }
    for (id, _) in graph.declarations().callables().iter() {
        add_declaration(
            source_index,
            SemanticEntity::Callable(id),
            FrontendDeclaration::Callable(id),
            result,
        );
    }
}

fn add_declaration(
    source_index: &SourceIndex,
    entity: SemanticEntity,
    declaration: FrontendDeclaration,
    result: &mut FrontendBindingsBuilder,
) {
    for binding in source_index.bindings_for(entity) {
        if binding.role() == SourceRole::Declaration
            && let SyntaxOrigin::Token(token) = binding.origin().syntax()
        {
            result.add_declaration(token, declaration);
        }
    }
}
