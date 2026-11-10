use super::{
    Action, Effect, Input, Operation, Place, Plan, Program, Site, Terminator, Value,
    memory::{Cell, Frame, Memory},
    validation,
};
use crate::{
    CompileTimeEvaluationLimits, ConstantScalarType, DependencyComputation, DependencyQuery,
    DependencyQueryError, scalar,
};
use nocter_model::{AccessCallableId, ConstantValue, ValueInputOperation};
use std::{collections::BTreeSet, sync::Arc};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    InvalidPlan,
    ArithmeticFailure,
    StepLimit,
    CallDepthLimit,
    DependencyCycle,
    ReachedUnreachable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub rule: Rule,
    pub site: Option<Site>,
    pub code: AccessCallableId,
    pub trace: Box<[Site]>,
    pub dependency_cycle: Box<[AccessCallableId]>,
}
#[derive(Debug)]
enum Fault {
    Rule(Rule),
    Cycle(Box<[AccessCallableId]>),
}
impl From<Rule> for Fault {
    fn from(rule: Rule) -> Self {
        Self::Rule(rule)
    }
}
impl Fault {
    fn from_query(error: DependencyQueryError<AccessCallableId, Rule>) -> Self {
        match error {
            DependencyQueryError::Cycle(path) => Self::Cycle(path),
            DependencyQueryError::Computation(rule) => Self::Rule(rule),
        }
    }
    fn into_query(self) -> DependencyQueryError<AccessCallableId, Rule> {
        match self {
            Self::Rule(rule) => DependencyQueryError::Computation(rule),
            Self::Cycle(path) => DependencyQueryError::Cycle(path),
        }
    }
}
type Result<T> = std::result::Result<T, Fault>;
type Dependencies = DependencyQuery<AccessCallableId, Value>;
/// One execution transaction. Ordinary calls preserve their state; immutable initializers use
/// the common dependency protocol, including frozen references in unevaluated source branches.
pub struct Executor<'a> {
    runtime: Runtime<'a>,
    dependencies: Dependencies,
}
struct Runtime<'a> {
    program: &'a Program,
    limits: CompileTimeEvaluationLimits,
    remaining: u64,
    memory: Memory,
    pending_initializers: usize,
    site: Option<Site>,
    failed_site: Option<Site>,
    trace: Vec<Site>,
}
impl<'a> Executor<'a> {
    #[must_use]
    pub fn new(program: &'a Program, limits: CompileTimeEvaluationLimits) -> Self {
        Self {
            dependencies: Dependencies::default(),
            runtime: Runtime {
                program,
                limits,
                remaining: 0,
                memory: Memory::default(),
                pending_initializers: 0,
                site: None,
                failed_site: None,
                trace: Vec::new(),
            },
        }
    }
    /// Consume the query authority after required requests, retaining its single frozen value set.
    #[must_use]
    pub fn into_completed(self) -> std::collections::BTreeMap<AccessCallableId, Arc<Value>> {
        self.dependencies.into_completed().into_iter().collect()
    }
    /// # Errors
    /// Returns the failing action/dependency and never publishes a partly initialized value.
    pub fn evaluate_initializer(
        &mut self,
        code: AccessCallableId,
    ) -> std::result::Result<Arc<Value>, Error> {
        self.runtime.remaining = self.runtime.limits.steps().get();
        self.runtime.site = None;
        self.runtime.failed_site = None;
        self.runtime.trace.clear();
        self.runtime
            .constant(&mut self.dependencies, code)
            .map_err(|fault| {
                let (rule, dependency_cycle) = match fault {
                    Fault::Rule(rule) => (rule, Box::new([]) as Box<[_]>),
                    Fault::Cycle(path) => (Rule::DependencyCycle, path),
                };
                Error {
                    rule,
                    site: self.runtime.failed_site.or(self.runtime.site),
                    code,
                    trace: self.runtime.trace.clone().into(),
                    dependency_cycle,
                }
            })
    }
}
impl DependencyComputation<AccessCallableId, Value, Rule> for Runtime<'_> {
    fn compute(
        &mut self,
        query: &mut Dependencies,
        code: AccessCallableId,
    ) -> std::result::Result<Value, DependencyQueryError<AccessCallableId, Rule>> {
        self.initialize(query, code).map_err(Fault::into_query)
    }
}
impl Runtime<'_> {
    fn initialize(&mut self, query: &mut Dependencies, code: AccessCallableId) -> Result<Value> {
        let plan = self.program.plans.get(&code).ok_or(Rule::InvalidPlan)?;
        if !plan.initializer {
            return Err(Rule::InvalidPlan.into());
        }
        if self.memory.frames.len() + self.pending_initializers
            >= self.limits.call_depth().get() as usize
        {
            return Err(Rule::CallDepthLimit.into());
        }
        self.pending_initializers += 1;
        let result: Result<()> = (|| {
            for dependency in &plan.dependencies {
                let saved = self.site;
                self.site = dependency.through.last().copied().or(saved);
                let result = self
                    .tick()
                    .and_then(|()| self.constant(query, dependency.initializer));
                if result.is_err() {
                    if self.failed_site.is_none() {
                        self.failed_site = self.site;
                    }
                    for &site in dependency.through.iter().rev() {
                        if Some(site) != self.failed_site && self.trace.last() != Some(&site) {
                            self.trace.push(site);
                        }
                    }
                }
                self.site = saved;
                result?;
            }
            Ok(())
        })();
        self.pending_initializers -= 1;
        result?;
        self.invoke(query, code, &[]).map(Arc::unwrap_or_clone)
    }
    fn tick(&mut self) -> Result<()> {
        self.remaining = self.remaining.checked_sub(1).ok_or(Rule::StepLimit)?;
        Ok(())
    }
    fn constant(&mut self, query: &mut Dependencies, code: AccessCallableId) -> Result<Arc<Value>> {
        query.resolve(self, code).map_err(Fault::from_query)
    }
    fn invoke(
        &mut self,
        query: &mut Dependencies,
        code: AccessCallableId,
        inputs: &[Cell],
    ) -> Result<Arc<Value>> {
        if self.memory.frames.len() + self.pending_initializers
            >= self.limits.call_depth().get() as usize
        {
            return Err(Rule::CallDepthLimit.into());
        }
        let plan = self.program.plans.get(&code).ok_or(Rule::InvalidPlan)?;
        if inputs.len() != plan.inputs.len() {
            return Err(Rule::InvalidPlan.into());
        }
        let mut frame = Frame {
            slots: plan.slots.clone(),
            ..Frame::default()
        };
        for (&(slot, operation), input) in plan.inputs.iter().zip(inputs) {
            match (operation, input) {
                (ValueInputOperation::Take, Cell::Data(value)) => {
                    if !validation::value(self.program, plan.slots[slot].data, value) {
                        return Err(Rule::InvalidPlan.into());
                    }
                    frame.owned.insert(slot, BTreeSet::new());
                }
                (ValueInputOperation::Read | ValueInputOperation::Edit, Cell::Alias(address)) => {
                    if !validation::value(
                        self.program,
                        plan.slots[slot].data,
                        &self.memory.read(address)?,
                    ) {
                        return Err(Rule::InvalidPlan.into());
                    }
                }
                _ => return Err(Rule::InvalidPlan.into()),
            }
            frame.cells.insert(slot, input.clone());
        }
        let index = self.memory.frames.len();
        self.memory.frames.push(frame);
        let saved = self.site;
        let result = self.run(query, code, plan, index);
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
    fn run(
        &mut self,
        query: &mut Dependencies,
        code: AccessCallableId,
        plan: &Plan,
        frame: usize,
    ) -> Result<Arc<Value>> {
        let mut point = plan.entry;
        loop {
            let block = plan.points.get(point).ok_or(Rule::InvalidPlan)?;
            for (action, item) in block.actions.iter().enumerate() {
                self.site = Some(Site::Action {
                    code,
                    point,
                    action,
                });
                self.tick()?;
                self.action(query, frame, item)?;
            }
            self.tick()?;
            match &block.terminator {
                Terminator::Jump(next) => point = *next,
                Terminator::Branch { condition, yes, no } => {
                    let value = self.read(frame, condition)?;
                    let Value::Bool(value) = value.as_ref() else {
                        return Err(Rule::InvalidPlan.into());
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
                        return Err(Rule::InvalidPlan.into());
                    }
                    let value = self.memory.read(&address)?;
                    if !validation::value(self.program, plan.result, &value) {
                        return Err(Rule::InvalidPlan.into());
                    }
                    return Ok(value);
                }
                Terminator::Unreachable => return Err(Rule::ReachedUnreachable.into()),
            }
        }
    }
    fn read(&self, frame: usize, place: &Place) -> Result<Arc<Value>> {
        Ok(self.memory.read(&self.memory.address(frame, place)?)?)
    }
    fn write(&mut self, frame: usize, place: &Place, value: Arc<Value>) -> Result<()> {
        Ok(self
            .memory
            .store(&self.memory.address(frame, place)?, value)?)
    }
    fn action(&mut self, query: &mut Dependencies, frame: usize, action: &Action) -> Result<()> {
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
        self.operation(query, frame, &action.operation, &taken)?;
        for effect in &alternative.after {
            self.tick()?;
            self.effect(frame, effect)?;
        }
        Ok(())
    }
    fn operation(
        &mut self,
        query: &mut Dependencies,
        frame: usize,
        op: &Operation,
        taken: &[Option<Arc<Value>>],
    ) -> Result<()> {
        let (destination, value) = match op {
            Operation::None => return Ok(()),
            Operation::Literal { destination, value } => (*destination, Arc::new(value.clone())),
            Operation::Constant {
                destination,
                initializer,
            } => (*destination, self.constant(query, *initializer)?),
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
                (*destination, self.invoke(query, *target, &inputs)?)
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
        _ => Err(Rule::InvalidPlan.into()),
    }
}
fn from_scalar(value: &ConstantValue) -> Result<Arc<Value>> {
    match value {
        ConstantValue::Bool(v) => Ok(Arc::new(Value::Bool(*v))),
        ConstantValue::Integer(v) => i32::try_from(*v)
            .map(|v| Arc::new(Value::I32(v)))
            .map_err(|_| Rule::ArithmeticFailure.into()),
        _ => Err(Rule::InvalidPlan.into()),
    }
}
const fn scalar_rule(error: scalar::ScalarEvaluationFailure) -> Rule {
    match error {
        scalar::ScalarEvaluationFailure::Arithmetic => Rule::ArithmeticFailure,
        scalar::ScalarEvaluationFailure::InvalidType => Rule::InvalidPlan,
    }
}
