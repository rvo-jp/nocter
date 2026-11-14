//! Project result identities and freeze pending value supply before initialization.
use super::Evidence;
use crate::value_access::{Failure, Query, ir::ExpressionKind};
use nocter_declaration_lowering::access_declarations::Context as DeclarationContext;

pub(super) fn recursive_environment(
    context: &DeclarationContext<'_>,
    scope: &std::collections::BTreeMap<
        nocter_model::ValueCode,
        &crate::value_access::flow::Control,
    >,
    id: nocter_model::ValueClosure,
) -> crate::value_access::accepted::Failure {
    for control in scope.values() {
        let query = &control.typed;
        for (expression, value) in query.body.expressions.iter() {
            if matches!(value.kind, ExpressionKind::Closure { closure, .. } if closure == id) {
                return crate::value_access::accepted::Failure::Body {
                    callable: query.source_owner(),
                    failure: crate::value_access::diagnostics::query_rejection(
                        context,
                        query,
                        Some(expression),
                        crate::value_access::Rule::InvalidConstruction,
                        "callable-result resolution makes this captured environment contain itself by value",
                    ),
                };
            }
        }
    }
    crate::value_access::accepted::Failure::Integrity(
        crate::value_access::flow::IntegrityError::InconsistentTypedBody,
    )
}

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
    if let ExpressionKind::Closure { closure, .. } = expression.kind {
        let environment = query
            .program()
            .closure(closure)
            .ok_or(Failure::InconsistentSyntax)?
            .environment();
        if !resolved_descriptor(query.program(), environment) {
            return Err(reject(
                Rule::TypeMismatch,
                "this owned capture has no established component payloads after callable-result resolution",
            ));
        }
    }
    if query
        .result_evidence
        .as_ref()
        .is_some_and(|evidence| evidence.unproduced(query.program(), access.data()))
    {
        // No constructor, snapshot or cleanup can execute for this absent shape. The common
        // reachable-use validator proves that fact over the original control graph.
        return Ok(());
    }
    validate_call_result(query, expression)?;
    if matches!(
        expression.kind,
        ExpressionKind::Aggregate { .. } | ExpressionKind::Repeat { .. }
    ) {
        let ready = if matches!(
            query.program().data_schema(access.data()),
            Some(PayloadSchema::Aggregate(_))
        ) {
            resolved_descriptor(query.program(), access.data())
        } else {
            query.program().owned_payload_supported(access.data())
        };
        if !ready {
            return Err(reject(
                Rule::UnsupportedData,
                "this structural value needs established component payloads; its callable result is unresolved",
            ));
        }
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

fn resolved_descriptor(
    program: &nocter_declarations::value_access::Program,
    data: nocter_model::TypeId,
) -> bool {
    program
        .payload_dependencies(data)
        .is_some_and(|dependencies| !dependencies.has_pending_producers())
}

fn validate_call_result(
    query: &Query,
    expression: &crate::value_access::ir::Expression,
) -> Result<(), Failure> {
    use nocter_declarations::value_access::PayloadSchema;
    let Some(access) = expression.ty.access() else {
        return Ok(());
    };
    if expression.ty.may_complete()
        && matches!(
            query.program().data_schema(access.data()),
            Some(PayloadSchema::Function(_))
        )
        && let ExpressionKind::Call {
            target: nocter_model::ValueFunctionOrigin::Closure(owner),
            kind,
            arguments,
        } = &expression.kind
    {
        let invocation = kind
            .invocation(arguments)
            .ok_or(Failure::InconsistentSyntax)?;
        let result = query.program().function_result(
            nocter_declarations::value_access::FunctionWitness {
                origin: nocter_model::ValueFunctionOrigin::Closure(*owner),
                invocation,
            },
            |parameter| {
                arguments
                    .iter()
                    .filter_map(|argument| argument.binding.as_ref())
                    .find(|binding| binding.parameter == parameter)
                    .map(|binding| binding.witness)
            },
        );
        if result != Some(access.data()) {
            return Err(Failure::InconsistentSyntax);
        }
    }
    Ok(())
}
