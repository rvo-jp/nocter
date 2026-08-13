//! Authoritative coverage classification for the first scalar MIR route.
//! A body rejected here remains on the legacy route; once accepted, MIR
//! construction and validation errors are authoritative.

use super::SemanticInputs;
use super::expressions::{mir_assignment_operator, mir_binary_operator, mir_comparison_operator};
use crate::ast::{
    AssignmentOperator, AssignmentStmt, BindingStmt, Block, Expr, ForRangeStmt, IfStmt, LoopStmt,
    Stmt, WhileStmt,
};
use crate::literals::decode_integer_literal_value;
use crate::mir::ComparisonOperator;
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::typecheck::{CheckedScalarType, PartialSemantic, TypedHir};

#[derive(Debug, Clone, Copy)]
pub(super) enum ScalarStatement<'a> {
    Binding(&'a BindingStmt),
    Assignment(&'a AssignmentStmt),
    If(&'a IfStmt),
    ForRange(&'a ForRangeStmt),
    Loop(&'a LoopStmt),
    While(&'a WhileStmt),
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ScalarTail<'a> {
    Expression(&'a Expr),
    Conditional(&'a IfStmt),
}

impl<'a> ScalarTail<'a> {
    pub(super) fn expression(self) -> Option<&'a Expr> {
        match self {
            Self::Expression(expression) => Some(expression),
            Self::Conditional(_) => None,
        }
    }

    pub(super) fn conditional(self) -> Option<&'a IfStmt> {
        match self {
            Self::Expression(Expr::If(if_)) => Some(if_),
            Self::Conditional(if_) => Some(if_),
            Self::Expression(_) => None,
        }
    }

    pub(super) fn is_supported(self, semantic: SemanticInputs<'_>) -> bool {
        match self {
            Self::Expression(Expr::Call(call)) => scalar_tail_call_is_supported(
                call,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
            Self::Expression(Expr::If(if_)) => scalar_conditional_is_supported(
                if_,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
            Self::Expression(expression) => scalar_expression_is_supported(
                expression,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
            Self::Conditional(if_) => scalar_conditional_is_supported(
                if_,
                semantic.resolved,
                semantic.resolved_sources,
                semantic.typed_hir,
            ),
        }
    }

    pub(super) fn result_type(self, typed_hir: &TypedHir) -> Option<crate::semantic::TyId> {
        match self {
            Self::Expression(expression) => known_expression_type(expression, typed_hir),
            Self::Conditional(if_) => scalar_value_block_result_type(&if_.then_block, typed_hir),
        }
    }
}

fn scalar_tail_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        // A failure-only `error` call can acquire the surrounding success
        // type contextually, but it is not a scalar-returning call. Until MIR
        // carries failure payload values explicitly, leave that construct on
        // the outcome-aware route instead of manufacturing a scalar result.
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
        && effective_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn scalar_value_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn aggregate_value_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && effective_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .is_some_and(|ty| {
                matches!(
                    crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
                        resolved_sources.get(&source).copied()
                    })
                    .map(|value| value.ty),
                    Ok(crate::abi::AbiType::Struct(_))
                        | Ok(crate::abi::AbiType::Array { .. })
                        | Ok(crate::abi::AbiType::Enum(_))
                )
            })
}

fn scalar_call_shape_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let Expr::Identifier(callee) = call.callee.without_groups() else {
        return false;
    };
    typed_hir
        .function_call_target(callee.span)
        .and_then(|target| resolved.semantic_db.definition(target))
        .is_some_and(|definition| definition.kind == crate::semantic::DefinitionKind::Function)
        && typed_hir.generic_function_call_target(call.span).is_none()
        && typed_hir.function_call_specialization(call.span).is_none()
        && call.arguments.iter().all(|argument| {
            let Some(ty) = known_expression_type(argument, typed_hir) else {
                return false;
            };
            scalar_type(ty, typed_hir).is_some()
                && scalar_expression_is_supported(argument, resolved, resolved_sources, typed_hir)
                || copy_aggregate_identifier_is_supported(
                    argument,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
                || borrow_identifier_is_supported(argument, resolved, typed_hir)
        })
}

pub(super) fn borrow_identifier_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return false;
    };
    resolved
        .local_symbol_for_identifier(identifier)
        .and_then(|symbol| typed_hir.binding_type_expr(symbol.id))
        .is_some_and(|ty| matches!(ty, crate::ast::TypeExpr::Borrow(_)))
}

pub(super) fn copy_aggregate_identifier_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let Expr::Identifier(identifier) = expression.without_groups() else {
        return false;
    };
    let Some(symbol) = resolved.local_symbol_for_identifier(identifier) else {
        return false;
    };
    let Some(ty) = typed_hir.binding_type_expr(symbol.id) else {
        return false;
    };
    matches!(
        crate::abi::abi_value_from_type_expr_with_resolver(ty, resolved, |source| {
            resolved_sources.get(&source).copied()
        })
        .map(|value| value.ty),
        Ok(crate::abi::AbiType::Struct(_))
            | Ok(crate::abi::AbiType::Array { .. })
            | Ok(crate::abi::AbiType::Enum(_))
    ) && crate::typecheck::type_expr_is_copy(ty, resolved) == Some(true)
}

impl<'a> ScalarStatement<'a> {
    pub(super) fn is_supported(self, semantic: SemanticInputs<'_>) -> bool {
        self.is_supported_in_context(
            semantic.resolved,
            semantic.resolved_sources,
            semantic.typed_hir,
            false,
        )
    }

    fn is_supported_in_context(
        self,
        resolved: &ResolveOutput,
        resolved_sources: &crate::resolve::ResolvedSources<'_>,
        typed_hir: &TypedHir,
        in_loop: bool,
    ) -> bool {
        match self {
            Self::Binding(binding) => {
                let scalar = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .is_some_and(|symbol| {
                        binding_scalar_type(symbol, typed_hir).is_some()
                            && typed_hir
                                .binding_type_expr(symbol)
                                .and_then(|ty| typed_hir.type_id(ty))
                                .is_some()
                    });
                let borrow = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .and_then(|symbol| typed_hir.binding_type_expr(symbol))
                    .is_some_and(|ty| matches!(ty, crate::ast::TypeExpr::Borrow(_)))
                    && borrow_expression_is_supported(&binding.initializer, resolved);
                let aggregate = resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .and_then(|symbol| typed_hir.binding_type_expr(symbol))
                    .is_some_and(|ty| {
                        (crate::typecheck::type_expr_is_copy(ty, resolved) == Some(true)
                            || super::super::drop_plans::is_supported(
                                ty,
                                resolved,
                                resolved_sources,
                                typed_hir,
                            ))
                            && matches!(
                                crate::abi::abi_value_from_type_expr_with_resolver(
                                    ty,
                                    resolved,
                                    |source| resolved_sources.get(&source).copied(),
                                )
                                .map(|value| value.ty),
                                Ok(crate::abi::AbiType::Struct(_))
                                    | Ok(crate::abi::AbiType::Array { .. })
                                    | Ok(crate::abi::AbiType::Enum(_))
                            )
                    })
                    && match binding.initializer.without_groups() {
                        Expr::Call(call) => aggregate_value_call_is_supported(
                            call,
                            resolved,
                            resolved_sources,
                            typed_hir,
                        ),
                        Expr::StructLiteral(literal) => scalar_struct_literal_is_supported(
                            literal,
                            resolved,
                            resolved_sources,
                            typed_hir,
                        ),
                        _ => false,
                    };
                (scalar
                    && known_expression_type(&binding.initializer, typed_hir).is_some()
                    && scalar_expression_is_supported(
                        &binding.initializer,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    ))
                    || borrow
                    || aggregate
            }
            Self::Assignment(assignment) => {
                (assignment.operator == AssignmentOperator::Assign
                    || mir_assignment_operator(assignment.operator).is_some())
                    && matches!(&assignment.target, Expr::Identifier(identifier) if resolved.local_symbol_for_identifier(identifier).is_some_and(|symbol| binding_scalar_type(symbol.id, typed_hir).is_some()))
                    && known_expression_type(&assignment.value, typed_hir).is_some()
                    && scalar_expression_is_supported(
                        &assignment.value,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
            }
            Self::While(statement) => {
                scalar_expression_is_supported(
                    &statement.condition,
                    resolved,
                    resolved_sources,
                    typed_hir,
                ) && known_expression_type(&statement.condition, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(crate::mir::ScalarType::Bool)
                    && scalar_loop_block_statements(
                        &statement.body,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    .is_some()
            }
            Self::If(statement) => {
                scalar_expression_is_supported(
                    &statement.condition,
                    resolved,
                    resolved_sources,
                    typed_hir,
                ) && known_expression_type(&statement.condition, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(crate::mir::ScalarType::Bool)
                    && scalar_conditional_statement_is_supported(
                        statement,
                        resolved,
                        resolved_sources,
                        typed_hir,
                        in_loop,
                    )
            }
            Self::ForRange(statement) => {
                let Some(symbol) = resolved.local_symbol_id_at_name_span(statement.name_span)
                else {
                    return false;
                };
                let Some(binding_scalar) = binding_scalar_type(symbol, typed_hir) else {
                    return false;
                };
                matches!(
                    binding_scalar,
                    crate::mir::ScalarType::I32 | crate::mir::ScalarType::Usize
                ) && known_expression_type(&statement.start, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    == Some(binding_scalar)
                    && known_expression_type(&statement.end, typed_hir)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        == Some(binding_scalar)
                    && scalar_expression_is_supported(
                        &statement.start,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    && scalar_expression_is_supported(
                        &statement.end,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    && scalar_loop_block_statements(
                        &statement.body,
                        resolved,
                        resolved_sources,
                        typed_hir,
                    )
                    .is_some()
            }
            Self::Loop(statement) => {
                scalar_loop_block_statements(&statement.body, resolved, resolved_sources, typed_hir)
                    .is_some()
            }
            Self::Break | Self::Continue => in_loop,
        }
    }
}

fn scalar_struct_literal_is_supported(
    literal: &crate::ast::StructLiteralExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let result_is_aggregate = typed_hir
        .expression(literal.span)
        .and_then(|expression| match expression.ty {
            crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
            crate::typecheck::PartialSemantic::Error => None,
        })
        .is_some_and(|ty| scalar_type(ty, typed_hir).is_none());
    result_is_aggregate
        && literal.fields.iter().all(|field| {
            typed_hir.field_target(field.name_span).is_some()
                && known_expression_type(&field.value, typed_hir)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    .is_some()
                && scalar_expression_is_supported(
                    &field.value,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        })
}

pub(super) fn borrow_expression_is_supported(expression: &Expr, resolved: &ResolveOutput) -> bool {
    let Expr::Borrow(borrow) = expression.without_groups() else {
        return false;
    };
    let Expr::Identifier(identifier) = borrow.expression.without_groups() else {
        return false;
    };
    resolved.local_symbol_for_identifier(identifier).is_some()
}

fn scalar_statement(statement: &Stmt) -> Option<ScalarStatement<'_>> {
    match statement {
        Stmt::Binding(binding) => Some(ScalarStatement::Binding(binding)),
        Stmt::Assignment(assignment) => Some(ScalarStatement::Assignment(assignment)),
        Stmt::If(statement) => Some(ScalarStatement::If(statement)),
        Stmt::ForRange(statement) => Some(ScalarStatement::ForRange(statement)),
        Stmt::Loop(statement) => Some(ScalarStatement::Loop(statement)),
        Stmt::While(statement) => Some(ScalarStatement::While(statement)),
        Stmt::Break(_) => Some(ScalarStatement::Break),
        Stmt::Continue(_) => Some(ScalarStatement::Continue),
        _ => None,
    }
}

pub(super) fn scalar_linear_block_statements<'a>(
    block: &'a Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    in_loop: bool,
) -> Option<Vec<ScalarStatement<'a>>> {
    if block.result.is_some() {
        return None;
    }
    let statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .map(scalar_statement)
        .collect::<Option<Vec<_>>>()?;
    let mut exited = false;
    for statement in &statements {
        if exited
            || matches!(
                statement,
                ScalarStatement::If(_)
                    | ScalarStatement::While(_)
                    | ScalarStatement::ForRange(_)
                    | ScalarStatement::Loop(_)
            )
            || !statement.is_supported_in_context(resolved, resolved_sources, typed_hir, in_loop)
        {
            return None;
        }
        exited = matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        );
    }
    Some(statements)
}

fn scalar_conditional_statement_is_supported(
    statement: &IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
    in_loop: bool,
) -> bool {
    let Some(then_statements) = scalar_linear_block_statements(
        &statement.then_block,
        resolved,
        resolved_sources,
        typed_hir,
        in_loop,
    ) else {
        return false;
    };
    let else_statements = statement
        .else_block
        .as_ref()
        .map(|block| {
            scalar_linear_block_statements(block, resolved, resolved_sources, typed_hir, in_loop)
        })
        .unwrap_or_else(|| Some(Vec::new()));
    let Some(else_statements) = else_statements else {
        return false;
    };
    let then_exits = then_statements.last().is_some_and(|statement| {
        matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        )
    });
    let else_exits = else_statements.last().is_some_and(|statement| {
        matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        )
    });
    !(then_exits && else_exits)
}

pub(super) fn scalar_loop_block_statements<'a>(
    block: &'a Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> Option<Vec<ScalarStatement<'a>>> {
    if block.result.is_some() {
        return None;
    }
    let statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .map(scalar_statement)
        .collect::<Option<Vec<_>>>()?;
    let mut exited = false;
    for statement in &statements {
        if exited
            || matches!(
                statement,
                ScalarStatement::While(_) | ScalarStatement::ForRange(_) | ScalarStatement::Loop(_)
            )
            || !statement.is_supported_in_context(resolved, resolved_sources, typed_hir, true)
        {
            return None;
        }
        exited = matches!(
            statement,
            ScalarStatement::Break | ScalarStatement::Continue
        );
    }
    Some(statements)
}

pub(super) fn scalar_body_parts(
    block: &Block,
) -> Option<(Vec<ScalarStatement<'_>>, ScalarTail<'_>)> {
    let runtime_statements = block
        .statements
        .iter()
        .filter(|statement| !matches!(statement, Stmt::Import(_) | Stmt::FromImport(_)))
        .collect::<Vec<_>>();
    let (body_statements, tail) = if let Some(result) = block.result.as_deref() {
        (
            runtime_statements.as_slice(),
            ScalarTail::Expression(result),
        )
    } else {
        let (last, leading) = runtime_statements.split_last()?;
        let tail = match last {
            Stmt::Return(statement) => ScalarTail::Expression(statement.expression.as_ref()?),
            Stmt::If(if_) => ScalarTail::Conditional(if_),
            _ => return None,
        };
        (leading, tail)
    };
    let statements = body_statements
        .iter()
        .map(|statement| scalar_statement(statement))
        .collect::<Option<Vec<_>>>()?;
    Some((statements, tail))
}

pub(super) fn scalar_expression_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    match expression {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value).is_some(),
        Expr::BoolLiteral(literal) => matches!(literal.value.as_str(), "true" | "false"),
        Expr::Identifier(identifier) => resolved.local_symbol_for_identifier(identifier).is_some(),
        Expr::Member(member) => super::projections::scalar_field_is_supported(
            member,
            SemanticInputs {
                resolved,
                resolved_sources,
                typed_hir,
            },
        ),
        Expr::Group(group) => {
            scalar_expression_is_supported(&group.expression, resolved, resolved_sources, typed_hir)
        }
        Expr::Call(call) => {
            scalar_value_call_is_supported(call, resolved, resolved_sources, typed_hir)
        }
        Expr::Force(force) => {
            let Expr::Call(call) = force.expression.without_groups() else {
                return false;
            };
            scalar_outcome_call_is_supported(call, resolved, resolved_sources, typed_hir)
        }
        Expr::Propagate(propagate) => {
            let Expr::Call(call) = propagate.expression.without_groups() else {
                return false;
            };
            scalar_outcome_call_is_supported(call, resolved, resolved_sources, typed_hir)
        }
        Expr::Otherwise(otherwise) => {
            let Expr::Call(call) = otherwise.value.without_groups() else {
                return false;
            };
            scalar_handled_call_is_supported(call, resolved, resolved_sources, typed_hir)
                && otherwise.fallback.result.is_some()
                && scalar_value_block_is_supported(
                    &otherwise.fallback,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = catch.expression.without_groups() else {
                return false;
            };
            matches!(catch.binding, crate::ast::CatchBinding::Discard { .. })
                && scalar_caught_call_is_supported(call, resolved, resolved_sources, typed_hir)
                && catch.catch_block.result.is_some()
                && scalar_value_block_is_supported(
                    &catch.catch_block,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::Unary(unary) => {
            let Some(operand_ty) = known_expression_type(&unary.operand, typed_hir) else {
                return false;
            };
            let Some(operand_scalar) = scalar_type(operand_ty, typed_hir) else {
                return false;
            };
            let operator_is_supported = match unary.operator {
                crate::ast::UnaryOperator::LogicalNot => operand_scalar == super::ScalarType::Bool,
                crate::ast::UnaryOperator::Negate => match operand_scalar {
                    super::ScalarType::I32 => true,
                    super::ScalarType::Integer(kind) => kind.is_signed(),
                    super::ScalarType::U8 | super::ScalarType::Usize | super::ScalarType::Bool => {
                        false
                    }
                },
                crate::ast::UnaryOperator::Move | crate::ast::UnaryOperator::Spread => false,
            };
            operator_is_supported
                && scalar_expression_is_supported(
                    &unary.operand,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::TypeConversion(conversion) => {
            let Some(source_ty) = known_expression_type(&conversion.expression, typed_hir) else {
                return false;
            };
            let Some(target_ty) = known_expression_type(expression, typed_hir) else {
                return false;
            };
            let Some(source_scalar) = scalar_type(source_ty, typed_hir) else {
                return false;
            };
            let Some(target_scalar) = scalar_type(target_ty, typed_hir) else {
                return false;
            };
            let checked_numeric_conversion = source_scalar == target_scalar
                || typed_hir
                    .conversion_plan(conversion.span)
                    .is_some_and(|plan| {
                        plan.kind == crate::typecheck::TypecheckConversionKind::LosslessInteger
                    });
            checked_numeric_conversion
                && scalar_expression_is_supported(
                    &conversion.expression,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::Binary(binary) => {
            (mir_binary_operator(binary.operator).is_some()
                || scalar_comparison_is_supported(binary, typed_hir)
                || scalar_logical_is_supported(binary, typed_hir))
                && scalar_expression_is_supported(
                    &binary.left,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
                && scalar_expression_is_supported(
                    &binary.right,
                    resolved,
                    resolved_sources,
                    typed_hir,
                )
        }
        Expr::If(if_) => {
            scalar_conditional_is_supported(if_, resolved, resolved_sources, typed_hir)
        }
        _ => false,
    }
}

fn scalar_logical_is_supported(binary: &crate::ast::BinaryExpr, typed_hir: &TypedHir) -> bool {
    matches!(
        binary.operator,
        crate::ast::BinaryOperator::LogicalAnd | crate::ast::BinaryOperator::LogicalOr
    ) && known_expression_type(&binary.left, typed_hir).and_then(|ty| scalar_type(ty, typed_hir))
        == Some(super::ScalarType::Bool)
        && known_expression_type(&binary.right, typed_hir).and_then(|ty| scalar_type(ty, typed_hir))
            == Some(super::ScalarType::Bool)
}

fn scalar_outcome_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                shape.layers.as_slice() == [crate::outcomes::OutcomeLayer::Fallible]
                    && typed_hir
                        .type_id(&shape.payload)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        .is_some()
            })
}

fn scalar_handled_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                matches!(
                    shape.layers.as_slice(),
                    [crate::outcomes::OutcomeLayer::Optional]
                        | [crate::outcomes::OutcomeLayer::Fallible]
                ) && typed_hir
                    .type_id(&shape.payload)
                    .and_then(|ty| scalar_type(ty, typed_hir))
                    .is_some()
            })
}

fn scalar_caught_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, resolved_sources, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| typed_hir.type_expr_by_id(ty))
            .map(|ty| crate::outcomes::outcome_shape_with_resolver(ty, resolved, |_| None))
            .is_some_and(|shape| {
                shape.layers.as_slice() == [crate::outcomes::OutcomeLayer::Fallible]
                    && typed_hir
                        .type_id(&shape.payload)
                        .and_then(|ty| scalar_type(ty, typed_hir))
                        .is_some()
            })
}

fn intrinsic_expression_type(
    span: crate::source::ByteSpan,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    let PartialSemantic::Known(ty) = typed_hir.expression(span)?.ty else {
        return None;
    };
    Some(ty)
}

fn scalar_value_block_is_supported(
    block: &Block,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    let Some((statements, tail)) = scalar_body_parts(block) else {
        return false;
    };
    statements.iter().all(|statement| {
        statement.is_supported_in_context(resolved, resolved_sources, typed_hir, false)
    }) && tail.is_supported(SemanticInputs {
        resolved,
        resolved_sources,
        typed_hir,
    })
}

fn scalar_value_block_result_type(
    block: &Block,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    scalar_body_parts(block)?.1.result_type(typed_hir)
}

fn scalar_conditional_is_supported(
    if_: &IfStmt,
    resolved: &ResolveOutput,
    resolved_sources: &crate::resolve::ResolvedSources<'_>,
    typed_hir: &TypedHir,
) -> bool {
    scalar_expression_is_supported(&if_.condition, resolved, resolved_sources, typed_hir)
        && scalar_value_block_is_supported(&if_.then_block, resolved, resolved_sources, typed_hir)
        && if_.else_block.as_ref().is_some_and(|block| {
            scalar_value_block_is_supported(block, resolved, resolved_sources, typed_hir)
        })
}

fn scalar_comparison_is_supported(binary: &crate::ast::BinaryExpr, typed_hir: &TypedHir) -> bool {
    let Some(operator) = mir_comparison_operator(binary.operator) else {
        return false;
    };
    if let Some(plan) = typed_hir.comparison_plan(binary.operator_span)
        && (plan.method.is_some()
            || plan.left_conversion.is_some()
            || plan.right_conversion.is_some())
    {
        return false;
    }
    let Some(left_ty) = known_expression_type(&binary.left, typed_hir) else {
        return false;
    };
    let Some(right_ty) = known_expression_type(&binary.right, typed_hir) else {
        return false;
    };
    let Some(left) = scalar_type(left_ty, typed_hir) else {
        return false;
    };
    let Some(right) = scalar_type(right_ty, typed_hir) else {
        return false;
    };
    left_ty == right_ty
        && left == right
        && (!matches!(left, super::ScalarType::Bool)
            || matches!(
                operator,
                ComparisonOperator::Equal | ComparisonOperator::NotEqual
            ))
}

pub(super) fn known_expression_type(
    expression: &Expr,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    effective_expression_type(expression.span(), typed_hir)
}

fn effective_expression_type(
    span: crate::source::ByteSpan,
    typed_hir: &TypedHir,
) -> Option<crate::semantic::TyId> {
    let expression = typed_hir.expression(span)?;
    if let Some(ty) = expression.contextual_ty {
        return Some(ty);
    }
    let PartialSemantic::Known(ty) = expression.ty else {
        return None;
    };
    Some(ty)
}

pub(super) fn binding_scalar_type(
    symbol: LocalSymbolId,
    typed_hir: &TypedHir,
) -> Option<super::ScalarType> {
    let ty = typed_hir.binding_type_expr(symbol)?;
    scalar_type(typed_hir.type_id(ty)?, typed_hir)
}

pub(super) fn scalar_type(
    ty: crate::semantic::TyId,
    typed_hir: &TypedHir,
) -> Option<super::ScalarType> {
    match typed_hir.scalar_type(ty)? {
        CheckedScalarType::Integer(crate::integer::IntegerType::I32) => {
            Some(super::ScalarType::I32)
        }
        CheckedScalarType::Integer(crate::integer::IntegerType::U8) => Some(super::ScalarType::U8),
        CheckedScalarType::Integer(crate::integer::IntegerType::Usize) => {
            Some(super::ScalarType::Usize)
        }
        CheckedScalarType::Bool => Some(super::ScalarType::Bool),
        CheckedScalarType::Integer(kind) => Some(super::ScalarType::Integer(kind)),
    }
}
