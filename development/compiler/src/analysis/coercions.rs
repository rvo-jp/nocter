//! Editor presentation for resolver-owned borrow coercion surfaces.

use crate::analysis::presentation::type_owner_presentation_label;
use crate::resolve::{ResolveOutput, TypeSymbol};

pub(crate) fn coercion_surface_markdown(
    owner: &TypeSymbol,
    resolved: &ResolveOutput,
) -> Option<String> {
    let owner_label = type_owner_presentation_label(owner, resolved);
    let entries = owner
        .coercions
        .iter()
        .filter(|coercion| coercion.is_accessible)
        .map(|coercion| {
            let visibility = if coercion.visibility.is_private() {
                String::new()
            } else {
                format!("{} ", coercion.visibility.source_notation())
            };
            let provenance = if coercion.result_provenance.is_some() {
                " from self"
            } else {
                ""
            };
            format!(
                "- `{visibility}coerce {}{} as {}{provenance}`",
                coercion.receiver.mode.source_prefix(),
                owner_label,
                crate::typecheck::type_expr_presentation_label(&coercion.target, resolved),
            )
        })
        .collect::<Vec<_>>();
    (!entries.is_empty()).then(|| format!("**Coercions**\n\n{}", entries.join("\n")))
}
