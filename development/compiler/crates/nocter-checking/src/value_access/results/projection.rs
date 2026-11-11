//! Project result identities and freeze pending value supply before initialization.
use super::Evidence;
use crate::value_access::{Failure, Query, ir::ExpressionKind};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;

pub(super) fn project(
    context: &DeclarationContext<'_>,
    query: &mut Query,
    evidence: &Evidence,
    projection: &nocter_declarations::value_access::ResolvedBodyTypes,
) -> Result<(), Failure> {
    crate::value_access::types::project(
        query,
        |data| projection.project(data).ok_or(Failure::InconsistentSyntax),
        |query, id, expression| {
            if let ExpressionKind::Call {
                target, arguments, ..
            } = &mut expression.kind
            {
                *target = evidence.origin(*target)?;
                for argument in arguments {
                    if let Some(binding) = &mut argument.binding {
                        binding.witness = evidence.witness(binding.witness)?;
                    }
                }
            }
            validate_payload(context, query, id, expression)?;
            crate::value_access::supply::resolve(context, query, id, expression, &|data| {
                projection.project(data).ok_or(Failure::InconsistentSyntax)
            })
        },
    )?;
    Ok(())
}

fn validate_payload(
    context: &DeclarationContext<'_>,
    query: &Query,
    id: nocter_model::AccessExpressionId,
    expression: &mut crate::value_access::ir::Expression,
) -> Result<(), Failure> {
    use crate::value_access::{
        Rule,
        ir::{IndexAction, SelectionAction},
    };
    use nocter_declarations::value_access::PayloadSchema;
    let reject = |rule, message| {
        crate::value_access::diagnostics::query_rejection(context, query, Some(id), rule, message)
    };
    let Some(access) = expression.ty.access() else {
        return Ok(());
    };
    if matches!(
        expression.kind,
        ExpressionKind::Aggregate { .. } | ExpressionKind::Repeat { .. }
    ) && !query.program().owned_payload_supported(access.data())
    {
        return Err(reject(
            Rule::UnsupportedData,
            "this structural value needs established component payloads; its callable result is unresolved",
        ));
    }
    if let ExpressionKind::Repeat { .. } = expression.kind {
        let Some(PayloadSchema::Array(array)) = query.program().data_schema(access.data()) else {
            return Err(Failure::InconsistentSyntax);
        };
        if array.length() > 1 && query.program().data_copyable(array.element()) != Some(true) {
            return Err(reject(
                Rule::InvalidConstruction,
                "array repetition greater than one requires Copy element data",
            ));
        }
    }
    if let ExpressionKind::Index {
        action: IndexAction::CopyPayload,
        ..
    } = expression.kind
        && expression.ty.may_complete()
        && query.program().data_copyable(access.data()) != Some(true)
    {
        return Err(reject(
            Rule::UnsupportedData,
            "selecting this element from a temporary array requires Copy element data",
        ));
    }
    if let ExpressionKind::Select { action, .. } = &mut expression.kind
        && matches!(
            action,
            SelectionAction::CopyPayload | SelectionAction::MovePayload
        )
    {
        *action = if query.program().data_copyable(access.data()) == Some(true) {
            SelectionAction::CopyPayload
        } else {
            SelectionAction::MovePayload
        };
    }
    Ok(())
}
