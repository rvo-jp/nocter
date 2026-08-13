//! Scalar expression evaluation into MIR places, rvalues, and operands.

use super::BuildError;
use super::body_builder::ControlFlowBuilder;
use super::coverage::{known_expression_type, scalar_type};
use crate::ast::Expr;
use crate::literals::decode_integer_literal_value;
use crate::mir::{
    BinaryOperator, CallArgument, ComparisonOperator, LocalId, LocalOrigin, LocalStorage, Operand,
    Place, Rvalue, ScalarType, ScopeId, Statement,
};
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::typecheck::TypedHir;
use std::collections::HashMap;

pub(super) fn lower_call(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<crate::mir::Local>,
    control_flow: &mut ControlFlowBuilder,
    scope: ScopeId,
) -> Result<(crate::semantic::DefId, Vec<CallArgument>, bool), BuildError> {
    let Expr::Identifier(callee) = call.callee.without_groups() else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    let returns_never = typed_hir
        .expression(call.span)
        .is_some_and(|expression| expression.diverges);
    let callee = typed_hir
        .function_call_target(callee.span)
        .map(|definition| resolved.callable_bodies.canonical_definition(definition))
        .ok_or(BuildError::MissingCallTarget)?;
    let arguments = call
        .arguments
        .iter()
        .map(|argument| {
            let ty = known_expression_type(argument, typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            let scalar =
                scalar_type(ty, typed_hir).ok_or(BuildError::UnsupportedClaimedExpression)?;
            let operand = lower_operand(
                argument,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            )?;
            Ok(CallArgument {
                operand,
                ty,
                scalar,
            })
        })
        .collect::<Result<Vec<_>, BuildError>>()?;
    Ok((callee, arguments, returns_never))
}

pub(super) fn lower_expression_to_place(
    destination: LocalId,
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<crate::mir::Local>,
    control_flow: &mut ControlFlowBuilder,
    scope: ScopeId,
) -> Result<(), BuildError> {
    let source = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    let value = match expression {
        Expr::Binary(binary) => {
            if let Some(operator) = mir_binary_operator(binary.operator) {
                Rvalue::Binary {
                    operator,
                    left: lower_operand(
                        &binary.left,
                        ty,
                        scalar,
                        resolved,
                        locals,
                        typed_hir,
                        local_declarations,
                        control_flow,
                        scope,
                    )?,
                    right: lower_operand(
                        &binary.right,
                        ty,
                        scalar,
                        resolved,
                        locals,
                        typed_hir,
                        local_declarations,
                        control_flow,
                        scope,
                    )?,
                    ty,
                }
            } else {
                let operator = mir_comparison_operator(binary.operator)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                let operand_ty = known_expression_type(&binary.left, typed_hir)
                    .ok_or(BuildError::MissingTypedExpression)?;
                let operand_scalar = scalar_type(operand_ty, typed_hir)
                    .ok_or(BuildError::UnsupportedClaimedExpression)?;
                Rvalue::Compare {
                    operator,
                    left: lower_operand(
                        &binary.left,
                        operand_ty,
                        operand_scalar,
                        resolved,
                        locals,
                        typed_hir,
                        local_declarations,
                        control_flow,
                        scope,
                    )?,
                    right: lower_operand(
                        &binary.right,
                        operand_ty,
                        operand_scalar,
                        resolved,
                        locals,
                        typed_hir,
                        local_declarations,
                        control_flow,
                        scope,
                    )?,
                    operand_ty,
                    operand_scalar,
                    result_ty: ty,
                }
            }
        }
        Expr::Group(group) => {
            return lower_expression_to_place(
                destination,
                &group.expression,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            );
        }
        Expr::Call(call) => {
            let (callee, arguments, returns_never) = lower_call(
                call,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            )?;
            if returns_never {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return control_flow.emit_returning_call(source, callee, arguments, destination);
        }
        Expr::Force(force) => {
            let Expr::Call(call) = force.expression.without_groups() else {
                return Err(BuildError::UnsupportedClaimedExpression);
            };
            let (callee, arguments, returns_never) = lower_call(
                call,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            )?;
            if returns_never {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return control_flow.emit_trapping_outcome_call(source, callee, arguments, destination);
        }
        Expr::Propagate(propagate) => {
            let Expr::Call(call) = propagate.expression.without_groups() else {
                return Err(BuildError::UnsupportedClaimedExpression);
            };
            let (callee, arguments, returns_never) = lower_call(
                call,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            )?;
            if returns_never {
                return Err(BuildError::UnsupportedClaimedExpression);
            }
            return control_flow.emit_propagating_outcome_call(
                source,
                callee,
                arguments,
                destination,
            );
        }
        _ => Rvalue::Use(lower_simple_operand(
            expression, ty, scalar, resolved, locals,
        )?),
    };
    control_flow.push_statement(Statement::Assign {
        destination: Place::local(destination),
        value,
        origin: crate::mir::Origin::Expression(source),
    })?;
    Ok(())
}

pub(super) fn lower_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<crate::mir::Local>,
    control_flow: &mut ControlFlowBuilder,
    scope: ScopeId,
) -> Result<Operand, BuildError> {
    if !matches!(
        expression,
        Expr::Binary(_) | Expr::Call(_) | Expr::Force(_) | Expr::Propagate(_)
    ) {
        return match expression {
            Expr::Group(group) => lower_operand(
                &group.expression,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                control_flow,
                scope,
            ),
            _ => lower_simple_operand(expression, ty, scalar, resolved, locals),
        };
    }

    let typed_expression = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?;
    let temporary = LocalId::from_index(local_declarations.len());
    local_declarations.push(crate::mir::locals::Local::scalar(
        ty,
        scalar,
        LocalStorage::Local,
        LocalOrigin::Temporary(typed_expression.id),
        scope,
    ));
    lower_expression_to_place(
        temporary,
        expression,
        ty,
        scalar,
        resolved,
        locals,
        typed_hir,
        local_declarations,
        control_flow,
        scope,
    )?;
    Ok(Operand::Copy(Place::local(temporary)))
}

fn lower_simple_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
) -> Result<Operand, BuildError> {
    match expression {
        Expr::IntegerLiteral(literal) => Ok(Operand::Constant(crate::mir::model::Constant {
            ty,
            scalar,
            value: decode_integer_literal_value(&literal.value)
                .ok_or(BuildError::InvalidScalarConstant)?,
        })),
        Expr::BoolLiteral(literal) => Ok(Operand::Constant(crate::mir::model::Constant {
            ty,
            scalar,
            value: match literal.value.as_str() {
                "false" => 0,
                "true" => 1,
                _ => return Err(BuildError::InvalidScalarConstant),
            },
        })),
        Expr::Identifier(identifier) => {
            let symbol = resolved
                .local_symbol_for_identifier(identifier)
                .map(|symbol| symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            Ok(Operand::Copy(Place::local(
                *locals.get(&symbol).ok_or(BuildError::MissingLocalSymbol)?,
            )))
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

pub(super) fn mir_binary_operator(operator: crate::ast::BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        crate::ast::BinaryOperator::Add => Some(BinaryOperator::Add),
        crate::ast::BinaryOperator::Subtract => Some(BinaryOperator::Subtract),
        crate::ast::BinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        crate::ast::BinaryOperator::Divide => Some(BinaryOperator::Divide),
        crate::ast::BinaryOperator::Remainder => Some(BinaryOperator::Remainder),
        _ => None,
    }
}

pub(super) fn mir_comparison_operator(
    operator: crate::ast::BinaryOperator,
) -> Option<ComparisonOperator> {
    match operator {
        crate::ast::BinaryOperator::Equal => Some(ComparisonOperator::Equal),
        crate::ast::BinaryOperator::NotEqual => Some(ComparisonOperator::NotEqual),
        crate::ast::BinaryOperator::Less => Some(ComparisonOperator::Less),
        crate::ast::BinaryOperator::LessEqual => Some(ComparisonOperator::LessEqual),
        crate::ast::BinaryOperator::Greater => Some(ComparisonOperator::Greater),
        crate::ast::BinaryOperator::GreaterEqual => Some(ComparisonOperator::GreaterEqual),
        _ => None,
    }
}
