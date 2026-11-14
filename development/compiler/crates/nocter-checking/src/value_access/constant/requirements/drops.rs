//! Explicit drop contracts retain their original pre-retirement source selections.
use super::{BoundCheckFailure, INVALID, Rule, Site, Statement, Violation, evaluation, flow};
use crate::value_access::{ValueSourceBoundary, ValueSourceSelection, ValueSourceSlot, results};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{TypeId, ValuePath};
use std::collections::BTreeMap;
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(super) struct Condition {
    violation: Violation,
    selections: Box<[flow::Location]>,
}
impl Condition {
    pub(super) fn violation(&self) -> Violation {
        self.violation
    }
    pub(super) fn data(&self) -> Result<TypeId, flow::IntegrityError> {
        let Rule::UnsupportedCleanup(data) = self.violation.rule() else {
            return Err(INVALID);
        };
        Ok(data)
    }

    pub(super) fn supports(
        &self,
        body: &flow::Query,
        frame: &CodeInstanceKey,
        work: &mut WorkBudget,
        domain: &mut evaluation::Types,
    ) -> Result<bool, BoundCheckFailure> {
        let query = body.typed();
        let program = query.program();
        if query.code() != frame.code() {
            return Err(INVALID.into());
        }
        let Site::Statement(statement) = self.violation.site else {
            return Err(INVALID.into());
        };
        let Some(Statement::Drop(local)) = query.body().statements().get(statement) else {
            return Err(INVALID.into());
        };
        let data = self.data()?;
        if query.body().locals().get(*local).ok_or(INVALID)?.data != data {
            return Err(INVALID.into());
        }
        // A dormant authored statement has no source operation. Preserve its definition-contract
        // obligation rather than inventing an executed value or dropping lexical const checks.
        if self.selections.is_empty() {
            return super::super::supports(program, data, domain, Some(frame)).map_err(Into::into);
        }
        let evidence = query.result_evidence.as_ref().ok_or(INVALID)?;
        for &at in &self.selections {
            let action = body
                .graph()
                .points()
                .get(at.point)
                .and_then(|point| point.actions.get(at.action))
                .ok_or(INVALID)?;
            if action.operation != flow::Operation::Drop(*local)
                || !matches!((action.origin, self.violation.site),
                    (flow::Origin::Statement(actual), Site::Statement(expected)) if actual == expected)
            {
                return Err(INVALID.into());
            }
            let key = match program.instantiate_closed_data(data) {
                Ok(key) => key,
                Err(_) => results::instantiate_stored_data(
                    program,
                    evidence,
                    frame,
                    &ValueSourceSelection {
                        boundary: ValueSourceBoundary {
                            point: at.point,
                            action: at.action,
                        },
                        slot: ValueSourceSlot::Local(*local),
                        path: ValuePath::default(),
                    },
                    data,
                    work,
                )
                .map_err(|failure| BoundCheckFailure::Resource { at, failure })?,
            };
            if !domain.supports_instance(program, key)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
pub(super) fn collect(
    body: &flow::Query,
    domain: &mut evaluation::Types,
) -> Result<Vec<Condition>, flow::IntegrityError> {
    let mut selections = BTreeMap::new();
    for (point, block) in body.graph().points().iter() {
        if !body.initialization().reachable(point) {
            continue;
        }
        for (action, operation) in block.actions.iter().enumerate() {
            if let flow::Operation::Drop(local) = operation.operation {
                let flow::Origin::Statement(statement) = operation.origin else {
                    return Err(INVALID);
                };
                if !matches!(body.typed().body().statements().get(statement), Some(Statement::Drop(expected)) if *expected == local)
                {
                    return Err(INVALID);
                }
                selections
                    .entry(statement)
                    .or_insert_with(Vec::new)
                    .push(flow::Location { point, action });
            }
        }
    }
    let mut conditions = Vec::new();
    for (id, statement) in body.typed().body().statements().iter() {
        if let Statement::Drop(local) = *statement {
            let data = body.typed().body().locals().get(local).ok_or(INVALID)?.data;
            if !domain.supports(body.typed().program(), data)? {
                conditions.push(Condition {
                    violation: Violation {
                        site: Site::Statement(id),
                        rule: Rule::UnsupportedCleanup(data),
                    },
                    selections: selections.remove(&id).unwrap_or_default().into(),
                });
            }
        }
    }
    Ok(conditions)
}
