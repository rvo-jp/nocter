use std::collections::HashMap;

use nocter_compile_input::CompileUnitInput;
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::ModuleId;
use nocter_syntax::{NodeId, NodeKind};

use super::NameResolutionInternalError;

/// Converts discovery-owned module identities to semantic IDs through exact source projection.
///
/// This is the only bridge from block-use discovery data into lexical checking. It never compares
/// rendered package names or module path strings.
pub(super) fn block_import_targets(
    input: &CompileUnitInput<'_>,
    bindings: &FrontendBindings,
) -> Result<HashMap<NodeId, ModuleId>, NameResolutionInternalError> {
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
        let target = bindings
            .block_import(node)
            .ok_or(NameResolutionInternalError::MissingUseResolution(node))?;
        if targets.insert(node, target).is_some() {
            return Err(NameResolutionInternalError::DuplicateUseResolution(node));
        }
    }
    Ok(targets)
}
