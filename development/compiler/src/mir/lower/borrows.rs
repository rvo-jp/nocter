//! Construction of explicit MIR loans and their borrow-value locals.

use super::context::LoweringContext;
use super::{BuildError, SemanticInputs};
use crate::ast::Expr;
use crate::mir::{BorrowKind, Loan, LoanId, Origin, Place, ScopeId, Statement};

pub(super) fn expression_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    let Expr::Borrow(borrow) = expression.without_groups() else {
        return false;
    };
    semantic
        .typed_hir
        .coercion_plan(expression.span())
        .is_none()
        && source_place_is_supported(&borrow.expression, semantic)
}

pub(super) fn source_place_is_supported(expression: &Expr, semantic: SemanticInputs<'_>) -> bool {
    match expression.without_groups() {
        Expr::Identifier(identifier) => semantic
            .resolved
            .local_symbol_for_identifier(identifier)
            .is_some(),
        Expr::Member(member) => super::projections::field_is_supported(member, semantic),
        Expr::Index(index) => super::indexes::is_supported(index, semantic),
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
    let source = lower_source_place(context, expression, scope)?;
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
    let source = Place::local(
        *context
            .locals_by_symbol
            .get(&source_symbol)
            .ok_or(BuildError::MissingLocalSymbol)?,
    );
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
) -> Result<(), BuildError> {
    let Expr::Borrow(borrow) = expression.without_groups() else {
        return Err(BuildError::UnsupportedClaimedExpression);
    };
    if borrow.is_readwrite != readwrite {
        return Err(BuildError::UnsupportedClaimedExpression);
    }
    let source = lower_source_place(context, &borrow.expression, scope)?;
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

fn lower_source_place(
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
            Ok(Place::local(
                *context
                    .locals_by_symbol
                    .get(&symbol)
                    .ok_or(BuildError::MissingLocalSymbol)?,
            ))
        }
        Expr::Member(member) => super::projections::lower_borrow_field_place(
            member,
            context.semantic,
            &context.locals_by_symbol,
            &mut context.projections,
            &mut context.drop_plans,
        )
        .map(|(place, _)| place),
        Expr::Index(index) => {
            super::indexes::lower_place(context, index, scope).map(|(place, _)| place)
        }
        _ => Err(BuildError::UnsupportedClaimedExpression),
    }
}
