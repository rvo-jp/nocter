//! Expression-domain conditions select original operand and result-definition occurrences.
use super::{BoundCheckFailure, INVALID, Rule, Site, Violation, evaluation, flow};
use crate::value_access::{ValueSourceId, ValueSourceSlot, results};
use nocter_constant_evaluation::state::WorkBudget;
use nocter_declarations::value_access::CodeInstanceKey;
use nocter_model::{AccessExpressionId, TypeId};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub(super) struct Condition {
    violation: Violation,
    selections: Box<[(flow::Location, ValueSourceId)]>,
}
impl Condition {
    pub(super) fn violation(&self) -> Violation {
        self.violation
    }
    pub(super) fn data(&self, body: &flow::Query) -> Result<TypeId, flow::IntegrityError> {
        let (Site::Expression(expression), Rule::UnsupportedData(data)) =
            (self.violation.site, self.violation.rule())
        else {
            return Err(INVALID);
        };
        if body
            .typed()
            .body()
            .expressions()
            .get(expression)
            .ok_or(INVALID)?
            .ty
            .normal()
            .map(nocter_model::ValueAccess::data)
            != Some(data)
        {
            return Err(INVALID);
        }
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
        if query.code() != frame.code() {
            return Err(INVALID.into());
        }
        let data = self.data(body)?;
        let program = query.program();
        // Dormant source retains its authored contract, not a counterfeit runtime occurrence.
        if self.selections.is_empty() {
            return super::super::supports(program, data, domain, None).map_err(Into::into);
        }
        let evidence = query.result_evidence.as_ref().ok_or(INVALID)?;
        for &(at, source) in &self.selections {
            let key = match program.instantiate_closed_data(data) {
                Ok(key) => key,
                Err(_) => {
                    results::instantiate_value_data(program, evidence, frame, source, data, work)
                        .map_err(|failure| BoundCheckFailure::Data { at, failure })?
                }
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
) -> Result<BTreeMap<AccessExpressionId, Condition>, flow::IntegrityError> {
    let query = body.typed();
    let mut conditions = BTreeMap::new();
    let mut projections = super::super::representation::Projections::default();
    for (id, expression) in query.body().expressions().iter() {
        if let Some(value) = expression.ty.normal()
            && !domain.supports(query.program(), value.data())?
            && !projections.covered(body, id, domain)?
        {
            conditions.insert(
                id,
                Condition {
                    violation: Violation {
                        site: Site::Expression(id),
                        rule: Rule::UnsupportedData(value.data()),
                    },
                    selections: Box::new([]),
                },
            );
        }
    }
    if conditions.is_empty() {
        return Ok(conditions);
    }
    let sources = query
        .result_evidence
        .as_ref()
        .and_then(|evidence| evidence.value_sources(query.code()))
        .and_then(|sources| sources.flow())
        .ok_or(INVALID)?;
    let mut selections = BTreeMap::<_, BTreeSet<_>>::new();
    for ((at, expression), source) in sources.operands() {
        if conditions.contains_key(&expression) {
            selections
                .entry(expression)
                .or_default()
                .insert((at, source));
        }
    }
    for ((at, slot), source) in sources.definitions() {
        if let ValueSourceSlot::Expression(expression) = slot
            && conditions.contains_key(&expression)
        {
            selections
                .entry(expression)
                .or_default()
                .insert((at, source));
        }
    }
    for (id, condition) in &mut conditions {
        let mut seen = BTreeSet::new();
        condition.selections = selections
            .remove(id)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, source)| seen.insert(*source))
            .collect();
    }
    // A dormant excluded-value descriptor needs no evaluator value. Actual operand/definition
    // occurrences still require ordinary representation checks and cannot use this exemption.
    let evidence = query.result_evidence.as_ref().ok_or(INVALID)?;
    let mut excluded = Vec::new();
    for (&id, condition) in &conditions {
        if condition.selections.is_empty()
            && evidence.excludes_value(query.program(), condition.data(body)?)
        {
            excluded.push(id);
        }
    }
    for id in excluded {
        conditions.remove(&id);
    }
    Ok(conditions)
}
