//! Selection for source-defined collection expansion operators.

use super::model::{Type, TypeEnvironment};
use crate::ast::{CallExpr, Expr, MemberExpr, MethodReceiverMode};
use crate::resolve::ResolveOutput;
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpansionMode {
    Readonly,
    Readwrite,
    Owned,
}

impl ExpansionMode {
    fn method_name(self) -> &'static str {
        match self {
            Self::Readonly => {
                crate::semantic::OperatorCallableKind::ReadonlyExpansion.lookup_name()
            }
            Self::Readwrite => {
                crate::semantic::OperatorCallableKind::ReadwriteExpansion.lookup_name()
            }
            Self::Owned => crate::semantic::OperatorCallableKind::OwnedExpansion.lookup_name(),
        }
    }

    fn receiver_mode(self) -> MethodReceiverMode {
        match self {
            Self::Readonly => MethodReceiverMode::ReadonlyBorrow,
            Self::Readwrite => MethodReceiverMode::ReadwriteBorrow,
            Self::Owned => MethodReceiverMode::Owned,
        }
    }

    fn requirement_source(self, target: &Type) -> Type {
        match self {
            Self::Readonly => Type::Borrow {
                is_readwrite: false,
                inner: Box::new(target.clone()),
            },
            Self::Readwrite => Type::Borrow {
                is_readwrite: true,
                inner: Box::new(target.clone()),
            },
            Self::Owned => target.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectedExpansion {
    pub(super) iterator_type: Type,
    pub(super) method: Option<super::iteration::IterationMethodResolution>,
    pub(super) protocol_method: Option<super::facts::TypecheckProtocolMethod>,
}

pub(super) fn select_expansion(
    target: &Type,
    mode: ExpansionMode,
    span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> Option<SelectedExpansion> {
    if target.is_unknown_or_unresolved() {
        return None;
    }
    let required_source = mode.requirement_source(target);
    if let Some(requirement) = environment.expansion_requirement(&required_source) {
        return Some(SelectedExpansion {
            iterator_type: requirement.result.clone(),
            method: None,
            protocol_method: None,
        });
    }

    let mut local = environment.clone();
    local.define_binding(
        "__nocter_expansion_source".to_string(),
        target.clone(),
        true,
    );
    let call = CallExpr {
        span,
        callee: Box::new(Expr::Member(MemberExpr {
            span,
            object: Box::new(Expr::Identifier(crate::ast::IdentifierExpr {
                span,
                name: "__nocter_expansion_source".to_string(),
            })),
            member: mode.method_name().to_string(),
            member_span: span,
        })),
        arguments_span: span,
        arguments: Vec::new(),
    };
    let selected = super::calls::resolved_method_call(resolved, &call, &local)?;
    if selected.method.receiver.mode != mode.receiver_mode() {
        return None;
    }
    let parameters = selected
        .method
        .signature
        .generic_parameters
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut substitutions = std::collections::HashMap::new();
    if let Some(owner_target) = &selected.method.owner_target_ty {
        super::type_expr::infer_type_expr_substitutions(
            owner_target,
            selected.self_type.opaque_lowering_view(),
            resolved,
            None,
            &parameters,
            &mut substitutions,
        );
    }
    let iterator_type = super::type_expr::type_expr_to_type_with_substitutions(
        &selected.method.signature.return_type,
        resolved,
        Some(&selected.self_type),
        &substitutions,
    );
    if iterator_type.is_unknown_or_unresolved() {
        return None;
    }
    let protocol_method = super::operators::operator_method_fact(&selected, span, resolved);
    Some(SelectedExpansion {
        iterator_type,
        method: Some(super::iteration::IterationMethodResolution {
            declaration: selected.method.name_span,
            method_name: selected.method.name.clone(),
            target_name: format!("{}.{}", selected.self_type.display(), selected.method.name),
            receiver_mode: selected.method.receiver.mode,
        }),
        protocol_method,
    })
}

fn mode_from_source_mode(
    mode: super::facts::TypecheckCollectionForSourceMode,
) -> Option<ExpansionMode> {
    match mode {
        super::facts::TypecheckCollectionForSourceMode::Direct => None,
        super::facts::TypecheckCollectionForSourceMode::ReadonlyConversion => {
            Some(ExpansionMode::Readonly)
        }
        super::facts::TypecheckCollectionForSourceMode::ReadwriteConversion => {
            Some(ExpansionMode::Readwrite)
        }
        super::facts::TypecheckCollectionForSourceMode::OwnedConversion => {
            Some(ExpansionMode::Owned)
        }
    }
}

pub(crate) fn specialize_collection_plan(
    mut plan: super::facts::TypecheckCollectionForPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckCollectionForPlan> {
    if plan.conversion.is_some()
        || plan.source_mode == super::facts::TypecheckCollectionForSourceMode::Direct
    {
        return Some(plan);
    }
    let mode = mode_from_source_mode(plan.source_mode)?;
    let source = super::type_expr::type_expr_to_type(&plan.source_type, resolved);
    let selected = select_expansion(
        &source,
        mode,
        plan.source_span,
        resolved,
        &TypeEnvironment::default(),
    )?;
    let expected = super::type_expr::type_expr_to_type(&plan.iterator_type, resolved);
    if !TypeEnvironment::default().types_equal(&selected.iterator_type, &expected) {
        return None;
    }
    plan.conversion = selected.protocol_method;
    Some(plan)
}

pub(crate) fn specialize_sequence_spread_plan(
    mut plan: super::facts::TypecheckSequenceSpreadPlan,
    resolved: &ResolveOutput,
) -> Option<super::facts::TypecheckSequenceSpreadPlan> {
    if plan.conversion.is_some()
        || plan.source_mode == super::facts::TypecheckCollectionForSourceMode::Direct
    {
        return Some(plan);
    }
    let mode = mode_from_source_mode(plan.source_mode)?;
    let source = super::type_expr::type_expr_to_type(&plan.source_type, resolved);
    let selected = select_expansion(
        &source,
        mode,
        plan.source_span,
        resolved,
        &TypeEnvironment::default(),
    )?;
    let expected = super::type_expr::type_expr_to_type(&plan.iterator_type, resolved);
    if !TypeEnvironment::default().types_equal(&selected.iterator_type, &expected) {
        return None;
    }
    plan.conversion = selected.protocol_method;
    Some(plan)
}

pub(super) fn types_support_expansion(
    target: &Type,
    mode: ExpansionMode,
    expected_result: &Type,
    span: ByteSpan,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
) -> bool {
    select_expansion(target, mode, span, resolved, environment)
        .is_some_and(|selected| environment.types_equal(&selected.iterator_type, expected_result))
}
