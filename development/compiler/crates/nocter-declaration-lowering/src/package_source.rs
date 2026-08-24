use std::collections::BTreeSet;

use nocter_source::SourceId;
use nocter_syntax::{NodeKind, SyntaxElement};

use crate::ModuleInput;
use crate::topology::LoweringError;
use crate::topology_violation::TopologyViolation;

/// Ensures that package directives occur only in the source selected as that package's
/// declaration and root module source.
pub(crate) fn validate_package_directive_ownership(
    package_declaration_sources: &BTreeSet<SourceId>,
    modules: &[&ModuleInput<'_>],
) -> Result<(), LoweringError> {
    for source in modules.iter().flat_map(|module| module.sources()) {
        if package_declaration_sources.contains(&source.syntax().source()) {
            continue;
        }
        for element in source.syntax().children(source.syntax().root_id()) {
            let SyntaxElement::Node(node) = element else {
                continue;
            };
            if source
                .syntax()
                .node(*node)
                .is_some_and(|node| node.kind() == NodeKind::PackageDirective)
            {
                return Err(TopologyViolation::package_directive_outside_root(*node).into());
            }
        }
    }
    Ok(())
}
