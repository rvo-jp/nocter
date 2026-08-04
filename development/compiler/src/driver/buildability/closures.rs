//! Reachability and ABI boundaries for generated closure callables.

use super::*;

pub(super) fn closure_parameters(
    expression: &crate::ast::ClosureExpr,
    plan: &crate::typecheck::TypecheckClosurePlan,
    receiver_mode: crate::ast::MethodReceiverMode,
) -> Vec<Parameter> {
    let self_ty = TypeExpr::Closure(plan.ty.clone());
    let receiver_ty = match receiver_mode {
        crate::ast::MethodReceiverMode::Owned => self_ty,
        crate::ast::MethodReceiverMode::ReadonlyBorrow
        | crate::ast::MethodReceiverMode::ReadwriteBorrow => {
            TypeExpr::Borrow(crate::ast::BorrowType {
                span: expression.parameters_span,
                is_readwrite: receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow,
                inner: Box::new(self_ty),
            })
        }
    };
    let mut parameters = vec![Parameter {
        span: expression.parameters_span,
        name: "self".to_string(),
        name_span: expression.parameters_span,
        ty: receiver_ty,
    }];
    parameters.extend(expression.parameters.iter().zip(&plan.ty.parameters).map(
        |(parameter, ty)| Parameter {
            span: parameter.span,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: ty.clone(),
        },
    ));
    parameters
}

pub(super) fn closure_signature_issues(
    expression: &crate::ast::ClosureExpr,
    plan: &crate::typecheck::TypecheckClosurePlan,
    receiver_mode: crate::ast::MethodReceiverMode,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Vec<BuildabilityIssue> {
    let parameters = closure_parameters(expression, plan, receiver_mode);
    let mut issues =
        callable_parameter_issues(&parameters, &HashMap::new(), resolved, resolved_sources);
    let source_resolver = |source| resolved_sources.get(&source).copied();
    if !callable_return_type_is_buildable_with_resolver(
        &plan.ty.return_type,
        resolved,
        &source_resolver,
    ) {
        issues.push(BuildabilityIssue {
            span: plan.ty.return_type.span(),
            construct: "closure return types outside the runtime ABI subset",
            help: "return a supported scalar, view, aggregate, optional, or fallible value from the closure",
        });
    }
    issues
}
