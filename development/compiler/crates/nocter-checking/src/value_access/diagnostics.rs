//! Project a reusable body rejection through its exact current source binding.
use super::{BodyDiagnostic, Failure, Query, Rule};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_model::AccessExpressionId;

pub(super) fn query_rejection(
    context: &DeclarationContext<'_>,
    query: &Query,
    origin: Option<AccessExpressionId>,
    rule: Rule,
    message: &str,
) -> Failure {
    (|| {
        let current = query.bind(context)?;
        let origin = match origin {
            Some(id) => current.expression_origin(id),
            None => current.block_origin(query.body.root()),
        }
        .ok_or(Failure::InconsistentSyntax)?;
        let source = super::source_origin(current.syntax(), origin)?;
        Ok::<_, Failure>(Failure::Rejected(Box::new(BodyDiagnostic {
            rule,
            source: nocter_diagnostics::SourceDiagnostic::new(
                rule.diagnostic().0,
                message,
                source,
                Vec::new(),
                None::<&str>,
            ),
        })))
    })()
    .unwrap_or_else(|failure| failure)
}
