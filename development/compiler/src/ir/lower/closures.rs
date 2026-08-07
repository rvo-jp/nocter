//! Generated callable entry points for concrete closure values.

use super::context::{ErrorPayloads, FunctionNames, FunctionSignatures, ResolvedSources};
use super::functions::lower_method_function_with_prologue;
use super::functions::parameters::{method_parameters, resolved_function_signature};
use super::{
    FunctionSignature, lower_signature_parameter_type, lower_signature_return_type,
    parameter_abi_word_count, success_return_passing,
};
use crate::ast::{
    BorrowExpr, ClosureCaptureMode, ClosureExpr, Expr, GenericParamList, IdentifierExpr,
    MethodDecl, MethodReceiver, MethodReceiverMode, Parameter, ParameterList, StructLiteralExpr,
    StructLiteralField, TypeExpr, UnaryExpr, UnaryOperator, Visibility,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, CallTarget, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::{SourceId, SourceMap};
use crate::typecheck::{TypecheckClosurePlan, TypecheckFacts};
use std::collections::HashMap;

pub(super) fn lower_closure_to_slot(
    expression: &ClosureExpr,
    ty: &TypeExpr,
    slot_index: usize,
    context: &super::context::LoweringContext,
    temporaries: &mut super::expressions::TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let value = context.abi_value_for_type_expr(ty).ok_or_else(|| {
        vec![Diagnostic::error(
            "E8015",
            "closure environment has no concrete ABI layout",
        )]
    })?;
    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(vec![Diagnostic::error(
            "E8015",
            "closure lowering requires resolved source information",
        )]);
    };
    let literal = closure_environment_literal(expression, ty.clone());
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: value.layout,
    }];
    instructions.extend(
        super::aggregates::lower_aggregate_struct_literal_to_location_with_temporaries(
            &literal,
            value.layout,
            AggregateLocation::Slot(slot_index),
            "E8015",
            "closure environments",
            resolved,
            context,
            temporaries,
        )?,
    );
    Ok(instructions)
}

fn closure_environment_literal(expression: &ClosureExpr, ty: TypeExpr) -> StructLiteralExpr {
    StructLiteralExpr {
        span: expression.span,
        ty,
        fields_span: expression.parameters_span,
        fields: expression
            .captures
            .iter()
            .map(|capture| {
                let identifier = Expr::Identifier(IdentifierExpr {
                    span: capture.name_span,
                    name: capture.name.clone(),
                });
                let value = match capture.mode {
                    ClosureCaptureMode::ReadonlyBorrow | ClosureCaptureMode::ReadwriteBorrow => {
                        Expr::Borrow(BorrowExpr {
                            span: capture.span,
                            operator_span: capture.operator_span,
                            is_readwrite: capture.mode == ClosureCaptureMode::ReadwriteBorrow,
                            expression: Box::new(identifier),
                        })
                    }
                    ClosureCaptureMode::Move => Expr::Unary(UnaryExpr {
                        span: capture.span,
                        operator: UnaryOperator::Move,
                        operator_span: capture.operator_span,
                        operand: Box::new(identifier),
                    }),
                };
                StructLiteralField {
                    span: capture.span,
                    name: capture.name.clone(),
                    name_span: capture.name_span,
                    value,
                }
            })
            .collect(),
    }
}

pub(super) fn closure_function_signature(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    receiver_mode: MethodReceiverMode,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<FunctionSignature> {
    let method = closure_method(expression, plan, receiver_mode, "call");
    let parameters = method_parameters(
        &method,
        &crate::ast::TypeExpr::Closure(plan.ty.clone()),
        &HashMap::new(),
    );
    let return_type = (*plan.ty.return_type).clone();
    let resolved_signature = resolved_function_signature(&parameters, return_type.clone());
    Some(FunctionSignature {
        return_type: lower_signature_return_type(&return_type, resolved, resolved_sources)?,
        parameter_types: Some(
            parameters
                .iter()
                .map(|parameter| {
                    lower_signature_parameter_type(&parameter.ty, resolved, resolved_sources)
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        parameter_abi_word_count: parameter_abi_word_count(
            &resolved_signature,
            resolved,
            resolved_sources,
        ),
        success_return_passing: success_return_passing(
            &resolved_signature,
            resolved,
            resolved_sources,
        ),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_closure_function<'a>(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    receiver_mode: MethodReceiverMode,
    name: String,
    sources: &SourceMap,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typecheck_facts: &'a TypecheckFacts,
    resolved_sources: ResolvedSources<'a>,
    error_payloads: ErrorPayloads,
) -> Result<Function, Vec<Diagnostic>> {
    let method = closure_method(expression, plan, receiver_mode, &name);
    lower_method_function_with_prologue(
        &method,
        &crate::ast::TypeExpr::Closure(plan.ty.clone()),
        &HashMap::new(),
        name,
        sources,
        target,
        function_signatures,
        function_names,
        root_source,
        resolved,
        typecheck_facts,
        resolved_sources,
        error_payloads,
        |context| closure_capture_prologue(expression, plan, context),
    )
}

fn closure_capture_prologue(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    context: &mut super::context::LoweringContext<'_>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let instructions = Vec::new();
    for (capture, capture_ty) in expression.captures.iter().zip(&plan.ty.captures) {
        let field = context
            .aggregate_field("self", &capture.name)
            .ok_or_else(closure_capture_lowering_diagnostic)?;
        let expected = context
            .ir_type_for_type_expr(&capture_ty.ty)
            .ok_or_else(closure_capture_lowering_diagnostic)?;
        if !closure_capture_field_matches_type(&field.kind, &expected) {
            return Err(closure_capture_lowering_diagnostic());
        }
        context.define_closure_capture_field(capture.name.clone(), field);
    }
    Ok(instructions)
}

fn closure_capture_field_matches_type(
    kind: &super::context::AggregateFieldKind,
    expected: &Type,
) -> bool {
    match (kind, expected) {
        (super::context::AggregateFieldKind::I32, Type::I32)
        | (super::context::AggregateFieldKind::U8, Type::U8)
        | (super::context::AggregateFieldKind::Usize, Type::Usize)
        | (super::context::AggregateFieldKind::Bool, Type::Bool)
        | (super::context::AggregateFieldKind::Str, Type::Str)
        | (super::context::AggregateFieldKind::Slice(_), Type::Slice { .. }) => true,
        (
            super::context::AggregateFieldKind::Borrow {
                is_readwrite: field_readwrite,
                inner: field_inner,
            },
            Type::Borrow {
                is_readwrite,
                inner,
            },
        ) => field_readwrite == is_readwrite && field_inner == inner.as_ref(),
        (
            super::context::AggregateFieldKind::Array { .. }
            | super::context::AggregateFieldKind::Aggregate { .. },
            Type::Aggregate { .. } | Type::DirectAggregate { .. },
        ) => true,
        _ => false,
    }
}

fn closure_capture_lowering_diagnostic() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8015",
        "closure capture storage does not match its inferred environment type",
    )]
}

fn closure_method(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    receiver_mode: MethodReceiverMode,
    name: &str,
) -> MethodDecl {
    let parameters = expression
        .parameters
        .iter()
        .zip(&plan.ty.parameters)
        .map(|(parameter, ty)| Parameter {
            span: parameter.span,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: ty.clone(),
        })
        .collect();
    MethodDecl {
        span: expression.span,
        visibility: Visibility::Private,
        result_allocation: None,
        receiver: MethodReceiver {
            span: expression.parameters_span,
            name: "self".to_string(),
            name_span: expression.parameters_span,
            mode: receiver_mode,
        },
        name: name.to_string(),
        name_span: expression.parameters_span,
        generics: GenericParamList {
            span: None,
            parameters: Vec::new(),
        },
        parameters: ParameterList {
            span: expression.parameters_span,
            parameters,
        },
        return_type: (*plan.ty.return_type).clone(),
        result_provenance: None,
        body: Some(expression.body.clone()),
    }
}
