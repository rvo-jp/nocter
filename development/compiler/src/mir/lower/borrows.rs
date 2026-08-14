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
            context
                .places_by_symbol
                .get(&symbol)
                .copied()
                .ok_or(BuildError::MissingLocalSymbol)
        }
        Expr::Member(member) => super::projections::lower_borrow_field_place(
            member,
            context.semantic,
            &context.places_by_symbol,
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
