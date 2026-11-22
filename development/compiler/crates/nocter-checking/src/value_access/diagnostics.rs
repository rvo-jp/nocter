//! Project a reusable body rejection through its exact current source binding.
use super::{BodyDiagnostic, Failure, Query, Rule};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;
use nocter_diagnostics::{DiagnosticNote, SourceDiagnostic};
use nocter_model::AccessExpressionId;
use nocter_syntax::{BoundSyntax, SyntaxOrigin};

pub(super) fn authored_rejection(
    syntax: BoundSyntax<'_>,
    origin: SyntaxOrigin,
    rule: Rule,
    message: impl Into<Box<str>>,
    notes: Vec<DiagnosticNote>,
) -> Failure {
    let source = match super::source_origin(syntax, origin) {
        Ok(source) => source,
        Err(failure) => return failure,
    };
    Failure::Rejected(Box::new(BodyDiagnostic {
        rule,
        source: SourceDiagnostic::new(rule.diagnostic().0, message, source, notes, None::<&str>),
    }))
}

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
        Ok::<_, Failure>(authored_rejection(
            current.syntax(),
            origin,
            rule,
            message,
            Vec::new(),
        ))
    })()
    .unwrap_or_else(|failure| failure)
}
