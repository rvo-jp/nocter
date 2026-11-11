use super::{Cells, Equalities, INVALID, equalities::ZERO};
use crate::value_access::flow::{IntegrityError, Operation, Terminator};
use crate::value_access::{Binary, BoundaryAction, ExpressionKind, Query as TypedQuery, Statement};
use nocter_model::{AccessExpressionId, AccessFlowPointId};

fn assign(state: &mut Equalities, to: usize, from: Option<usize>) {
    if let Some(from) = from {
        state.assign(to, from, 0);
    } else {
        state.forget(to);
    }
}
fn forget_locals(cells: &Cells, state: &mut Equalities) {
    for &cell in cells.locals.values() {
        state.forget(cell);
    }
}
fn arithmetic(
    state: &mut Equalities,
    result: usize,
    operation: Binary,
    left: Option<usize>,
    right: Option<usize>,
) {
    if let (Some(left), Some(right)) = (left, right) {
        let offset = state.constant(right).map(i128::from);
        match operation {
            Binary::CheckedAdd => {
                if let Some(offset) = offset {
                    state.assign(result, left, offset);
                    return;
                }
                if let Some(offset) = state.constant(left) {
                    state.assign(result, right, i128::from(offset));
                    return;
                }
            }
            Binary::CheckedSubtract => {
                if let Some(offset) = offset {
                    state.assign(result, left, -offset);
                    return;
                }
                if let Some(difference) = state.difference(left, right) {
                    state.assign(result, ZERO, difference);
                    return;
                }
            }
            _ => {}
        }
    }
    state.forget(result);
}
fn evaluate(
    typed: &TypedQuery,
    cells: &Cells,
    state: &mut Equalities,
    id: AccessExpressionId,
) -> Result<(), IntegrityError> {
    let expression = typed.body().expressions().get(id).ok_or(INVALID)?;
    // A place identifies current storage; evaluating its address never writes that local cell.
    if matches!(expression.kind, ExpressionKind::Place(_)) {
        return Ok(());
    }
    let result = cells.value(id);
    if let Some(result) = result {
        match expression.kind {
            ExpressionKind::Binary { operation, .. } => {
                arithmetic(
                    state,
                    result,
                    operation,
                    cells.operand(id, 0),
                    cells.operand(id, 1),
                );
            }
            ExpressionKind::Boundary { source, .. } => assign(state, result, cells.value(source)),
            ExpressionKind::Move(source) => assign(state, result, cells.local(typed, source)),
            _ => {
                state.forget(result);
            }
        }
        // Singleton normal-value evidence belongs to typing. It grants neither effect omission
        // nor a result on the operation's failure path.
        if let Some(value) = expression.normal_integer() {
            state.assume(result, ZERO, value);
        }
    }
    let consumed = match expression.kind {
        ExpressionKind::Move(source) => Some(source),
        ExpressionKind::Boundary {
            source,
            action: BoundaryAction::TakeValue,
        } => typed
            .body()
            .expressions()
            .get(source)
            .and_then(|value| value.referent(source)),
        _ => None,
    };
    if let Some(local) = consumed.and_then(|source| cells.local(typed, source)) {
        state.forget(local);
    }
    Ok(())
}
pub(super) fn action(
    typed: &TypedQuery,
    cells: &Cells,
    state: &mut Equalities,
    operation: &Operation,
) -> Result<(), IntegrityError> {
    match *operation {
        Operation::Evaluate(id) => evaluate(typed, cells, state, id)?,
        Operation::PrepareOperand {
            expression,
            operand,
            value,
        } => {
            if let Some(&target) = cells.operands.get(&(expression, operand)) {
                assign(state, target, cells.value(value));
            }
        }
        Operation::Forward { source, result } => {
            if let Some(target) = cells.value(result) {
                assign(state, target, cells.value(source));
            }
        }
        Operation::Bind { local, value } => {
            if let Some(&target) = cells.locals.get(&local) {
                assign(state, target, cells.value(value));
            }
        }
        Operation::Assign { target, value } => {
            if let Some(target) = cells.local(typed, target) {
                assign(state, target, cells.value(value));
            } else if !super::owned_base(typed, target) {
                // A distinct owned aggregate cannot alias an owned scalar local. A write
                // through a read/edit base has no such separation proof here.
                forget_locals(cells, state);
            }
        }
        Operation::CompoundAssign {
            statement,
            target,
            value,
        } => {
            let Some(Statement::CompoundAssign { operation, .. }) =
                typed.body().statements().get(statement)
            else {
                return Err(INVALID);
            };
            if let Some(target) = cells.local(typed, target) {
                arithmetic(state, target, *operation, Some(target), cells.value(value));
            } else if !super::owned_base(typed, target) {
                forget_locals(cells, state);
            }
        }
        Operation::CommitCall(id) => {
            // May-writes forget their actual current targets. Independent owned scalar cells
            // and earlier frozen operands survive; unknown aliases/callees remain conservative.
            if let Some(targets) = cells.call_writes.at(id) {
                for &target in targets {
                    if let Some(cell) = cells.local(typed, target) {
                        state.forget(cell);
                    } else if !super::owned_base(typed, target) {
                        forget_locals(cells, state);
                    }
                }
            } else {
                forget_locals(cells, state);
            }
            if let Some(target) = cells.value(id) {
                state.forget(target);
                if let Some(value) = typed
                    .body()
                    .expressions()
                    .get(id)
                    .ok_or(INVALID)?
                    .normal_integer()
                {
                    state.assume(target, ZERO, value);
                }
            }
        }
        Operation::Drop(local) => {
            if let Some(&cell) = cells.locals.get(&local) {
                state.forget(cell);
            }
        }
        Operation::ExitScope { ref locals, .. } => {
            for local in locals {
                if let Some(&cell) = cells.locals.get(local) {
                    state.forget(cell);
                }
            }
        }
        Operation::IndexFailure(_)
        | Operation::BeginEvaluation(_)
        | Operation::EndEvaluation(_)
        | Operation::Condition(_)
        | Operation::BooleanConstant { .. }
        | Operation::BooleanAnswer { .. }
        | Operation::PrepareArgument { .. }
        | Operation::CancelCall { .. }
        | Operation::Discard(_)
        | Operation::StageReturn(_) => {}
    }
    Ok(())
}
pub(super) fn edge(
    typed: &TypedQuery,
    cells: &Cells,
    state: &mut Equalities,
    terminator: Terminator,
    successor: AccessFlowPointId,
) -> Result<(), IntegrityError> {
    if let Terminator::Branch { condition, yes, no } = terminator {
        let expression = typed.body().expressions().get(condition).ok_or(INVALID)?;
        let numeric_truth = if let ExpressionKind::Binary { operation, .. } = expression.kind {
            cells
                .operand(condition, 0)
                .zip(cells.operand(condition, 1))
                .and_then(|(left, right)| state.difference(left, right))
                .and_then(|difference| match operation {
                    Binary::Equal => Some(difference == 0),
                    Binary::NotEqual => Some(difference != 0),
                    Binary::Less => Some(difference < 0),
                    Binary::LessEqual => Some(difference <= 0),
                    Binary::Greater => Some(difference > 0),
                    Binary::GreaterEqual => Some(difference >= 0),
                    _ => None,
                })
        } else {
            None
        };
        if yes != no
            && expression
                .normal_boolean()
                .or(numeric_truth)
                .is_some_and(|value| (value && successor == no) || (!value && successor == yes))
        {
            state.bottom();
            return Ok(());
        }
        if yes != no
            && let ExpressionKind::Binary { operation, .. } = expression.kind
            && ((operation == Binary::Equal && successor == yes)
                || (operation == Binary::NotEqual && successor == no))
            && let (Some(left), Some(right)) =
                (cells.operand(condition, 0), cells.operand(condition, 1))
        {
            state.assume(left, right, 0);
        }
    }
    Ok(())
}
