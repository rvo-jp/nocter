//! Read-only selected environments use the existing metered invocation backing.
use super::{Cell, Executor, Input, Place, Result, Rule, Value};
use crate::state::{CallCase, Type};
impl Executor<'_, '_> {
    pub(super) fn selected_call(
        &mut self,
        frame: usize,
        callee: &Place,
        cases: &[CallCase],
        inputs: &[Input],
        taken: &[Option<Value>],
    ) -> Result<Value> {
        let _input_storage = self
            .memory
            .storage
            .items(inputs.len().checked_add(1).ok_or(Rule::StorageLimit)?)?;
        let mut selected = self.inputs(frame, inputs, taken)?;
        let value = self.read(frame, callee)?;
        let (tag, payload) = value.data().callable().ok_or(Rule::InvalidPlan)?;
        let index = usize::try_from(tag).map_err(|_| Rule::InvalidPlan)?;
        let case = cases.get(index).ok_or(Rule::InvalidPlan)?;
        let (_, environment) = self
            .program
            .types
            .get(case.data)
            .and_then(Type::function_identity)
            .ok_or(Rule::InvalidPlan)?;
        if environment.is_some() {
            let environment = payload
                .data()
                .component(0)
                .ok_or(Rule::InvalidPlan)?
                .clone();
            // Scoped immutable invocation backing grants neither another source owner nor Copy.
            selected.insert(0, Cell::Data(environment));
        }
        self.invoke(case.target, &selected)
    }
}
