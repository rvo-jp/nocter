//! Refine original outcome control from proved inactive values without removing source effects.
use super::{Control, IntegrityError, Operation, Point, Terminator};
use crate::value_access::{ExpressionKind, Query, Referent};
use nocter_model::{AccessExpressionId, TypeId};
use std::sync::Arc;
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

impl Control {
    pub(in crate::value_access) fn refine_outcome_domains(&mut self) -> Result<(), IntegrityError> {
        let typed = &self.typed;
        let Some(evidence) = typed.result_evidence.as_ref() else {
            return Ok(());
        };
        let graph = Arc::make_mut(&mut self.graph);
        graph.points = graph.points.try_map(|_, point| {
            let mut point = point.clone();
            if let Terminator::Branch { condition, no, .. } = point.terminator {
                let expression = typed.body().expressions().get(condition).ok_or(INVALID)?;
                if let ExpressionKind::PrimaryCondition { source } = expression.kind {
                    let data = typed.body().referent_data(source).ok_or(INVALID)?;
                    if evidence.excludes_primary(typed.program(), data) {
                        point.terminator = Terminator::Jump(no);
                    }
                }
            } else if let Some(source) = assertion_source(typed, &point)?
                && evidence.excludes_primary(typed.program(), source.data(typed)?)
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

#[derive(Clone, Copy, Debug)]
pub(in crate::value_access) enum OutcomeSource {
    Value(AccessExpressionId),
    Referent(Referent),
}
impl OutcomeSource {
    pub(in crate::value_access) fn data(self, typed: &Query) -> Result<TypeId, IntegrityError> {
        match self {
            Self::Value(source) => typed
                .body()
                .expressions()
                .get(source)
                .ok_or(INVALID)?
                .ty
                .access()
                .map(nocter_model::ValueAccess::data)
                .ok_or(INVALID),
            Self::Referent(source) => typed.body().referent_data(source).ok_or(INVALID),
        }
    }
}

/// The check is the last original action and has a distinct normal continuation.
pub(in crate::value_access) fn assertion_source(
    typed: &Query,
    point: &Point,
) -> Result<Option<OutcomeSource>, IntegrityError> {
    if !matches!(point.terminator, Terminator::Jump(_)) {
        return Ok(None);
    }
    let Some(super::Action {
        operation: Operation::Evaluate(id),
        ..
    }) = point.actions.last()
    else {
        return Ok(None);
    };
    let expression = typed.body().expressions().get(*id).ok_or(INVALID)?;
    Ok(match expression.kind {
        ExpressionKind::RequirePrimary { source } => Some(OutcomeSource::Value(source)),
        ExpressionKind::GuardView {
            source,
            primary: true,
        } => Some(OutcomeSource::Referent(source)),
        _ => None,
    })
}
