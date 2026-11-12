//! Integrity of the closed product, never another source-acceptance or ownership checker.
use super::{
    Body, Effect, Failure, Operation, Place, Program, Projection, SlotKey, Storage, Terminator,
};
use crate::{
    MirArithmeticCheck as Check, MirBinaryOperation as Binary, MirUnaryOperation as Unary,
};
use nocter_model::{AccessInstanceId, TypeId, ValueInputOperation};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeRepresentation};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn program(program: &Program) -> Result<(), Failure> {
    if !program.bodies.contains_key(&program.root) {
        return Err(Failure::InvalidBody(program.root));
    }
    let root = program
        .bodies
        .get(&program.root)
        .ok_or(Failure::InvalidBody(program.root))?;
    let context = Context {
        program,
        body: root,
        id: program.root,
    };
    let used: BTreeSet<_> = program
        .bodies
        .values()
        .flat_map(|body| body.statics.iter().map(|&(_, id)| id))
        .collect();
    context.require(used.iter().eq(program.statics.keys()))?;
    for value in program.statics.values() {
        context.frozen(value.data, &value.value)?;
    }
    for (&id, body) in &program.bodies {
        validate_body(program, id, body)?;
    }
    let mut pending = BTreeSet::from([program.root]);
    let mut reached = BTreeSet::new();
    while let Some(id) = pending.pop_first() {
        if !reached.insert(id) {
            continue;
        }
        let body = program.bodies.get(&id).ok_or(Failure::InvalidBody(id))?;
        for operation in body.actions.values().flat_map(|action| &action.operations) {
            if let Operation::Call(call) = operation {
                pending.insert(call.target);
            }
        }
    }
    if reached.len() != program.bodies.len() {
        return Err(Failure::InvalidBody(program.root));
    }
    Ok(())
}

struct Context<'a> {
    program: &'a Program,
    body: &'a Body,
    id: AccessInstanceId,
}
impl Context<'_> {
    fn invalid(&self) -> Failure {
        Failure::InvalidBody(self.id)
    }
    fn require(&self, condition: bool) -> Result<(), Failure> {
        if condition {
            Ok(())
        } else {
            Err(self.invalid())
        }
    }
    fn place(&self, place: &Place) -> Result<(), Failure> {
        let slot = self
            .body
            .slots
            .get(place.slot)
            .ok_or_else(|| self.invalid())?;
        self.require(slot.representations.contains(&place.root))?;
        let mut data = place.root;
        for projection in &place.projections {
            data = match projection {
                Projection::Field(id) => {
                    if let nocter_model::ValueField::Variant(primary) = id {
                        data = self
                            .program
                            .runtime
                            .types()
                            .outcome_payload(data, *primary)
                            .ok_or_else(|| self.invalid())?;
                        continue;
                    }
                    if let (nocter_model::ValueField::Array(index), Some((element, length))) = (
                        id,
                        self.program
                            .runtime
                            .types()
                            .get(data)
                            .and_then(RuntimeType::array),
                    ) {
                        self.require(*index < length)?;
                        data = element;
                        continue;
                    }
                    let Some(RuntimeTypeRepresentation::ValueAggregate { fields }) =
                        self.program.runtime.type_representations().get(data)
                    else {
                        return Err(self.invalid());
                    };
                    fields
                        .iter()
                        .find(|field| field.field() == *id)
                        .ok_or_else(|| self.invalid())?
                        .ty()
                }
                Projection::Index(id) => {
                    let (element, length) = self
                        .program
                        .runtime
                        .types()
                        .get(data)
                        .and_then(RuntimeType::array)
                        .ok_or_else(|| self.invalid())?;
                    self.snapshot(*id, length)?;
                    element
                }
                Projection::Environment => {
                    let Some(RuntimeType::StaticCallable {
                        environment: Some(data),
                    }) = self.program.runtime.types().get(data)
                    else {
                        return Err(self.invalid());
                    };
                    *data
                }
            };
        }
        self.require(data == place.data && self.program.runtime.types().get(data).is_some())
    }
    fn snapshot(&self, id: nocter_model::MirLocalId, length: u64) -> Result<(), Failure> {
        let slot = self.body.slots.get(id).ok_or_else(|| self.invalid())?;
        let SlotKey::IndexSnapshot(index) = slot.key else {
            return Err(self.invalid());
        };
        self.bounded_slot(id, index.bound(), length)
    }
    fn range_index(&self, id: nocter_model::MirLocalId, length: u64) -> Result<(), Failure> {
        let slot = self.body.slots.get(id).ok_or_else(|| self.invalid())?;
        let bound = match slot.key {
            SlotKey::IndexSnapshot(index) => index.bound(),
            SlotKey::ArrayFrontier(frontier) => frontier.bound(),
            _ => return Err(self.invalid()),
        };
        self.bounded_slot(id, bound, length)
    }
    fn bounded_slot(
        &self,
        id: nocter_model::MirLocalId,
        bound: u64,
        length: u64,
    ) -> Result<(), Failure> {
        let slot = self.body.slots.get(id).ok_or_else(|| self.invalid())?;
        let data = self
            .program
            .runtime
            .types()
            .primitive(RuntimePrimitive::Usize)
            .ok_or_else(|| self.invalid())?;
        self.require(
            length > 0
                && bound == length
                && slot.storage == Storage::Payload
                && slot.representations.as_ref() == [data],
        )
    }
    fn array_remainder(
        &self,
        source: &Place,
        missing: &[super::RemainderExclusion],
    ) -> Result<(), Failure> {
        self.place(source)?;
        self.require(
            self.program
                .runtime
                .types()
                .get(source.data)
                .and_then(RuntimeType::array)
                .is_some_and(|(_, length)| length > 0)
                && missing.len() > 1
                && self
                    .body
                    .slots
                    .get(source.slot)
                    .is_some_and(|slot| slot.storage == Storage::Payload),
        )?;
        let mut seen = BTreeSet::new();
        for exclusion in missing {
            let hole = match exclusion {
                super::RemainderExclusion::Value(hole) => hole,
                super::RemainderExclusion::ArrayPrefix { array, endpoint } => {
                    let (_, length) = self
                        .program
                        .runtime
                        .types()
                        .get(array.data)
                        .and_then(RuntimeType::array)
                        .ok_or_else(|| self.invalid())?;
                    self.require(matches!(
                        self.body.slots.get(*endpoint).map(|slot| slot.key),
                        Some(SlotKey::ArrayFrontier(_))
                    ))?;
                    self.range_index(*endpoint, length)?;
                    array
                }
            };
            self.place(hole)?;
            self.require(
                seen.insert(exclusion)
                    && hole.slot == source.slot
                    && hole.root == source.root
                    && hole.projections.len() > source.projections.len()
                    && hole.projections.starts_with(&source.projections),
            )?;
        }
        Ok(())
    }
    fn effect(&self, effect: &Effect) -> Result<(), Failure> {
        match effect {
            Effect::VariantTag { destination, .. } => {
                self.place(destination)?;
                self.require(matches!(
                    self.program.runtime.types().get(destination.data),
                    Some(RuntimeType::Optional(_) | RuntimeType::Fallible(_))
                ))
            }

            Effect::DisposeArrayRemainder { source, missing } => {
                self.array_remainder(source, missing)
            }
            Effect::ConsolidateArrayPrefix { source, endpoint } => {
                self.place(source)?;
                let (_, length) = self
                    .program
                    .runtime
                    .types()
                    .get(source.data)
                    .and_then(RuntimeType::array)
                    .ok_or_else(|| self.invalid())?;
                if !matches!(
                    self.body.slots.get(*endpoint).map(|slot| slot.key),
                    Some(SlotKey::ArrayFrontier(_))
                ) {
                    return Err(self.invalid());
                }
                self.range_index(*endpoint, length)
            }
            Effect::DisposeArrayElements { source, range } => {
                self.place(source)?;
                let (_, length) = self
                    .program
                    .runtime
                    .types()
                    .get(source.data)
                    .and_then(RuntimeType::array)
                    .ok_or_else(|| self.invalid())?;
                match range {
                    nocter_model::ArrayRange::Constant { .. } => {
                        self.require(range.resolve(length, |_| None).is_some())
                    }
                    nocter_model::ArrayRange::Before(id)
                    | nocter_model::ArrayRange::Through(id)
                    | nocter_model::ArrayRange::After(id) => self.range_index(*id, length),
                }
            }
            Effect::Initialize(place)
            | Effect::End(place)
            | Effect::Dispose { source: place, .. } => self.place(place),
            Effect::Transfer {
                source,
                destination,
            } => {
                self.place(source)?;
                self.place(destination)?;
                self.require(source.data == destination.data)
            }
            Effect::Reinterpret {
                source,
                destination,
            } => {
                self.place(source)?;
                self.place(destination)?;
                let source = self.program.runtime.types().get(source.data);
                let destination = self.program.runtime.types().get(destination.data);
                self.require(matches!((source, destination), (Some(RuntimeType::StaticCallable { environment: left }), Some(RuntimeType::StaticCallable { environment: right })) if left == right))
            }
            Effect::BindAddress {
                destination,
                source,
            } => {
                self.place(source)?;
                let slot = self
                    .body
                    .slots
                    .get(*destination)
                    .ok_or_else(|| self.invalid())?;
                self.require(
                    slot.storage == Storage::Address && slot.representations.contains(&source.data),
                )
            }
        }
    }
    fn primitive(&self, data: TypeId, role: RuntimePrimitive) -> bool {
        self.program.runtime.types().get(data) == Some(&RuntimeType::Primitive(role))
    }
    fn integer_type(&self, data: TypeId) -> Option<nocter_model::BuiltinNumericType> {
        match self.program.runtime.types().get(data)? {
            RuntimeType::Primitive(primitive) => primitive.integer(),
            _ => None,
        }
    }
    fn integer(&self, data: TypeId) -> bool {
        self.integer_type(data).is_some()
    }
    fn integer_literal(&self, data: TypeId, value: i128) -> bool {
        self.integer_type(data)
            .and_then(nocter_model::BuiltinNumericType::integer_bounds)
            .is_some_and(|(minimum, maximum)| value >= minimum && value <= maximum)
    }
    #[allow(clippy::too_many_lines)]
    fn operation(&self, operation: &Operation) -> Result<(), Failure> {
        let boolean = RuntimePrimitive::Bool;
        match operation {
            Operation::BoundsTrap => Ok(()),
            Operation::PrimaryCondition {
                source,
                destination,
            } => {
                self.place(source)?;
                self.place(destination)?;
                self.require(
                    matches!(
                        self.program.runtime.types().get(source.data),
                        Some(RuntimeType::Optional(_) | RuntimeType::Fallible(_))
                    ) && self.primitive(destination.data, RuntimePrimitive::Bool),
                )
            }
            Operation::RequirePrimary { source } => {
                self.place(source)?;
                self.require(matches!(
                    self.program.runtime.types().get(source.data),
                    Some(RuntimeType::Optional(_) | RuntimeType::Fallible(_))
                ))
            }
            Operation::Frozen { destination, value } => {
                self.place(destination)?;
                self.frozen(destination.data, value)
            }
            Operation::Constant { destination, value } => {
                self.place(destination)?;
                self.require(match value {
                    super::Constant::Integer(value) => {
                        self.integer_literal(destination.data, *value)
                    }
                    super::Constant::Boolean(_) => self.primitive(destination.data, boolean),
                    super::Constant::Zero => matches!(
                        self.program.runtime.types().get(destination.data),
                        Some(RuntimeType::Unit | RuntimeType::StaticCallable { environment: None })
                    ),
                })
            }
            Operation::Copy {
                source,
                destination,
            } => {
                self.place(source)?;
                self.place(destination)?;
                self.require(source.data == destination.data)
            }
            Operation::Storage(effect) => self.effect(effect),
            Operation::Repeat {
                source,
                destination,
            } => {
                self.place(source)?;
                self.place(destination)?;
                let Some(RuntimeType::FixedArray { element, length }) =
                    self.program.runtime.types().get(destination.data)
                else {
                    return Err(self.invalid());
                };
                let slot = self
                    .body
                    .slots
                    .get(destination.slot)
                    .ok_or_else(|| self.invalid())?;
                self.require(
                    source.data == *element
                        && *length > 1
                        && destination.projections.is_empty()
                        && slot.storage == Storage::Payload,
                )
            }
            Operation::Index {
                base,
                index,
                destination,
                copy,
                bounds,
            } => {
                self.place(base)?;
                self.place(index)?;
                self.place(destination)?;
                let Some((element, _)) = self
                    .program
                    .runtime
                    .types()
                    .get(base.data)
                    .and_then(RuntimeType::array)
                else {
                    return Err(self.invalid());
                };
                let slot = self
                    .body
                    .slots
                    .get(destination.slot)
                    .ok_or_else(|| self.invalid())?;
                self.require(
                    self.primitive(index.data, RuntimePrimitive::Usize)
                        && *bounds != nocter_model::IndexBoundsCheck::ProvenTrap
                        && destination.data == element
                        && destination.projections.is_empty()
                        && slot.storage
                            == if *copy {
                                Storage::Payload
                            } else {
                                Storage::Address
                            },
                )
            }
            Operation::Unary {
                operation,
                operand,
                destination,
                check,
            } => {
                self.place(operand)?;
                self.place(destination)?;
                self.require(match operation {
                    Unary::LogicalNot => {
                        self.primitive(operand.data, boolean)
                            && self.primitive(destination.data, boolean)
                            && *check == Check::NotRequired
                    }
                    Unary::Negate => {
                        matches!(
                            self.integer_type(operand.data),
                            Some(nocter_model::BuiltinNumericType::Integer { signed: true, .. })
                        ) && operand.data == destination.data
                            && *check == Check::Required
                    }
                })
            }
            Operation::Binary {
                operation,
                left,
                right,
                destination,
                invert,
                check,
            } => {
                self.place(left)?;
                self.place(right)?;
                self.place(destination)?;
                self.require(left.data == right.data)?;
                self.require(match operation {
                    Binary::Add
                    | Binary::Subtract
                    | Binary::Multiply
                    | Binary::Divide
                    | Binary::Remainder => {
                        self.integer(left.data)
                            && left.data == destination.data
                            && !invert
                            && *check
                                == if *operation == Binary::Remainder {
                                    Check::NonZeroDivisor
                                } else {
                                    Check::Required
                                }
                    }
                    Binary::Equal => {
                        (self.primitive(left.data, boolean) || self.integer(left.data))
                            && self.primitive(destination.data, boolean)
                            && *check == Check::NotRequired
                    }
                    Binary::Less => {
                        self.integer(left.data)
                            && self.primitive(destination.data, boolean)
                            && *check == Check::NotRequired
                    }
                    Binary::ShiftLeft | Binary::ShiftRightSigned | Binary::ShiftRightUnsigned => {
                        false
                    }
                })
            }
            Operation::Call(call) => {
                self.place(&call.answer)?;
                let callee = self
                    .program
                    .bodies
                    .get(&call.target)
                    .ok_or_else(|| self.invalid())?;
                let inputs = callee.signature.ordered().collect::<Vec<_>>();
                self.require(
                    call.inputs.len() == inputs.len()
                        && call.answer.data == callee.signature.result(),
                )?;
                self.require(self.body.slots.get(call.answer.slot).is_some_and(|slot| {
                    slot.storage == Storage::Payload && matches!(slot.key, SlotKey::Answer(_))
                }))?;
                for (actual, expected) in call.inputs.iter().zip(inputs) {
                    self.place(&actual.source)?;
                    self.require(
                        actual.source.data == expected.data
                            && actual.operation == expected.operation,
                    )?;
                    if actual.operation == ValueInputOperation::Take {
                        self.require(
                            self.body
                                .slots
                                .get(actual.source.slot)
                                .is_some_and(|slot| slot.storage == Storage::Payload),
                        )?;
                    }
                }
                Ok(())
            }
        }
    }
}

fn validate_body(program: &Program, id: AccessInstanceId, body: &Body) -> Result<(), Failure> {
    let context = Context { program, body, id };
    let mut bound = BTreeSet::new();
    for &(slot, id) in &body.statics {
        let data = program.statics.get(&id).ok_or_else(|| context.invalid())?;
        let slot_data = body.slots.get(slot).ok_or_else(|| context.invalid())?;
        context.require(
            bound.insert(slot)
                && !body.inputs.contains(&slot)
                && slot_data.storage == Storage::Address
                && slot_data.representations.as_ref() == [data.data],
        )?;
    }
    let mut keys = BTreeSet::new();
    for (_, slot) in body.slots.iter() {
        context.require(keys.insert(slot.key) && !slot.representations.is_empty())?;
        for &data in &slot.representations {
            context.require(program.runtime.types().get(data).is_some())?;
        }
    }
    let inputs = body.signature.ordered().collect::<Vec<_>>();
    context.require(inputs.len() == body.inputs.len())?;
    context
        .require(body.inputs.iter().copied().collect::<BTreeSet<_>>().len() == body.inputs.len())?;
    for (input, &local) in inputs.into_iter().zip(&body.inputs) {
        let slot = body.slots.get(local).ok_or_else(|| context.invalid())?;
        let storage = if input.operation == ValueInputOperation::Take {
            Storage::Payload
        } else {
            Storage::Address
        };
        context.require(
            slot.key == SlotKey::Local(input.local)
                && slot.storage == storage
                && slot.representations.contains(&input.data),
        )?;
    }
    let mut lengths = BTreeMap::new();
    for (&at, action) in &body.actions {
        context.require(body.terminators.contains_key(&at.point))?;
        let length = lengths.entry(at.point).or_insert(0);
        context.require(at.action == *length)?;
        *length += 1;
        context.require(!action.alternatives.is_empty())?;
        for alternative in &action.alternatives {
            for effect in alternative.before.iter().chain(&alternative.after) {
                context.effect(effect)?;
            }
            for bit in alternative
                .tests
                .iter()
                .chain(&alternative.before_writes)
                .chain(&alternative.after_writes)
            {
                context.require(bit.word < body.flags.len() && bit.mask.is_power_of_two())?;
            }
        }
        for effect in &action.call_bindings {
            context.effect(effect)?;
        }
        for operation in &action.operations {
            context.operation(operation)?;
        }
    }
    for (&at, action) in &body.actions {
        if action
            .operations
            .iter()
            .any(|operation| matches!(operation, Operation::BoundsTrap))
        {
            context.require(
                matches!(action.operations.as_ref(), [Operation::BoundsTrap])
                    && action.call_bindings.is_empty()
                    && action.alternatives.iter().all(|alternative| {
                        alternative.before.is_empty()
                            && alternative.after.is_empty()
                            && alternative.before_writes.is_empty()
                            && alternative.after_writes.is_empty()
                    })
                    && lengths.get(&at.point) == Some(&(at.action + 1))
                    && matches!(
                        body.terminators.get(&at.point),
                        Some(Terminator::Unreachable)
                    ),
            )?;
        }
    }
    validate_control(&context)
}

fn validate_control(context: &Context<'_>) -> Result<(), Failure> {
    let body = context.body;
    context.require(body.terminators.contains_key(&body.entry))?;
    let mut pending = BTreeSet::from([body.entry]);
    let mut reached = BTreeSet::new();
    while let Some(point) = pending.pop_first() {
        if !reached.insert(point) {
            continue;
        }
        match body
            .terminators
            .get(&point)
            .ok_or_else(|| context.invalid())?
        {
            Terminator::Jump(next) => {
                pending.insert(*next);
            }
            Terminator::Branch { yes, no, .. } => {
                pending.extend([*yes, *no]);
            }
            Terminator::Return(_) | Terminator::Unreachable => {}
        }
    }
    context.require(reached.len() == body.terminators.len())?;
    for terminator in body.terminators.values() {
        match terminator {
            Terminator::Jump(next) => context.require(body.terminators.contains_key(next))?,
            Terminator::Branch { condition, yes, no } => {
                context.place(condition)?;
                context.require(
                    context.primitive(condition.data, RuntimePrimitive::Bool)
                        && body.terminators.contains_key(yes)
                        && body.terminators.contains_key(no),
                )?;
            }
            Terminator::Return(place) => {
                context.place(place)?;
                let slot = body
                    .slots
                    .get(place.slot)
                    .ok_or_else(|| context.invalid())?;
                context.require(
                    place.data == body.signature.result()
                        && slot.key == SlotKey::Return
                        && place.projections.is_empty()
                        && slot.storage == Storage::Payload,
                )?;
            }
            Terminator::Unreachable => {}
        }
    }
    Ok(())
}

impl Context<'_> {
    fn frozen(
        &self,
        data: nocter_model::TypeId,
        value: &nocter_model::FrozenData,
    ) -> Result<(), Failure> {
        use nocter_model::{ConstantValue, FrozenNode};
        let mut pending = vec![(data, value.root())];
        let mut seen = std::collections::BTreeSet::new();
        while let Some((data, node)) = pending.pop() {
            if !seen.insert((data, node)) {
                continue;
            }
            match (&value.nodes()[node], self.program.runtime.types().get(data)) {
                (FrozenNode::Unit, Some(RuntimeType::Unit))
                | (
                    FrozenNode::Scalar(ConstantValue::Bool(_)),
                    Some(RuntimeType::Primitive(RuntimePrimitive::Bool)),
                )
                | (
                    FrozenNode::Variant {
                        primary: false,
                        payload: None,
                    },
                    Some(RuntimeType::Optional(_)),
                ) => {}
                (
                    FrozenNode::Scalar(ConstantValue::Integer(value)),
                    Some(RuntimeType::Primitive(_)),
                ) if self.integer_literal(data, *value) => {}
                (
                    FrozenNode::Aggregate(values),
                    Some(RuntimeType::FixedArray { element, length }),
                ) => {
                    self.require(u64::try_from(values.len()) == Ok(*length))?;
                    pending.extend(values.iter().map(|&node| (*element, node)));
                }
                (
                    FrozenNode::Repeated {
                        element: default,
                        length: count,
                        replacements,
                    },
                    Some(RuntimeType::FixedArray { element, length }),
                ) => {
                    self.require(count.get() == *length)?;
                    pending.push((*element, *default));
                    pending.extend(replacements.iter().map(|&(_, node)| (*element, node)));
                }

                (
                    FrozenNode::Variant {
                        primary: true,
                        payload: Some(node),
                    },
                    Some(RuntimeType::Optional(payload) | RuntimeType::Fallible(payload)),
                ) => pending.push((*payload, *node)),
                (FrozenNode::Aggregate(values), Some(RuntimeType::EmptyArray { .. })) => {
                    self.require(values.is_empty())?;
                }
                (FrozenNode::Aggregate(values), Some(RuntimeType::Aggregate)) => {
                    let Some(nocter_runtime_contract::RuntimeTypeRepresentation::ValueAggregate {
                        fields,
                    }) = self.program.runtime.type_representations().get(data)
                    else {
                        return Err(self.invalid());
                    };
                    self.require(fields.len() == values.len())?;
                    pending.extend(
                        fields
                            .iter()
                            .zip(values)
                            .map(|(field, &node)| (field.ty(), node)),
                    );
                }
                _ => return Err(self.invalid()),
            }
        }
        Ok(())
    }
}
