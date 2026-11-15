//! Refine original outcome control from proved inactive values without removing source effects.
use super::{Control, IntegrityError, Operation, Point, Terminator};
use crate::value_access::{ExpressionKind, Query, results::Evidence};
use std::sync::Arc;
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

impl Control {
    pub(in crate::value_access) fn refine_outcome_domains(&mut self) -> Result<(), IntegrityError> {
        let typed = &self.typed;
        let Some(evidence) = typed.result_evidence.as_ref() else {
            return Ok(());
        };
        self.graph.points = self.graph.points.try_map(|_, point| {
            let mut point = point.clone();
            if let Terminator::Branch { condition, no, .. } = point.terminator {
                let expression = typed.body().expressions().get(condition).ok_or(INVALID)?;
                if let ExpressionKind::PrimaryCondition { source } = expression.kind {
                    let data = typed.body().referent_data(source).ok_or(INVALID)?;
                    if evidence.excludes_primary(typed.program(), data) {
                        point.terminator = Terminator::Jump(no);
                    }
                }
            } else if matches!(point.terminator, Terminator::Jump(_))
                && excluded_assertion(typed, evidence, &point)?
            {
                point.terminator = Terminator::Unreachable;
            }
            Ok(point)
        })?;
        for nested in Arc::make_mut(&mut self.typed).closures.values_mut() {
            nested.refine_outcome_domains()?;
        }
        Ok(())
    }
}

fn excluded_assertion(
    typed: &Query,
    evidence: &Evidence,
    point: &Point,
) -> Result<bool, IntegrityError> {
    let Some(super::Action {
        operation: Operation::Evaluate(id),
        ..
    }) = point.actions.last()
    else {
        return Ok(false);
    };
    let expression = typed.body().expressions().get(*id).ok_or(INVALID)?;
    let data = match expression.kind {
        ExpressionKind::RequirePrimary { source } => typed
            .body()
            .expressions()
            .get(source)
            .ok_or(INVALID)?
            .ty
            .access()
            .ok_or(INVALID)?
            .data(),
        ExpressionKind::GuardView {
            source,
            primary: true,
        } => typed.body().referent_data(source).ok_or(INVALID)?,
        _ => return Ok(false),
    };
    Ok(evidence.excludes_primary(typed.program(), data))
}
