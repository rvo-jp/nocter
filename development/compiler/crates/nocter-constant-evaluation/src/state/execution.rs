use super::{
    Action, Effect, Input, Operation, Place, Plan, Program, Site, Terminator, Value,
    memory::{Cell, Frame, Memory},
    validation,
};
use crate::{CompileTimeEvaluationLimits, ConstantScalarType, scalar};
use nocter_model::{AccessCallableId, ConstantValue, ValueInputOperation};
use std::{collections::BTreeSet, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    InvalidPlan,
    ArithmeticFailure,
    StepLimit,
    CallDepthLimit,
    ReachedUnreachable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub rule: Rule,
    pub site: Option<Site>,
    pub code: AccessCallableId,
    pub trace: Box<[Site]>,
}
type Result<T> = std::result::Result<T, Rule>;
/// One deterministic work budget shared by dependency preparation and its closed executions.
pub struct Budget {
    limits: CompileTimeEvaluationLimits,
    remaining: u64,
}
impl Budget {
    #[must_use]
    pub fn new(limits: CompileTimeEvaluationLimits) -> Self {
        Self {
            limits,
            remaining: limits.steps().get(),
        }
    }
    #[must_use]
    pub const fn limits(&self) -> CompileTimeEvaluationLimits {
        self.limits
    }
    /// # Errors
    /// Reports exhausted deterministic work without replenishing a nested request's budget.
    pub fn step(&mut self) -> Result<()> {
        self.remaining = self.remaining.checked_sub(1).ok_or(Rule::StepLimit)?;
        Ok(())
    }
}
/// Executes a closed graph only. Checking owns dependency requests and completed values.
pub struct Executor<'a, 'budget> {
    program: &'a Program,
    budget: &'budget mut Budget,
    suspended_depth: usize,
    memory: Memory,
    site: Option<Site>,
    failed_site: Option<Site>,
    trace: Vec<Site>,
}
impl<'a, 'budget> Executor<'a, 'budget> {
    #[must_use]
    pub fn new(program: &'a Program, budget: &'budget mut Budget, suspended_depth: usize) -> Self {
        Self {
            program,
            budget,
            suspended_depth,
            memory: Memory::default(),
            site: None,
            failed_site: None,
            trace: Vec::new(),
        }
    }
    /// # Errors
    /// Reports a failed closed initializer with its exact operation and call path.
    pub fn evaluate_initializer(
        &mut self,
        code: AccessCallableId,
    ) -> std::result::Result<Arc<Value>, Error> {
        self.site = None;
        self.failed_site = None;
        self.trace.clear();
        let result = if self.program.plans.get(&code).is_some_and(|p| p.initializer) {
            self.invoke(code, &[])
        } else {
            Err(Rule::InvalidPlan)
        };
        result.map_err(|rule| Error {
            rule,
            site: self.failed_site.or(self.site),
            code,
            trace: self.trace.clone().into(),
        })
    }
    fn tick(&mut self) -> Result<()> {
        self.budget.step()
    }
    fn invoke(&mut self, code: AccessCallableId, inputs: &[Cell]) -> Result<Arc<Value>> {
        if self.memory.frames.len() + self.suspended_depth
            >= self.budget.limits.call_depth().get() as usize
        {
            return Err(Rule::CallDepthLimit);
        }
        let plan = self.program.plans.get(&code).ok_or(Rule::InvalidPlan)?;
        if inputs.len() != plan.inputs.len() {
            return Err(Rule::InvalidPlan);
        }
        let mut frame = Frame {
            slots: plan.slots.clone(),
            ..Frame::default()
        };
        for (&(slot, operation), input) in plan.inputs.iter().zip(inputs) {
            match (operation, input) {
                (ValueInputOperation::Take, Cell::Data(value)) => {
                    if !validation::value(self.program, plan.slots[slot].data, value) {
                        return Err(Rule::InvalidPlan);
                    }
                    frame.owned.insert(slot, BTreeSet::new());
                }
                (ValueInputOperation::Read | ValueInputOperation::Edit, Cell::Alias(address)) => {
                    if !validation::value(
                        self.program,
                        plan.slots[slot].data,
                        &self.memory.read(address)?,
                    ) {
                        return Err(Rule::InvalidPlan);
                    }
                }
                _ => return Err(Rule::InvalidPlan),
            }
            frame.cells.insert(slot, input.clone());
        }
        let index = self.memory.frames.len();
        self.memory.frames.push(frame);
        let saved = self.site;
        let result = self.run(code, plan, index);
        if result.is_err() && self.failed_site.is_none() {
            self.failed_site = self.site;
        }
        if result.is_err()
            && let Some(site) = saved
        {
            self.trace.push(site);
        }
        self.site = saved;
        self.memory.frames.pop();
        result
    }
    fn run(&mut self, code: AccessCallableId, plan: &Plan, frame: usize) -> Result<Arc<Value>> {
        let mut point = plan.entry;
        loop {
            let block = plan.points.get(point).ok_or(Rule::InvalidPlan)?;
            for (action, item) in block.actions.iter().enumerate() {
                self.site = Some(Site {
                    code,
                    point,
                    action,
                });
                self.tick()?;
                self.action(frame, item)?;
            }
            self.tick()?;
            match &block.terminator {
                Terminator::Jump(next) => point = *next,
                Terminator::Branch { condition, yes, no } => {
                    let value = self.read(frame, condition)?;
                    let Value::Bool(value) = value.as_ref() else {
                        return Err(Rule::InvalidPlan);
                    };
                    point = if *value { *yes } else { *no };
                }
                Terminator::Return(place) => {
                    let address = self.memory.address(frame, place)?;
                    let owners = &self.memory.frames[frame].owned;
                    if owners.len() != 1
                        || owners
                            .get(&address.slot)
                            .is_none_or(|holes| !holes.is_empty())
                    {
                        return Err(Rule::InvalidPlan);
                    }
                    let value = self.memory.read(&address)?;
                    if !validation::value(self.program, plan.result, &value) {
                        return Err(Rule::InvalidPlan);
                    }
                    return Ok(value);
                }
                Terminator::Unreachable => return Err(Rule::ReachedUnreachable),
            }
        }
    }
    fn read(&self, frame: usize, place: &Place) -> Result<Arc<Value>> {
        self.memory.read(&self.memory.address(frame, place)?)
    }
    fn write(&mut self, frame: usize, place: &Place, value: Arc<Value>) -> Result<()> {
        self.memory
            .store(&self.memory.address(frame, place)?, value)
    }
    fn action(&mut self, frame: usize, action: &Action) -> Result<()> {
        let owners = &self.memory.frames[frame].owned;
        let alternative = action
            .alternatives
            .iter()
            .find(|a| {
                a.owned.len() == owners.len()
                    && a.owned.iter().all(|(slot, holes)| {
                        owners.get(slot).is_some_and(|actual| {
                            holes.iter().map(|p| p.to_vec()).collect::<BTreeSet<_>>() == *actual
                        })
                    })
            })
            .ok_or(Rule::InvalidPlan)?;
        // Taking arguments are protected before their source obligations are retired. Scoped
        // addresses are bound after the frozen before-steps have installed retained temporaries.
        let taken = if let Operation::Call { inputs, .. } = &action.operation {
            inputs
                .iter()
                .map(|input| {
                    if input.operation == ValueInputOperation::Take {
                        self.read(frame, &input.place).map(Some)
                    } else {
                        Ok(None)
                    }
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        for effect in &alternative.before {
            self.tick()?;
            self.effect(frame, effect)?;
        }
        self.operation(frame, &action.operation, &taken)?;
        for effect in &alternative.after {
            self.tick()?;
            self.effect(frame, effect)?;
        }
        Ok(())
    }
    fn operation(
        &mut self,
        frame: usize,
        op: &Operation,
        taken: &[Option<Arc<Value>>],
    ) -> Result<()> {
        let (destination, value) = match op {
            Operation::None => return Ok(()),
            Operation::Literal { destination, value } => (*destination, Arc::new(value.clone())),
            Operation::Alias {
                destination,
                source,
            } => {
                let address = self.memory.address(frame, source)?;
                self.memory.frames[frame]
                    .cells
                    .insert(*destination, Cell::Alias(address));
                return Ok(());
            }
            Operation::Copy {
                destination,
                source,
            } => {
                let value = self.read(frame, source)?;
                return self.write(frame, destination, value);
            }
            Operation::Unary {
                destination,
                operation,
                operand,
            } => {
                let value = self.read(frame, operand)?;
                let (ty, value) = scalar_value(&value)?;
                (
                    *destination,
                    from_scalar(
                        &scalar::unary(*operation, ty, &value, None).map_err(scalar_rule)?,
                    )?,
                )
            }
            Operation::Binary {
                destination,
                operation,
                left,
                right,
            } => {
                let (ty, left) = scalar_value(self.read(frame, left)?.as_ref())?;
                let (_, right) = scalar_value(self.read(frame, right)?.as_ref())?;
                let value = from_scalar(
                    &scalar::value_binary(*operation, ty, &left, &right).map_err(scalar_rule)?,
                )?;
                return self.write(frame, destination, value);
            }
            Operation::Compare {
                destination,
                operation,
                left,
                right,
            } => {
                let (_, left) = scalar_value(self.read(frame, left)?.as_ref())?;
                let (_, right) = scalar_value(self.read(frame, right)?.as_ref())?;
                (
                    *destination,
                    from_scalar(
                        &scalar::compare(*operation, &left, &right, None).map_err(scalar_rule)?,
                    )?,
                )
            }
            Operation::Call {
                destination,
                target,
                inputs,
            } => {
                let inputs = self.inputs(frame, inputs, taken)?;
                (*destination, self.invoke(*target, &inputs)?)
            }
        };
        self.write(frame, &Place::root(destination), value)
    }
    fn inputs(
        &self,
        frame: usize,
        inputs: &[Input],
        taken: &[Option<Arc<Value>>],
    ) -> Result<Vec<Cell>> {
        inputs
            .iter()
            .enumerate()
            .map(|(index, input)| match input.operation {
                ValueInputOperation::Take => Ok(Cell::Data(
                    taken
                        .get(index)
                        .and_then(Option::as_ref)
                        .ok_or(Rule::InvalidPlan)?
                        .clone(),
                )),
                ValueInputOperation::Read | ValueInputOperation::Edit => {
                    Ok(Cell::Alias(self.memory.address(frame, &input.place)?))
                }
            })
            .collect()
    }
    fn effect(&mut self, frame: usize, effect: &Effect) -> Result<()> {
        match effect {
            Effect::Construct(slot) => Ok(self
                .memory
                .construct(&self.memory.address(frame, &Place::root(*slot))?)?),
            Effect::Dispose(place) | Effect::Invoke(place) => Ok(self
                .memory
                .retire(&self.memory.address(frame, place)?, false)?),
            Effect::Release(place) => Ok(self
                .memory
                .retire(&self.memory.address(frame, place)?, true)?),
            Effect::Move {
                source,
                destination,
            }
            | Effect::Replace {
                source,
                destination,
            } => {
                let value = self.read(frame, source)?;
                self.memory
                    .retire(&self.memory.address(frame, source)?, false)?;
                let destination = self.memory.address(frame, destination)?;
                self.memory.store(&destination, value)?;
                if matches!(effect, Effect::Move { .. }) {
                    self.memory.construct(&destination)?;
                }
                Ok(())
            }
            Effect::Assemble {
                destination,
                fields,
            } => {
                let mut values = vec![None; fields.len()];
                for (field, source) in fields {
                    values[*field] = Some(self.read(frame, source)?);
                    self.memory
                        .retire(&self.memory.address(frame, source)?, false)?;
                }
                let values = values
                    .into_iter()
                    .collect::<Option<Box<[_]>>>()
                    .ok_or(Rule::InvalidPlan)?;
                self.write(
                    frame,
                    &Place::root(*destination),
                    Arc::new(Value::Record(values)),
                )?;
                Ok(self
                    .memory
                    .construct(&self.memory.address(frame, &Place::root(*destination))?)?)
            }
        }
    }
}
fn scalar_value(value: &Value) -> Result<(ConstantScalarType, ConstantValue)> {
    match value {
        Value::Bool(v) => Ok((ConstantScalarType::Bool, ConstantValue::Bool(*v))),
        Value::I32(v) => Ok((
            ConstantScalarType::Integer(nocter_model::BuiltinType::I32),
            ConstantValue::Integer(i128::from(*v)),
        )),
        _ => Err(Rule::InvalidPlan),
    }
}
fn from_scalar(value: &ConstantValue) -> Result<Arc<Value>> {
    match value {
        ConstantValue::Bool(v) => Ok(Arc::new(Value::Bool(*v))),
        ConstantValue::Integer(v) => i32::try_from(*v)
            .map(|v| Arc::new(Value::I32(v)))
            .map_err(|_| Rule::ArithmeticFailure),
        _ => Err(Rule::InvalidPlan),
    }
}
const fn scalar_rule(error: scalar::ScalarEvaluationFailure) -> Rule {
    match error {
        scalar::ScalarEvaluationFailure::Arithmetic => Rule::ArithmeticFailure,
        scalar::ScalarEvaluationFailure::InvalidType => Rule::InvalidPlan,
    }
}
