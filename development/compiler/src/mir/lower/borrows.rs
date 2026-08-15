//! Construction of explicit MIR loans and their borrow-value locals.

use super::context::LoweringContext;
use super::{BuildError, SemanticInputs};
use crate::ast::Expr;
use crate::mir::{BorrowKind, Loan, LoanId, LoanLifetime, Origin, Place, ScopeId, Statement};

pub(super) fn place_argument(
    context: &mut LoweringContext<'_>,
    source: Place,
    inner: &crate::ast::TypeExpr,
    readwrite: bool,
    scope: ScopeId,
    origin: Origin,
) -> Result<crate::mir::CallArgument, BuildError> {
    let borrow_type = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
        span: context.scopes[scope.index()].span,
        is_readwrite: readwrite,
        inner: Box::new(inner.clone()),
    });
    let ty = context
        .semantic
        .typed_hir
        .type_id(&borrow_type)
        .ok_or(BuildError::MissingMethodReceiverType)?;
    let local = crate::mir::LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::borrow(
        ty,
        readwrite,
        crate::mir::LocalStorage::Local,
        crate::mir::LocalOrigin::Desugared(context.scopes[scope.index()].span),
        scope,
    ));
    lower_place_to_local(context, local, source, readwrite, scope, origin)?;
    Ok(crate::mir::CallArgument {
        operand: if readwrite {
            crate::mir::Operand::Move(Place::local(local))
        } else {
            crate::mir::Operand::Copy(Place::local(local))
        },
        ty,
        representation: crate::mir::ValueRepresentation::Borrow,
    })
}

pub(super) fn expression_argument(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
    inner: &crate::ast::TypeExpr,
    readwrite: bool,
    scope: ScopeId,
    origin: Origin,
) -> Result<crate::mir::CallArgument, BuildError> {
    let borrow_type = crate::ast::TypeExpr::Borrow(crate::ast::BorrowType {
        span: expression.span(),
        is_readwrite: readwrite,
        inner: Box::new(inner.clone()),
    });
    let ty = context
        .semantic
        .typed_hir
        .type_id(&borrow_type)
        .ok_or(BuildError::MissingMethodReceiverType)?;
    let local = crate::mir::LocalId::from_index(context.locals.len());
    context.locals.push(crate::mir::Local::borrow(
        ty,
        readwrite,
        crate::mir::LocalStorage::Local,
        crate::mir::LocalOrigin::Desugared(expression.span()),
        scope,
    ));
    lower_implicit_to_local(context, local, expression, readwrite, scope, origin)?;
    Ok(crate::mir::CallArgument {
        operand: if readwrite {
            crate::mir::Operand::Move(Place::local(local))
        } else {
            crate::mir::Operand::Copy(Place::local(local))
        },
        ty,
        representation: crate::mir::ValueRepresentation::Borrow,
    })
}

pub(super) fn expression_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Expr::Borrow(borrow) = expression.without_groups() else {
        return false;
    };
    semantic
        .typed_hir
        .coercion_plan(expression.span())
        .is_none()
        && (source_place_is_supported(&borrow.expression, semantic)
            || !borrow.is_readwrite
                && readonly_temporary_is_supported(&borrow.expression, semantic))
}

pub(super) fn readonly_temporary_is_supported(
    expression: &Expr,
    semantic: SemanticInputs<'_>,
) -> bool {
    let Some(ty) =
        super::source_model::intrinsic_expression_type(expression.span(), semantic.typed_hir)
            .or_else(|| super::source_model::known_expression_type(expression, semantic.typed_hir))
    else {
        return false;
    };
    let Some(representation) = super::source_model::value_representation(ty, semantic) else {
        return false;
    };
    super::source_model::value_expression_is_supported(expression, representation, semantic)
}

pub(super) fn source_place_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    match expression.without_groups() {
        Expr::Identifier(identifier) => semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .is_some(),
        Expr::Member(member) => {
            super::projections::field_is_supported(member, semantic)
                || super::source_model::staged_scalar_field_is_supported(member, semantic)
        }
        Expr::Index(index) => {
            super::indexes::is_supported(index, semantic)
                || super::indexes::view_is_supported(index, semantic)
        }
        _ => false,
    }
}

pub(super) fn lower_implicit_to_local(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    expression: &Expr,
    readwrite: bool,
    scope: ScopeId,
    origin: Origin,
) -> Result<(), BuildError> {
    let source = lower_source_or_readonly_temporary(context, expression, readwrite, scope)?;
    let loan = LoanId::from_index(context.loans.len());
    context.loans.push(Loan {
        id: loan,
        source,
        destination,
        kind: if readwrite {
            BorrowKind::Readwrite
        } else {
            BorrowKind::Readonly
        },
        scope,
        lifetime: LoanLifetime::Call,
    });
    context
        .control_flow
        .push_statement(Statement::BeginLoan { loan, origin })
}

pub(super) fn lower_symbol_to_local(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    source_symbol: crate::resolve::LocalSymbolId,
    readwrite: bool,
    scope: ScopeId,
    origin: Origin,
) -> Result<(), BuildError> {
    let source = *context
        .places_by_symbol
        .get(&source_symbol)
        .ok_or(BuildError::MissingLocalSymbol)?;
    let loan = LoanId::from_index(context.loans.len());
    context.loans.push(Loan {
        id: loan,
        source,
        destination,
        kind: if readwrite {
            BorrowKind::Readwrite
        } else {
            BorrowKind::Readonly
        },
        scope,
        lifetime: LoanLifetime::Scope,
    });
    context
        .control_flow
        .push_statement(Statement::BeginLoan { loan, origin })
}

pub(super) fn lower_place_to_local(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    source: Place,
    readwrite: bool,
    scope: ScopeId,
    origin: Origin,
) -> Result<(), BuildError> {
    let loan = LoanId::from_index(context.loans.len());
    context.loans.push(Loan {
        id: loan,
        source,
        destination,
        kind: if readwrite {
            BorrowKind::Readwrite
        } else {
            BorrowKind::Readonly
        },
        scope,
        lifetime: LoanLifetime::Call,
    });
    context
        .control_flow
        .push_statement(Statement::BeginLoan { loan, origin })
}

pub(super) fn lower_to_local(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    expression: &Expr,
    readwrite: bool,
    scope: ScopeId,
    lifetime: LoanLifetime,
) -> Result<(), BuildError> {
    if context.lower_coercion_to_local(destination, expression, scope)? {
        return Ok(());
    }
    lower_to_local_without_coercion(context, destination, expression, readwrite, scope, lifetime)
}

pub(super) fn lower_to_local_without_coercion(
    context: &mut LoweringContext<'_>,
    destination: crate::mir::LocalId,
    expression: &Expr,
    readwrite: bool,
    scope: ScopeId,
    lifetime: LoanLifetime,
) -> Result<(), BuildError> {
    let Expr::Borrow(borrow) = expression.without_groups() else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    if borrow.is_readwrite != readwrite {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let source = lower_source_or_readonly_temporary(context, &borrow.expression, readwrite, scope)?;
    let loan = LoanId::from_index(context.loans.len());
    context.loans.push(Loan {
        id: loan,
        source,
        destination,
        kind: if readwrite {
            BorrowKind::Readwrite
        } else {
            BorrowKind::Readonly
        },
        scope,
        lifetime,
    });
    let origin = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .map(|expression| Origin::Expression(expression.id))
        .ok_or(BuildError::MissingTypedExpression)?;
    context
        .control_flow
        .push_statement(Statement::BeginLoan { loan, origin })
}

fn lower_source_or_readonly_temporary(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
    readwrite: bool,
    scope: ScopeId,
) -> Result<Place, BuildError> {
    if matches!(
        expression.without_groups(),
        Expr::Identifier(_) | Expr::Member(_) | Expr::Index(_)
    ) {
        match lower_source_place(context, expression, scope) {
            Ok(place) => return Ok(place),
            Err(error) if readwrite => {
                return Err(error.context("lower readwrite borrow source place"));
            }
            Err(_) => {}
        }
    }
    if readwrite || !readonly_temporary_is_supported(expression, context.semantic) {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let typed = context
        .semantic
        .typed_hir
        .expression(expression.span())
        .ok_or(BuildError::MissingTypedExpression)?;
    let ty = match typed.ty {
        crate::typecheck::PartialSemantic::Known(ty) => ty,
        crate::typecheck::PartialSemantic::Error => {
            return Err(BuildError::MissingTypedExpression);
        }
    };
    let representation = super::source_model::value_representation(ty, context.semantic)
        .ok_or(BuildError::UnsupportedClaimedExpression)?;
    let local = context.local_for_type(ty, crate::mir::LocalOrigin::Temporary(typed.id), scope)?;
    context.lower_value_to_place(local, expression, ty, representation, scope)?;
    Ok(Place::local(local))
}

pub(super) fn lower_source_place(
    context: &mut LoweringContext<'_>,
    expression: &Expr,
    scope: ScopeId,
) -> Result<Place, BuildError> {
    match expression.without_groups() {
        Expr::Identifier(identifier) => {
            let symbol = context
                .semantic
                .resolved
                .local_symbol_for_identifier(identifier)
                .map(|symbol| symbol.id)
                .ok_or(BuildError::MissingLocalSymbol)?;
            context
                .places_by_symbol
                .get(&symbol)
                .copied()
                .ok_or(BuildError::MissingLocalSymbol)
        }
        Expr::Member(member) => {
            if super::projections::field_is_supported(member, context.semantic) {
                super::projections::lower_borrow_field_place(
                    member,
                    context.semantic,
                    &context.places_by_symbol,
                    &mut context.projections,
                    &mut context.drop_plans,
                )
                .map(|(place, _)| place)
            } else {
                let result_ty = context
                    .semantic
                    .typed_hir
                    .expression(member.span)
                    .and_then(|expression| match expression.ty {
                        crate::typecheck::PartialSemantic::Known(ty) => Some(ty),
                        crate::typecheck::PartialSemantic::Error => None,
                    })
                    .ok_or(BuildError::MissingTypedExpression)?;
                let scalar =
                    super::source_model::scalar_type(result_ty, context.semantic.typed_hir)
                        .ok_or(BuildError::UnsupportedClaimedExpression)?;
                context.lower_value_member_source(
                    member,
                    result_ty,
                    crate::mir::ValueRepresentation::Scalar(scalar),
                    scope,
                )
            }
        }
        Expr::Index(index) => {
            super::indexes::lower_place(context, index, scope).map(|(place, _)| place)
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}
