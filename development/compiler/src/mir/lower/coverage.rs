//! Authoritative coverage classification for the first scalar MIR route.
//! A body rejected here remains on the legacy route; once accepted, MIR
//! construction and validation errors are authoritative.

use super::expressions::{mir_binary_operator, mir_comparison_operator};
use crate::ast::{AssignmentOperator, AssignmentStmt, BindingStmt, Block, Expr, IfStmt, Stmt};
use crate::literals::decode_integer_literal_value;
use crate::mir::ComparisonOperator;
use crate::resolve::{LocalSymbolId, ResolveOutput};
use crate::typecheck::{CheckedScalarType, PartialSemantic, TypedHir};

#[derive(Debug, Clone, Copy)]
pub(super) enum ScalarStatement<'a> {
    Binding(&'a BindingStmt),
    Assignment(&'a AssignmentStmt),
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

    pub(super) fn is_supported(self, resolved: &ResolveOutput, typed_hir: &TypedHir) -> bool {
        match self {
            Self::Expression(Expr::Call(call)) => {
                scalar_tail_call_is_supported(call, resolved, typed_hir)
            }
            Self::Expression(Expr::If(if_)) => {
                scalar_conditional_is_supported(if_, resolved, typed_hir)
            }
            Self::Expression(expression) => {
                scalar_expression_is_supported(expression, resolved, typed_hir)
            }
            Self::Conditional(if_) => scalar_conditional_is_supported(if_, resolved, typed_hir),
        }
    }

    pub(super) fn result_type(self, typed_hir: &TypedHir) -> Option<crate::semantic::TyId> {
        match self {
            Self::Expression(expression) => known_expression_type(expression, typed_hir),
            Self::Conditional(if_) => {
                known_expression_type(scalar_branch_result(&if_.then_block)?, typed_hir)
            }
        }
    }
}

fn scalar_tail_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, typed_hir)
        && effective_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn scalar_value_call_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    scalar_call_shape_is_supported(call, resolved, typed_hir)
        && intrinsic_expression_type(call.span, typed_hir)
            .and_then(|ty| scalar_type(ty, typed_hir))
            .is_some()
}

fn scalar_call_shape_is_supported(
    call: &crate::ast::CallExpr,
    resolved: &ResolveOutput,
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
            known_expression_type(argument, typed_hir)
                .and_then(|ty| scalar_type(ty, typed_hir))
                .is_some()
                && scalar_expression_is_supported(argument, resolved, typed_hir)
        })
}

impl<'a> ScalarStatement<'a> {
    pub(super) fn is_supported(self, resolved: &ResolveOutput, typed_hir: &TypedHir) -> bool {
        match self {
            Self::Binding(binding) => {
                resolved
                    .local_symbol_id_at_name_span(binding.name_span)
                    .is_some_and(|symbol| {
                        binding_scalar_type(symbol, typed_hir).is_some()
                            && typed_hir
                                .binding_type_expr(symbol)
                                .and_then(|ty| typed_hir.type_id(ty))
                                .is_some()
                    })
                    && known_expression_type(&binding.initializer, typed_hir).is_some()
                    && scalar_expression_is_supported(&binding.initializer, resolved, typed_hir)
            }
            Self::Assignment(assignment) => {
                assignment.operator == AssignmentOperator::Assign
                    && matches!(&assignment.target, Expr::Identifier(identifier) if resolved.local_symbol_for_identifier(identifier).is_some_and(|symbol| binding_scalar_type(symbol.id, typed_hir).is_some()))
                    && known_expression_type(&assignment.value, typed_hir).is_some()
                    && scalar_expression_is_supported(&assignment.value, resolved, typed_hir)
            }
        }
    }
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
        .map(|statement| match statement {
            Stmt::Binding(binding) => Some(ScalarStatement::Binding(binding)),
            Stmt::Assignment(assignment) => Some(ScalarStatement::Assignment(assignment)),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((statements, tail))
}

pub(super) fn scalar_expression_is_supported(
    expression: &Expr,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    match expression {
        Expr::IntegerLiteral(literal) => decode_integer_literal_value(&literal.value).is_some(),
        Expr::BoolLiteral(literal) => matches!(literal.value.as_str(), "true" | "false"),
        Expr::Identifier(identifier) => resolved.local_symbol_for_identifier(identifier).is_some(),
        Expr::Group(group) => {
            scalar_expression_is_supported(&group.expression, resolved, typed_hir)
        }
        Expr::Call(call) => scalar_value_call_is_supported(call, resolved, typed_hir),
        Expr::Binary(binary) => {
            (mir_binary_operator(binary.operator).is_some()
                || scalar_comparison_is_supported(binary, typed_hir))
                && scalar_expression_is_supported(&binary.left, resolved, typed_hir)
                && scalar_expression_is_supported(&binary.right, resolved, typed_hir)
        }
        // A top-level value conditional is selected by `ScalarTail`. Nested
        // conditionals require expression-level CFG construction and must not
        // be claimed as ordinary scalar operands before that route exists.
        Expr::If(_) => false,
        _ => false,
    }
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

pub(super) fn scalar_branch_result(block: &Block) -> Option<&Expr> {
    let (statements, tail) = scalar_body_parts(block)?;
    statements.is_empty().then(|| tail.expression()).flatten()
}

fn scalar_conditional_is_supported(
    if_: &IfStmt,
    resolved: &ResolveOutput,
    typed_hir: &TypedHir,
) -> bool {
    scalar_expression_is_supported(&if_.condition, resolved, typed_hir)
        && scalar_branch_result(&if_.then_block)
            .is_some_and(|result| scalar_expression_is_supported(result, resolved, typed_hir))
        && if_
            .else_block
            .as_ref()
            .and_then(scalar_branch_result)
            .is_some_and(|result| scalar_expression_is_supported(result, resolved, typed_hir))
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
        CheckedScalarType::Integer(crate::integer::IntegerType::Usize) => {
            Some(super::ScalarType::Usize)
        }
        CheckedScalarType::Bool => Some(super::ScalarType::Bool),
        CheckedScalarType::Integer(_) => None,
    }
}
