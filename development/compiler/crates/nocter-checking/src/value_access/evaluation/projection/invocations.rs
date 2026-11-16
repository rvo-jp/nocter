//! Original candidate correspondence; static capability proof and runtime selection stay separate.
use super::{Builder, ExpressionKind, Failure, INVALID, Result, plan};
use crate::value_access::{CallKind, InstantiationFailure, flow::resources::Holder};
use nocter_declarations::value_access::{CodeInstanceKey, DataInstanceKey, InstanceSchema};
use nocter_model::{AccessExpressionId, ValueCode, ValueInputOperation};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct Case {
    data: DataInstanceKey,
    frame: CodeInstanceKey,
}
#[derive(Clone)]
pub(super) enum Selection {
    One(CodeInstanceKey),
    Choice {
        cases: Box<[Case]>,
        result: DataInstanceKey,
    },
}
impl Builder<'_, '_> {
    pub(super) fn invocation(&mut self, id: AccessExpressionId) -> Result<Selection> {
        if let Some(selected) = self.invocations.get(&id) {
            return Ok(selected.clone());
        }
        let site = nocter_model::ValueApplication {
            owner: self.instance.code(),
            call: id,
        };
        let selected =
            match self
                .checked
                .instantiate_invocation(self.instance, site, &self.control, self.work)
            {
                Ok(frame) => Selection::One(frame),
                Err(InstantiationFailure::Unresolved) => self.choice(id)?,
                Err(failure) => return Err(failure.into()),
            };
        self.invocations.insert(id, selected.clone());
        Ok(selected)
    }
    fn choice(&mut self, id: AccessExpressionId) -> Result<Selection> {
        let ExpressionKind::Call {
            kind: CallKind::Value,
            arguments,
            ..
        } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(Failure::UnresolvedContext);
        };
        if arguments
            .first()
            .is_none_or(|argument| argument.operation != ValueInputOperation::Read)
        {
            return Err(Failure::UnresolvedContext);
        }
        let domain = self.checked.instantiate_invocation_domain(
            self.instance,
            nocter_model::ValueApplication {
                owner: self.instance.code(),
                call: id,
            },
            &self.control,
            self.work,
        )?;
        let carrier = self.prepared.get(id, 0).ok_or(INVALID)?;
        let expected = carrier
            .function_representations(self.program)
            .map_err(|_| INVALID)?
            .ok_or(INVALID)?;
        if expected.len() < 2 {
            return Err(Failure::UnresolvedContext);
        }
        let by_witness = expected
            .iter()
            .map(|key| {
                let InstanceSchema::Function(witness) =
                    key.schema(self.program).map_err(|_| INVALID)?
                else {
                    return Err(INVALID);
                };
                Ok((witness, key.clone()))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let result = self
            .program
            .instantiate_closed_data(self.data(id)?)
            .map_err(|_| Failure::UnresolvedContext)?;
        let mut frames = BTreeMap::new();
        for case in domain.cases() {
            self.work.step().map_err(|_| Failure::WorkLimit)?;
            if case.source().kind != CallKind::Value
                || case.source().arguments.len() != arguments.len()
            {
                return Err(INVALID);
            }
            let key = by_witness.get(case.callee()).ok_or(INVALID)?;
            self.check_choice_frame(id, arguments, key, case.frame(), &result)?;
            if frames.get(key).is_some_and(|frame| frame != case.frame()) {
                return Err(Failure::UnresolvedContext);
            }
            frames.insert(key.clone(), case.frame().clone());
        }
        if frames.len() != expected.len() {
            return Err(Failure::UnresolvedContext);
        }
        let cases = expected
            .iter()
            .map(|data| {
                Ok(Case {
                    data: data.clone(),
                    frame: frames.remove(data).ok_or(INVALID)?,
                })
            })
            .collect::<Result<Box<[_]>>>()?;
        Ok(Selection::Choice { cases, result })
    }
    fn check_choice_frame(
        &mut self,
        id: AccessExpressionId,
        arguments: &[crate::value_access::Argument],
        key: &DataInstanceKey,
        frame: &CodeInstanceKey,
        result: &DataInstanceKey,
    ) -> Result<()> {
        let boundary = self.instances.boundary(self.checked, frame, self.work)?;
        let offset = match frame.code() {
            ValueCode::Closure(_) => {
                let environment = key
                    .environment(self.program)
                    .map_err(|_| INVALID)?
                    .ok_or(INVALID)?;
                let input = boundary.inputs.first().ok_or(INVALID)?;
                if !boundary.environment_backing
                    || input.operation != ValueInputOperation::Read
                    || !input
                        .data
                        .has_same_representation(self.program, &environment)
                {
                    return Err(Failure::UnresolvedContext);
                }
                1
            }
            ValueCode::Declaration(_) => 0,
            ValueCode::Expression(_) => return Err(INVALID),
        };
        let explicit = boundary.inputs.get(offset..).ok_or(INVALID)?;
        if !boundary
            .result
            .has_same_representation(self.program, result)
            || explicit.len() + 1 != arguments.len()
        {
            return Err(Failure::UnresolvedContext);
        }
        for (index, input) in explicit.iter().enumerate() {
            let supplied = self.prepared.get(id, index + 1).ok_or(INVALID)?;
            if input.operation != arguments[index + 1].operation
                || !input.data.has_same_representation(self.program, supplied)
            {
                return Err(Failure::UnresolvedContext);
            }
        }
        Ok(())
    }
    pub(super) fn selected_call(
        &mut self,
        id: AccessExpressionId,
        selection: Selection,
    ) -> Result<plan::Operation> {
        let Selection::Choice { cases, .. } = selection else {
            return Err(INVALID);
        };
        let ExpressionKind::Call { arguments, .. } = &self
            .query
            .typed()
            .body()
            .expressions()
            .get(id)
            .ok_or(INVALID)?
            .kind
        else {
            return Err(INVALID);
        };
        let arguments = arguments.clone();
        let mut inputs = Vec::new();
        for (index, argument) in arguments.iter().enumerate().skip(1) {
            let data = self.prepared.get(id, index).ok_or(INVALID)?.clone();
            inputs.push(self.argument_input(id, index, argument, &data, false)?);
        }
        let data = self.prepared.get(id, 0).ok_or(INVALID)?.clone();
        let carrier = self.argument_input(id, 0, &arguments[0], &data, false)?;
        let plan::Input::Read(callee) = carrier else {
            return Err(INVALID);
        };
        // A prepared domain is not evidence that a changing local has encoded a selector.
        // Until incoming local/loop edges are normalized, retain the unqualified boundary.
        let represented = self.types.get_instance(self.program, data)?;
        if self.place_data(&callee)? != represented {
            return Err(Failure::UnresolvedContext);
        }
        let cases = cases
            .iter()
            .map(|case| {
                Ok(plan::CallCase {
                    data: self.types.get_instance(self.program, case.data.clone())?,
                    target: self.instances.intern(case.frame.clone())?,
                })
            })
            .collect::<Result<Box<[_]>>>()?;
        Ok(plan::Operation::SelectedCall {
            destination: self.holder(Holder::Expression(id))?,
            callee,
            cases,
            inputs: inputs.into(),
        })
    }
}
