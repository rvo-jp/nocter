//! Concrete data demand from the frozen ABI, reached operations and actual resource obligations.
use super::Failure;
use nocter_checking::value_access::{
    ExpressionKind, Referent, TypedBody,
    flow::{self, Operation},
};
use nocter_model::{AccessExpressionId, AccessLocalId, TypeId};
use std::collections::BTreeSet;

pub(super) fn data(checked: &flow::Query) -> Result<BTreeSet<TypeId>, Failure> {
    let body = checked.typed().body();
    let mut needed = Needed {
        body,
        data: BTreeSet::from([checked.typed().result_data()]),
    };
    for input in body
        .inputs()
        .environment()
        .into_iter()
        .chain(body.inputs().explicit().iter().copied())
    {
        needed.local(input.local)?;
    }
    for (point, block) in checked.graph().points().iter() {
        if !checked.initialization().reachable(point) {
            continue;
        }
        for (action, event) in block.actions.iter().enumerate() {
            needed.operation(&event.operation)?;
            for transition in checked
                .access()
                .resources()
                .at(flow::Location { point, action })
            {
                for step in transition.steps() {
                    step.try_map_payloads::<Failure>(|payload| {
                        needed.data.insert(payload.data());
                        Ok(payload)
                    })?;
                }
            }
        }
    }
    Ok(needed.data)
}
struct Needed<'a> {
    body: &'a TypedBody,
    data: BTreeSet<TypeId>,
}
impl Needed<'_> {
    fn local(&mut self, local: AccessLocalId) -> Result<(), Failure> {
        self.data.insert(
            self.body
                .locals()
                .get(local)
                .ok_or(Failure::InvalidAcceptedUnit)?
                .data,
        );
        Ok(())
    }
    fn referent(&mut self, referent: Referent) -> Result<(), Failure> {
        self.data.insert(
            self.body
                .referent_data(referent)
                .ok_or(Failure::InvalidAcceptedUnit)?,
        );
        if let Some(base) = self.body.referent_base(referent) {
            self.local(
                self.body
                    .places()
                    .get(base)
                    .ok_or(Failure::InvalidAcceptedUnit)?
                    .local,
            )?;
        }
        Ok(())
    }
    fn expression(&mut self, id: AccessExpressionId) -> Result<(), Failure> {
        let expression = self
            .body
            .expressions()
            .get(id)
            .ok_or(Failure::InvalidAcceptedUnit)?;
        if let Some(access) = expression.ty.access() {
            self.data.insert(access.data());
        }
        if let Some(referent) = expression.referent(id) {
            self.referent(referent)?;
        }
        Ok(())
    }
    fn operation(&mut self, operation: &Operation) -> Result<(), Failure> {
        match *operation {
            Operation::Evaluate(id)
            | Operation::Condition(id)
            | Operation::Discard(id)
            | Operation::StageReturn(id)
            | Operation::CommitCall(id) => self.expression(id)?,
            Operation::PrepareOperand { value, .. } => self.expression(value)?,
            Operation::BooleanConstant { result, .. } => self.expression(result)?,
            Operation::Forward { source, result } | Operation::BooleanAnswer { source, result } => {
                self.expression(source)?;
                self.expression(result)?;
            }
            Operation::PrepareArgument { call, argument } => {
                let ExpressionKind::Call { arguments, .. } = &self
                    .body
                    .expressions()
                    .get(call)
                    .ok_or(Failure::InvalidAcceptedUnit)?
                    .kind
                else {
                    return Err(Failure::InvalidAcceptedUnit);
                };
                self.expression(
                    arguments
                        .get(argument)
                        .ok_or(Failure::InvalidAcceptedUnit)?
                        .value,
                )?;
            }
            Operation::Bind { local, value } => {
                self.local(local)?;
                self.expression(value)?;
            }
            Operation::Assign { target, value }
            | Operation::CompoundAssign { target, value, .. } => {
                self.referent(target)?;
                self.expression(value)?;
            }
            Operation::Drop(local) => self.local(local)?,
            Operation::IndexFailure(id) => {
                let ExpressionKind::Index { base, index, .. } = self
                    .body
                    .expressions()
                    .get(id)
                    .ok_or(Failure::InvalidAcceptedUnit)?
                    .kind
                else {
                    return Err(Failure::InvalidAcceptedUnit);
                };
                self.expression(base)?;
                self.expression(index)?;
            }
            // Beginning evaluation never proves that its result exists. Cancellation/scope exit
            // contribute only their actual initialized resource obligations above.
            Operation::BeginEvaluation(_)
            | Operation::EndEvaluation(_)
            | Operation::CancelCall { .. }
            | Operation::ExitScope { .. } => {}
        }
        Ok(())
    }
}
