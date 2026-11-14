use super::{
    Action, Budget, Data, Effect, Input, Operation, Place, Plan, Program, Site, Terminator, Value,
    memory::{Address, Cell, Frame, Memory},
};
use crate::{ConstantScalarType, scalar};
use nocter_model::{ConstantValue, ValueCode, ValueCodeRoot, ValueInputOperation};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    InvalidPlan,
    UnavailablePlan,
    PlanWorkLimit,
    ArithmeticFailure,
    IndexOutOfBounds,
    InactiveVariant,
    StepLimit,
    CallDepthLimit,
    StorageLimit,
    ReachedUnreachable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub rule: Rule,
    pub site: Option<Site>,
    pub code: ValueCodeRoot,
    pub trace: Box<[Site]>,
}
type Result<T> = std::result::Result<T, Rule>;
/// Executes validated plans and call boundaries. A source may realize already selected bodies.
pub struct Executor<'a, 'budget> {
    program: std::borrow::Cow<'a, Program>,
    source: Option<&'a mut dyn super::PlanSource>,
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
        Self::from_parts(
            std::borrow::Cow::Borrowed(program),
            None,
            budget,
            suspended_depth,
        )
    }
    /// Execute with an append-only source of already selected code bodies.
    #[must_use]
    pub fn with_source(
        program: Program,
        source: &'a mut dyn super::PlanSource,
        budget: &'budget mut Budget,
        suspended_depth: usize,
    ) -> Self {
        Self::from_parts(
            std::borrow::Cow::Owned(program),
            Some(source),
            budget,
            suspended_depth,
        )
    }
    fn from_parts(
        program: std::borrow::Cow<'a, Program>,
        source: Option<&'a mut dyn super::PlanSource>,
        budget: &'budget mut Budget,
        suspended_depth: usize,
    ) -> Self {
        let memory = Memory::new(budget.storage.clone());
        Self {
            program,
            source,
            budget,
            suspended_depth,
            memory,
            site: None,
            failed_site: None,
            trace: Vec::new(),
        }
    }
    /// # Errors
    /// Reports a failed closed initializer with its exact operation and call path.
    pub fn evaluate_initializer(
        &mut self,
        code: impl Into<ValueCodeRoot>,
    ) -> std::result::Result<Value, Error> {
        let code = code.into();
        self.site = None;
        self.failed_site = None;
        self.trace.clear();
        let result = if self
            .program
            .contracts
            .get(&code.code().into())
            .is_some_and(|p| p.initializer)
        {
            self.invoke(code.code().into(), &[])
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
    fn invoke(&mut self, code: super::CodeInstance, inputs: &[Cell]) -> Result<Value> {
        if self
            .memory
            .frames
            .len()
            .checked_add(self.suspended_depth)
            .is_none_or(|depth| depth >= self.budget.limits().call_depth().get() as usize)
        {
            return Err(Rule::CallDepthLimit);
        }
        if !self.program.contracts.contains_key(&code) {
            return Err(Rule::InvalidPlan);
        }
        if !self.program.plans.contains_key(&code) {
            let extension = self
                .source
                .as_mut()
                .ok_or(Rule::UnavailablePlan)?
                .load(code)?;
            if !extension.plans.contains_key(&code) {
                return Err(Rule::InvalidPlan);
            }
            self.program.to_mut().extend(extension)?;
        }
        let plan = self
            .program
            .plans
            .get(&code)
            .cloned()
            .ok_or(Rule::InvalidPlan)?;
        if inputs.len() != plan.inputs.len() {
            return Err(Rule::InvalidPlan);
        }
        let mut frame = Frame::new(&self.memory.storage, plan.slots.clone())?;
        self.bind_inputs(code.source, &plan, &mut frame, inputs)?;
        for &(slot, id) in &plan.statics {
            let value = &self
                .program
                .statics
                .get(&id)
                .ok_or(Rule::InvalidPlan)?
                .value;
            self.memory.bind_static(&mut frame, slot, id, value)?;
        }
        let index = self.memory.frames.len();
        self.memory.frames.push(frame);
        let saved = self.site;
        let result = self.run(code.source, &plan, index);
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
    fn bind_inputs(
        &self,
        code: ValueCode,
        plan: &Plan,
        frame: &mut Frame,
        inputs: &[Cell],
    ) -> Result<()> {
        for (index, (&(slot, operation), input)) in plan.inputs.iter().zip(inputs).enumerate() {
            match (operation, input) {
                (ValueInputOperation::Take, Cell::Data(_)) => {
                    frame.own(&self.memory.storage, slot)?;
                }
                (ValueInputOperation::Read | ValueInputOperation::Edit, Cell::Alias(_)) => {}
                (ValueInputOperation::Read | ValueInputOperation::Edit, Cell::Data(_))
                    if index == 0 && matches!(code, ValueCode::Closure(_)) =>
                {
                    let backing = plan.environment_backing.ok_or(Rule::InvalidPlan)?;
                    frame.set(&self.memory.storage, backing, input.clone())?;
                    frame.set(
                        &self.memory.storage,
                        slot,
                        Cell::Alias(Address {
                            root: super::memory::Root::Local {
                                frame: self.memory.frames.len(),
                                slot: backing,
                            },
                            fields: super::memory::Path::default(),
                        }),
                    )?;
                    continue;
                }
                _ => return Err(Rule::InvalidPlan),
            }
            frame.set(&self.memory.storage, slot, input.clone())?;
        }
        Ok(())
    }
    fn run(&mut self, code: ValueCode, plan: &Plan, frame: usize) -> Result<Value> {
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
                    let Data::Bool(value) = value.data() else {
                        return Err(Rule::InvalidPlan);
                    };
                    point = if *value { *yes } else { *no };
                }
                Terminator::Return(place) => {
                    let address = self.memory.address(frame, place)?;
                    let (_, slot) = address.local()?;
                    let owners = &self.memory.frames[frame].owned;
                    if owners.len() != 1 || owners.get(&slot).is_none_or(|holes| !holes.is_empty())
                    {
                        return Err(Rule::InvalidPlan);
                    }
                    let value = self.memory.read(&address)?;
                    return Ok(value);
                }
                Terminator::Unreachable => return Err(Rule::ReachedUnreachable),
            }
        }
    }
    fn read(&self, frame: usize, place: &Place) -> Result<Value> {
        self.memory.read(&self.memory.address(frame, place)?)
    }
    fn write(&mut self, frame: usize, place: &Place, value: Value) -> Result<()> {
        self.memory
            .store(&self.memory.address(frame, place)?, value)
    }
    fn action(&mut self, frame: usize, action: &Action) -> Result<()> {
        let owners = &self.memory.frames[frame].owned;
        let mut selected: Option<&super::Alternative> = None;
        for alternative in &action.alternatives {
            // Test complete owner presence before resolving any hole. Another route can have
            // the same numbers but no initialized backing for this alternative's snapshot.
            if !alternative.owned.keys().eq(owners.keys()) {
                continue;
            }
            let mut matches = true;
            for (slot, holes) in &alternative.owned {
                let actual = owners.get(slot).ok_or(Rule::InvalidPlan)?;
                if holes.is_empty() {
                    if !actual.is_empty() {
                        matches = false;
                        break;
                    }
                    continue;
                }
                let _paths = self.memory.storage.items(holes.len())?;
                let mut resolved = holes
                    .iter()
                    .map(|missing| self.memory.resolve_missing(frame, missing))
                    .collect::<Result<Vec<_>>>()?;
                // Symbolic slot/recipe order never establishes numeric element order.
                resolved.sort_unstable();
                if !actual.matches(&resolved) {
                    matches = false;
                    break;
                }
            }
            if matches {
                if let Some(previous) = selected
                    && (previous.before != alternative.before
                        || previous.after != alternative.after)
                {
                    // Equal numbers do not establish which logical backing/lifecycle route
                    // occurred. Never let ordering choose between conflicting matching phases.
                    return Err(Rule::InvalidPlan);
                }
                selected = Some(alternative);
            }
        }
        let alternative = selected.ok_or(Rule::InvalidPlan)?;
        // Invoke records the exact protection point after committed materializations and before
        // source retirement/reclamation. Scoped addresses are bound after the before phase.
        let inputs = if let Operation::Call { inputs, .. } = &action.operation {
            inputs.as_ref()
        } else {
            &[]
        };
        let _taken_storage = if matches!(action.operation, Operation::Call { .. }) {
            Some(self.memory.storage.items(inputs.len())?)
        } else {
            None
        };
        let mut taken = vec![None; inputs.len()];
        for step in alternative.sequence(&action.operation) {
            match step {
                super::sequence::Step::Before(effect) => {
                    self.tick()?;
                    if let Effect::Invoke {
                        inputs: protected, ..
                    } = effect
                    {
                        for (index, place) in protected {
                            let index = *index;
                            if taken.get(index).is_none_or(Option::is_some) {
                                return Err(Rule::InvalidPlan);
                            }
                            taken[index] = Some(self.read(frame, place)?);
                        }
                    }
                    self.effect(frame, effect)?;
                }
                super::sequence::Step::Operation(op) => self.operation(frame, op, &taken)?,
                super::sequence::Step::After(effect) => {
                    self.tick()?;
                    self.effect(frame, effect)?;
                }
            }
        }
        Ok(())
    }
    fn scalar_type(&self, frame: usize, place: &Place) -> Result<ConstantScalarType> {
        let slots = &self
            .memory
            .frames
            .get(frame)
            .ok_or(Rule::InvalidPlan)?
            .slots;
        let data = super::validation::place_data(&self.program, slots, place)?;
        self.program
            .types
            .get(data)
            .and_then(super::Type::scalar)
            .ok_or(Rule::InvalidPlan)
    }
    #[allow(clippy::too_many_lines)]
    fn operation(&mut self, frame: usize, op: &Operation, taken: &[Option<Value>]) -> Result<()> {
        let (destination, value) = match op {
            Operation::None => return Ok(()),
            Operation::RequireVariant {
                source,
                primary,
                view,
            } => {
                self.require_variant(frame, source, *primary)?;
                if let Some(destination) = view {
                    let address = self.memory.address(frame, source)?;
                    self.bind_alias(frame, *destination, address)?;
                }
                return Ok(());
            }
            Operation::PrimaryCondition {
                source,
                destination,
            } => {
                let value = self.read(frame, source)?;
                let Data::Variant { primary, .. } = value.data() else {
                    return Err(Rule::InvalidPlan);
                };
                (
                    *destination,
                    self.memory.storage.primitive(Data::Bool(*primary))?,
                )
            }
            Operation::BoundsTrap => return Err(Rule::IndexOutOfBounds),
            Operation::Literal { destination, value } => {
                (*destination, self.memory.storage.import(value)?)
            }
            Operation::Alias {
                destination,
                source,
            } => {
                let address = self.memory.address(frame, source)?;
                return self.bind_alias(frame, *destination, address);
            }
            Operation::Index {
                destination,
                base,
                index,
            }
            | Operation::IndexCopy {
                destination,
                base,
                index,
            } => {
                let address = self.index_address(frame, base, index)?;
                return if matches!(op, Operation::Index { .. }) {
                    self.bind_alias(frame, *destination, address)
                } else {
                    let value = self.memory.read(&address)?;
                    self.write(frame, &Place::root(*destination), value)
                };
            }
            Operation::Repeat {
                destination,
                source,
            } => (*destination, self.repeated(frame, *destination, source)?),
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
                let ty = self.scalar_type(frame, operand)?;
                let value = scalar_value(value.data())?;
                (
                    *destination,
                    from_scalar(
                        &scalar::unary(*operation, ty, &value, None).map_err(scalar_rule)?,
                        &self.memory.storage,
                    )?,
                )
            }
            Operation::Binary {
                destination,
                operation,
                left,
                right,
            } => {
                let ty = self.scalar_type(frame, left)?;
                let left = scalar_value(self.read(frame, left)?.data())?;
                let right = scalar_value(self.read(frame, right)?.data())?;
                let value = from_scalar(
                    &scalar::value_binary(*operation, ty, &left, &right).map_err(scalar_rule)?,
                    &self.memory.storage,
                )?;
                return self.write(frame, destination, value);
            }
            Operation::Compare {
                destination,
                operation,
                left,
                right,
            } => {
                let left = scalar_value(self.read(frame, left)?.data())?;
                let right = scalar_value(self.read(frame, right)?.data())?;
                (
                    *destination,
                    from_scalar(
                        &scalar::compare(*operation, &left, &right, None).map_err(scalar_rule)?,
                        &self.memory.storage,
                    )?,
                )
            }
            Operation::Call {
                destination,
                target,
                inputs,
            } => {
                let _input_storage = self.memory.storage.items(inputs.len())?;
                let inputs = self.inputs(frame, inputs, taken)?;
                (*destination, self.invoke(*target, &inputs)?)
            }
        };
        self.write(frame, &Place::root(destination), value)
    }
    fn require_variant(&self, frame: usize, source: &Place, expected: bool) -> Result<()> {
        if matches!(
            self.read(frame, source)?.data(),
            Data::Variant { primary, .. } if *primary == expected
        ) {
            Ok(())
        } else {
            Err(Rule::InactiveVariant)
        }
    }
    fn repeated(&self, frame: usize, destination: usize, source: &Place) -> Result<Value> {
        let slots = &self
            .memory
            .frames
            .get(frame)
            .ok_or(Rule::InvalidPlan)?
            .slots;
        let data = slots.get(destination).ok_or(Rule::InvalidPlan)?.data;
        let Some(super::Type::Array { length, .. }) = self.program.types.get(data) else {
            return Err(Rule::InvalidPlan);
        };
        let length = std::num::NonZeroU64::new(*length).ok_or(Rule::InvalidPlan)?;
        self.memory
            .storage
            .repeated(self.read(frame, source)?, length, std::iter::empty())
    }
    fn bind_alias(&mut self, frame: usize, destination: usize, address: Address) -> Result<()> {
        self.memory.frames[frame].set(&self.memory.storage, destination, Cell::Alias(address))
    }
    fn index_address(&self, frame: usize, base: &Place, index: &Place) -> Result<Address> {
        // Resolve the base before reading the index. Do not read the complete array:
        // the checked plan can select initialized components of an incomplete owner.
        let mut address = self.memory.address(frame, base)?;
        let data =
            super::validation::place_data(&self.program, &self.memory.frames[frame].slots, base)?;
        let Some(super::Type::Array { length, .. }) = self.program.types.get(data) else {
            return Err(Rule::InvalidPlan);
        };
        let index = self.read(frame, index)?;
        let Data::Integer(index) = index.data() else {
            return Err(Rule::InvalidPlan);
        };
        let index = u64::try_from(*index).map_err(|_| Rule::InvalidPlan)?;
        if index >= *length {
            return Err(Rule::IndexOutOfBounds);
        }
        // Host representation capacity is a storage limit, never a language bound.
        let index = usize::try_from(index).map_err(|_| Rule::StorageLimit)?;
        address.fields = address.fields.extended(&self.memory.storage, &[index])?;
        Ok(address)
    }
    fn inputs(&self, frame: usize, inputs: &[Input], taken: &[Option<Value>]) -> Result<Vec<Cell>> {
        inputs
            .iter()
            .enumerate()
            .map(|(index, input)| match input {
                Input::Take(_) => Ok(Cell::Data(
                    taken
                        .get(index)
                        .and_then(Option::as_ref)
                        .ok_or(Rule::InvalidPlan)?
                        .clone(),
                )),
                Input::Read(place) | Input::Edit(place) => {
                    let address = self.memory.address(frame, place)?;
                    // Closed types need no recursive shape walk; initialization still matters
                    // even when the callee never reads this scoped input.
                    self.memory.read(&address)?;
                    Ok(Cell::Alias(address))
                }
            })
            .collect()
    }
    fn effect(&mut self, frame: usize, effect: &Effect) -> Result<()> {
        match effect {
            Effect::MoveArrayElement {
                array,
                selected,
                destination,
            } => self.move_array_element(frame, array, selected, *destination),
            Effect::ConsolidateArrayPrefix { source, endpoint } => {
                let (address, range) =
                    self.array_span(frame, source, &nocter_model::ArrayRange::Through(*endpoint))?;
                self.memory.consolidate_prefix(&address, range.end)
            }
            Effect::DisposeArrayElements { source, range } => {
                let (address, interval) = self.array_span(frame, source, range)?;
                if interval.is_empty() {
                    return Ok(());
                }
                self.memory
                    .retire_array(&address, interval.start, interval.end)
            }
            Effect::Construct(slot) => Ok(self
                .memory
                .construct(&self.memory.address(frame, &Place::root(*slot))?)?),
            Effect::Dispose(place) | Effect::Invoke { source: place, .. } => Ok(self
                .memory
                .retire(&self.memory.address(frame, place)?, false)?),
            Effect::DisposeArrayRemainder(place) | Effect::Release(place) => Ok(self
                .memory
                .retire(&self.memory.address(frame, place)?, true)?),
            Effect::Move {
                source,
                destination,
            }
            | Effect::AdaptFunction {
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
                if matches!(effect, Effect::Move { .. } | Effect::AdaptFunction { .. }) {
                    self.memory.construct(&destination)?;
                }
                Ok(())
            }
            Effect::Assemble {
                destination,
                fields,
            } => {
                let _assembly_storage = self.memory.storage.items(fields.len())?;
                let data = self.memory.frames[frame]
                    .slots
                    .get(*destination)
                    .ok_or(Rule::InvalidPlan)?
                    .data;
                let value = if matches!(
                    self.program.types.get(data),
                    Some(super::Type::Variant { .. })
                ) {
                    match fields.as_ref() {
                        [] => self.memory.storage.variant(false, None)?,
                        [(field, source)] => {
                            let payload = self.read(frame, source)?;
                            self.memory
                                .retire(&self.memory.address(frame, source)?, false)?;
                            self.memory.storage.variant(*field == 0, Some(payload))?
                        }
                        _ => return Err(Rule::InvalidPlan),
                    }
                } else {
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
                    self.memory
                        .storage
                        .aggregate(values.into_vec().into_iter())?
                };
                self.write(frame, &Place::root(*destination), value)?;
                Ok(self
                    .memory
                    .construct(&self.memory.address(frame, &Place::root(*destination))?)?)
            }
        }
    }
    fn move_array_element(
        &mut self,
        frame: usize,
        array: &Place,
        selected: &Place,
        destination: usize,
    ) -> Result<()> {
        let array_address = self.memory.address(frame, array)?;
        let selected_address = self.memory.address(frame, selected)?;
        self.memory.require_array_element(
            &array_address,
            &selected_address,
            self.array_length(frame, array)?,
        )?;
        let value = self.memory.read(&selected_address)?;
        self.memory.retire(&selected_address, false)?;
        let destination = self.memory.address(frame, &Place::root(destination))?;
        self.memory.store(&destination, value)?;
        self.memory.construct(&destination)
    }
    fn array_length(&self, frame: usize, source: &Place) -> Result<u64> {
        let data =
            super::validation::place_data(&self.program, &self.memory.frames[frame].slots, source)?;
        let Some(super::Type::Array { length, .. }) = self.program.types.get(data) else {
            return Err(Rule::InvalidPlan);
        };
        Ok(*length)
    }
    fn array_span(
        &self,
        frame: usize,
        source: &Place,
        range: &nocter_model::ArrayRange<usize>,
    ) -> Result<(Address, std::ops::Range<u64>)> {
        let address = self.memory.address(frame, source)?;
        let length = self.array_length(frame, source)?;
        let captured = match range {
            nocter_model::ArrayRange::Constant { .. } => None,
            nocter_model::ArrayRange::Before(slot)
            | nocter_model::ArrayRange::Through(slot)
            | nocter_model::ArrayRange::After(slot) => {
                Some(self.memory.snapshot_index(frame, *slot, length)?)
            }
        };
        Ok((
            address,
            range
                .resolve(length, |_| captured)
                .ok_or(Rule::InvalidPlan)?,
        ))
    }
}
fn scalar_value(value: &Data) -> Result<ConstantValue> {
    match value {
        Data::Bool(value) => Ok(ConstantValue::Bool(*value)),
        Data::Integer(value) => Ok(ConstantValue::Integer(*value)),
        _ => Err(Rule::InvalidPlan),
    }
}
fn from_scalar(value: &ConstantValue, storage: &super::budget::Storage) -> Result<Value> {
    match value {
        ConstantValue::Bool(value) => storage.primitive(Data::Bool(*value)),
        ConstantValue::Integer(value) => storage.primitive(Data::Integer(*value)),
        _ => Err(Rule::InvalidPlan),
    }
}
const fn scalar_rule(error: scalar::ScalarEvaluationFailure) -> Rule {
    match error {
        scalar::ScalarEvaluationFailure::Arithmetic => Rule::ArithmeticFailure,
        scalar::ScalarEvaluationFailure::InvalidType => Rule::InvalidPlan,
    }
}
