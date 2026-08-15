//! Generated callable entry points for concrete closure values.

use super::context::{FunctionNames, FunctionSignatures, ResolvedSources};
use super::functions::parameters::{
    lower_aggregate_parameter_setup, lower_function_return_type, lower_scalar_parameters,
    resolved_function_signature, validate_parameter_slots_match_function_abi,
};
use super::{
    FunctionSignature, lower_signature_parameter_type, lower_signature_return_type,
    parameter_abi_word_count, success_return_passing,
};
use crate::ast::{ClosureExpr, MethodReceiverMode, Parameter, TypeExpr};
use crate::diagnostics::Diagnostic;
use crate::ir::{CallTarget, Function};
use crate::resolve::ResolveOutput;
use crate::source::{SourceId, SourceMap};
use crate::typecheck::{TypecheckClosurePlan, TypedHir};

pub(super) fn closure_function_signature(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    receiver_mode: MethodReceiverMode,
    resolved: &ResolveOutput,
    resolved_sources: &ResolvedSources<'_>,
) -> Option<FunctionSignature> {
    let parameters = closure_parameters(expression, plan, receiver_mode);
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
    mir_bodies: &crate::mir::BodyCache,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    error_payloads: super::context::ErrorPayloads,
    root_source: SourceId,
    resolved: &'a ResolveOutput,
    typed_hir: &'a TypedHir,
    resolved_sources: ResolvedSources<'a>,
) -> Result<Function, Vec<Diagnostic>> {
    let parameters = closure_parameters(expression, plan, receiver_mode);
    let return_type_expr = (*plan.ty.return_type).clone();
    let parameter_slots = lower_scalar_parameters(
        &name,
        &parameters,
        root_source,
        resolved,
        &resolved_sources,
        sources,
    )?;
    validate_parameter_slots_match_function_abi(
        &name,
        &parameters,
        &return_type_expr,
        resolved,
        &resolved_sources,
        &parameter_slots,
    )?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameter_slots);
    let return_type =
        lower_function_return_type(&return_type_expr, &name, resolved, &resolved_sources)?;
    let instructions = super::mir::lower_closure_body(
        mir_bodies,
        expression,
        &plan.ty,
        receiver_mode,
        &return_type,
        &parameters,
        resolved,
        &resolved_sources,
        typed_hir,
        &name,
        &function_signatures,
        &function_names,
        &error_payloads,
        &parameter_slots,
        root_source,
        sources,
    )?;
    let mut lowered = parameter_setup;
    lowered.extend(instructions);
    Ok(Function {
        name,
        target,
        return_type,
        instructions: lowered,
    })
}

fn closure_parameters(
    expression: &ClosureExpr,
    plan: &TypecheckClosurePlan,
    receiver_mode: MethodReceiverMode,
) -> Vec<Parameter> {
    let closure_ty = TypeExpr::Closure(plan.ty.clone());
    let receiver_ty = match receiver_mode {
        MethodReceiverMode::Owned => closure_ty,
        MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {
            TypeExpr::Borrow(crate::ast::BorrowType {
                span: expression.parameters_span,
                is_readwrite: receiver_mode == MethodReceiverMode::ReadwriteBorrow,
                inner: Box::new(closure_ty),
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
