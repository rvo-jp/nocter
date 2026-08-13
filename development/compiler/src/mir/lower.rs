//! First vertical typed-HIR-to-MIR route: a scalar integer literal returned by
//! an otherwise source-empty body.

use super::ids::{BasicBlockId, LocalId};
use super::model::{
    BasicBlock, BinaryOperator, Body, Constant, Local, LocalSource, Operand, Place, Rvalue,
    ScalarType, Statement, Terminator,
};
use super::validate;
use super::validate::ValidationError;
use crate::ast::{Block, Expr, Parameter};
use crate::literals::decode_integer_literal_value;
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::semantic::SemanticDb;
use crate::typecheck::TypedHir;
use std::collections::HashMap;

mod coverage;
use coverage::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BuildError {
    MissingSourceBody,
    MissingTypedExpression,
    InvalidScalarConstant,
    MissingLocalSymbol,
    MissingParameterType,
    UnsupportedClaimedExpression,
    InvalidMir(Vec<ValidationError>),
}

pub(crate) fn try_build_scalar_body(
    block: &Block,
    parameters: &[Parameter],
    return_scalar: ScalarType,
    semantic_db: &SemanticDb,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> Option<Result<Body, BuildError>> {
    let (source_statements, tail) = scalar_body_parts(block)?;
    if !source_statements
        .iter()
        .all(|statement| statement.is_supported(resolved, typed_hir))
        || !tail.is_supported(resolved)
        || !parameters.iter().all(|parameter| {
            resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .is_some_and(|symbol| binding_scalar_type(symbol, typed_hir).is_some())
                && typed_hir.type_id(&parameter.ty).is_some()
        })
    {
        return None;
    }

    let return_ty = tail.result_type(typed_hir)?;
    Some((|| {
        let source_body = semantic_db
            .body_at(block.span)
            .ok_or(BuildError::MissingSourceBody)?;
        let return_local = LocalId::from_index(0);
        let mut locals = vec![Local {
            ty: return_ty,
            scalar: return_scalar,
            source: LocalSource::Return,
        }];
        let mut locals_by_symbol = HashMap::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let ty = typed_hir
                .type_id(&parameter.ty)
                .ok_or(BuildError::MissingParameterType)?;
            let symbol = resolved
                .local_symbol_id_at_name_span(parameter.name_span)
                .ok_or(BuildError::MissingLocalSymbol)?;
            let scalar = binding_scalar_type(symbol, typed_hir)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let local = LocalId::from_index(locals.len());
            locals.push(Local {
                ty,
                scalar,
                source: LocalSource::Parameter { symbol, index },
            });
            locals_by_symbol.insert(symbol, local);
        }
        let mut mir_statements = Vec::new();
        for source_statement in source_statements {
            let (local, value) = match source_statement {
                ScalarStatement::Binding(binding) => {
                    let symbol = resolved
                        .local_symbol_id_at_name_span(binding.name_span)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    let ty = typed_hir
                        .binding_type_expr(symbol)
                        .and_then(|ty| typed_hir.type_id(ty))
                        .ok_or(BuildError::MissingTypedExpression)?;
                    let scalar = binding_scalar_type(symbol, typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                    let local = LocalId::from_index(locals.len());
                    locals.push(Local {
                        ty,
                        scalar,
                        source: LocalSource::Binding(symbol),
                    });
                    locals_by_symbol.insert(symbol, local);
                    (local, &binding.initializer)
                }
                ScalarStatement::Assignment(assignment) => {
                    let Expr::Identifier(identifier) = &assignment.target else {
                        return Err(BuildError::UnsupportedClaimedExpression);
                    };
                    let symbol = resolved
                        .local_symbol_for_identifier(identifier)
                        .map(|symbol| symbol.id)
                        .ok_or(BuildError::MissingLocalSymbol)?;
                    (
                        *locals_by_symbol
                            .get(&symbol)
                            .ok_or(BuildError::MissingLocalSymbol)?,
                        &assignment.value,
                    )
                }
            };
            let destination_local = &locals[local.index()];
            let destination_ty = destination_local.ty;
            let destination_scalar = destination_local.scalar;
            lower_expression_to_place(
                local,
                value,
                destination_ty,
                destination_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut mir_statements,
            )?;
        }
        let blocks = if let Some(if_) = tail.conditional() {
            let condition_ty = known_expression_type(&if_.condition, typed_hir)
                .ok_or(BuildError::MissingTypedExpression)?;
            let condition = lower_operand(
                &if_.condition,
                condition_ty,
                ScalarType::Bool,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut mir_statements,
            )?;
            let then_result = scalar_branch_result(&if_.then_block)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let else_result = if_
                .else_block
                .as_ref()
                .and_then(scalar_branch_result)
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            let mut then_statements = Vec::new();
            lower_expression_to_place(
                return_local,
                then_result,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut then_statements,
            )?;
            let mut else_statements = Vec::new();
            lower_expression_to_place(
                return_local,
                else_result,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut else_statements,
            )?;
            vec![
                BasicBlock {
                    statements: mir_statements,
                    terminator: Terminator::Switch {
                        condition,
                        then_target: BasicBlockId::from_index(1),
                        else_target: BasicBlockId::from_index(2),
                    },
                },
                BasicBlock {
                    statements: then_statements,
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(3),
                    },
                },
                BasicBlock {
                    statements: else_statements,
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(3),
                    },
                },
                BasicBlock {
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ]
        } else {
            let expression = tail
                .expression()
                .ok_or(BuildError::UnsupportedClaimedExpression)?;
            lower_expression_to_place(
                return_local,
                expression,
                return_ty,
                return_scalar,
                resolved,
                &locals_by_symbol,
                typed_hir,
                &mut locals,
                &mut mir_statements,
            )?;
            vec![
                BasicBlock {
                    statements: mir_statements,
                    terminator: Terminator::Goto {
                        target: BasicBlockId::from_index(1),
                    },
                },
                BasicBlock {
                    statements: Vec::new(),
                    terminator: Terminator::Return,
                },
            ]
        };
        let body = Body {
            source_body,
            source_span: block.span,
            return_local,
            locals,
            entry: BasicBlockId::from_index(0),
            blocks,
        };
        validate(&body).map_err(BuildError::InvalidMir)?;
        Ok(body)
    })())
}

fn lower_expression_to_place(
    destination: LocalId,
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<(), BuildError> {
    let source = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?
        .id;
    let value = match expression {
        Expr::Binary(binary) => Rvalue::Binary {
            operator: mir_binary_operator(binary.operator)
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            left: lower_operand(
                &binary.left,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            )?,
            right: lower_operand(
                &binary.right,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            )?,
            ty,
        },
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
                statements,
            );
        }
        _ => Rvalue::Use(lower_simple_operand(
            expression, ty, scalar, resolved, locals,
        )?),
    };
    statements.push(Statement::Assign {
        destination: Place { local: destination },
        value,
        source,
    });
    Ok(())
}

fn lower_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
    typed_hir: &TypedHir,
    local_declarations: &mut Vec<Local>,
    statements: &mut Vec<Statement>,
) -> Result<Operand, BuildError> {
    if !matches!(expression, Expr::Binary(_)) {
        return match expression {
            Expr::Group(group) => lower_operand(
                &group.expression,
                ty,
                scalar,
                resolved,
                locals,
                typed_hir,
                local_declarations,
                statements,
            ),
            _ => lower_simple_operand(expression, ty, scalar, resolved, locals),
        };
    }

    let typed_expression = typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?;
    let temporary = LocalId::from_index(local_declarations.len());
    local_declarations.push(Local {
        ty,
        scalar,
        source: LocalSource::Temporary(typed_expression.id),
    });
    lower_expression_to_place(
        temporary,
        expression,
        ty,
        scalar,
        resolved,
        locals,
        typed_hir,
        local_declarations,
        statements,
    )?;
    Ok(Operand::Copy(Place { local: temporary }))
}

fn lower_simple_operand(
    expression: &Expr,
    ty: crate::semantic::TyId,
    scalar: ScalarType,
    resolved: &ResolveOutput,
    locals: &HashMap<LocalSymbolId, LocalId>,
) -> Result<Operand, BuildError> {
    match expression {
        Expr::IntegerLiteral(literal) => Ok(Operand::Constant(Constant {
            ty,
            scalar,
            value: decode_integer_literal_value(&literal.value)
                .ok_or(BuildError::InvalidScalarConstant)?,
        })),
        Expr::BoolLiteral(literal) => Ok(Operand::Constant(Constant {
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
            Ok(Operand::Copy(Place {
                local: *locals.get(&symbol).ok_or(BuildError::MissingLocalSymbol)?,
            }))
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}

fn mir_binary_operator(operator: crate::ast::BinaryOperator) -> Option<BinaryOperator> {
    match operator {
        crate::ast::BinaryOperator::Add => Some(BinaryOperator::Add),
        crate::ast::BinaryOperator::Subtract => Some(BinaryOperator::Subtract),
        crate::ast::BinaryOperator::Multiply => Some(BinaryOperator::Multiply),
        crate::ast::BinaryOperator::Divide => Some(BinaryOperator::Divide),
        crate::ast::BinaryOperator::Remainder => Some(BinaryOperator::Remainder),
        _ => None,
    }
}

#[cfg(test)]
#[path = "lower/tests.rs"]
mod tests;
