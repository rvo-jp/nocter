use std::collections::HashMap;

use nocter_compile_input::{CompileUnitInput, UseTargetInput};
use nocter_declarations::DeclarationGraph;
use nocter_model::ModuleId;
use nocter_source_index::{SemanticEntity, SourceIndex, SourceRole};
use nocter_syntax::{NodeId, NodeKind};

use super::NameResolutionInternalError;

/// Converts discovery-owned module identities to semantic IDs through exact source projection.
///
/// This is the only bridge from block-use discovery data into lexical checking. It never compares
/// rendered package names or module path strings.
pub(super) fn block_import_targets(
    input: &CompileUnitInput<'_>,
    graph: &DeclarationGraph,
    source_index: &SourceIndex,
) -> Result<HashMap<NodeId, ModuleId>, NameResolutionInternalError> {
    let mut modules_by_source = HashMap::new();
    for (module, _) in graph.modules().iter() {
        let mut found = false;
        for binding in source_index.bindings_for(SemanticEntity::Module(module)) {
            if !matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            ) {
                continue;
            }
            found = true;
            if modules_by_source
                .insert(binding.origin().source(), module)
                .is_some()
            {
                return Err(NameResolutionInternalError::DuplicateModuleSource(
                    binding.origin().source(),
                ));
            }
        }
        if !found {
            return Err(NameResolutionInternalError::MissingModuleSource(module));
        }
    }

    let mut modules_by_identity = HashMap::new();
    for module in input.modules() {
        let id = module
            .sources()
            .first()
            .and_then(|source| modules_by_source.get(&source.syntax().source()))
            .copied()
            .ok_or_else(|| {
                NameResolutionInternalError::UnknownInputModule(module.identity().clone())
            })?;
        modules_by_identity.insert(module.identity().clone(), id);
    }

    let mut targets = HashMap::new();
    for resolution in input.use_resolutions() {
        let node = resolution.declaration();
        let Some(tree) = input.modules().iter().find_map(|module| {
            module
                .sources()
                .iter()
                .find(|source| source.syntax().source() == node.source())
                .map(nocter_compile_input::ModuleSourceInput::syntax)
        }) else {
            continue;
        };
        if tree.node(node).map(nocter_syntax::SyntaxNode::kind)
            != Some(NodeKind::BlockUseDeclaration)
        {
            continue;
        }
        let UseTargetInput::Module(identity) = resolution.target() else {
            return Err(NameResolutionInternalError::InvalidBlockImportTarget(node));
        };
        let target = modules_by_identity
            .get(identity)
            .copied()
            .ok_or_else(|| NameResolutionInternalError::UnknownInputModule(identity.clone()))?;
        if targets.insert(node, target).is_some() {
            return Err(NameResolutionInternalError::DuplicateUseResolution(node));
        }
    }
    Ok(targets)
}
