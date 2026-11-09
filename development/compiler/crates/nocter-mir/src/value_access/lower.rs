use super::{
    Action, Alternative, BitValue, Body, Disposal, Effect, Failure, Location, Place, Projection,
    Slot, SlotKey, Storage,
};
use nocter_checking::value_access::{
    ArgumentAction, ExpressionKind,
    accepted::DataEvidence,
    flow::{
        self, Operation,
        resources::{Disposal as CheckedDisposal, Holder, Payload, Step},
    },
};
use nocter_model::{AccessExpressionId, AccessInstanceId, MirLocalId, TypeId};
use nocter_model::{ArenaBuilder, ValueAccessMode, ValueInputOperation};
use nocter_target_program::value_access::{Instance, InstanceProgram, lifecycle::EnvironmentPart};
use nocter_target_program::value_access::{lifecycle, storage};
use std::collections::BTreeMap;

pub(super) struct Builder<'a> {
    pub(super) id: AccessInstanceId,
    pub(super) instances: &'a InstanceProgram,
    pub(super) instance: &'a Instance,
    pub(super) checked: &'a flow::Query,
    slots: ArenaBuilder<MirLocalId, Slot>,
    keys: BTreeMap<SlotKey, MirLocalId>,
}
impl Builder<'_> {
    pub(super) fn invalid(&self) -> Failure {
        Failure::InvalidBody(self.id)
    }
    pub(super) fn data(&self, source: TypeId) -> Result<TypeId, Failure> {
        self.instance
            .data()
            .get(&source)
            .and_then(DataEvidence::payload)
            .map(Payload::data)
            .ok_or_else(|| self.invalid())
    }
    pub(super) fn expression(&self, id: AccessExpressionId) -> Result<TypeId, Failure> {
        let access = self
            .checked
            .typed()
            .body()
            .expressions()
            .get(id)
            .and_then(|e| e.ty.access())
            .ok_or_else(|| self.invalid())?;
        self.data(access.data())
    }
    pub(super) fn arguments(
        &self,
        call: AccessExpressionId,
    ) -> Result<&[nocter_checking::value_access::Argument], Failure> {
        let Some(ExpressionKind::Call { arguments, .. }) = self
            .checked
            .typed()
            .body()
            .expressions()
            .get(call)
            .map(|e| &e.kind)
        else {
            return Err(self.invalid());
        };
        Ok(arguments)
    }
    pub(super) fn slot(
        &mut self,
        key: SlotKey,
        data: TypeId,
        storage: Storage,
    ) -> Result<MirLocalId, Failure> {
        if let Some(&id) = self.keys.get(&key) {
            let invalid = self.invalid();
            let slot = self.slots.get_mut(id).ok_or(invalid)?;
            if slot.storage != storage {
                return Err(invalid);
            }
            if !slot.representations.contains(&data) {
                let mut representations = slot.representations.to_vec();
                representations.push(data);
                representations.sort_unstable();
                slot.representations = representations.into();
            }
            return Ok(id);
        }
        let id = self.slots.insert(Slot {
            key,
            storage,
            representations: Box::new([data]),
        });
        self.keys.insert(key, id);
        Ok(id)
    }
    pub(super) fn root(
        &mut self,
        holder: Holder,
        selected: TypeId,
        partial: bool,
    ) -> Result<Place, Failure> {
        let (key, root, storage) = match holder {
            Holder::Local(local) => {
                let item = self
                    .checked
                    .typed()
                    .body()
                    .locals()
                    .get(local)
                    .ok_or_else(|| self.invalid())?;
                (
                    SlotKey::Local(local),
                    self.data(item.data)?,
                    form(item.access().mode()),
                )
            }
            Holder::Expression(expression) => {
                let item = self
                    .checked
                    .typed()
                    .body()
                    .expressions()
                    .get(expression)
                    .and_then(|e| e.ty.access())
                    .ok_or_else(|| self.invalid())?;
                (
                    SlotKey::Expression(expression),
                    self.data(item.data())?,
                    form(item.mode()),
                )
            }
            Holder::Prepared { call, index } => {
                let root = if partial {
                    self.expression(
                        self.arguments(call)?
                            .get(index)
                            .ok_or_else(|| self.invalid())?
                            .value,
                    )?
                } else {
                    selected
                };
                (SlotKey::Prepared { call, index }, root, Storage::Payload)
            }
            Holder::Temporary { call, index } => {
                let root = self.expression(
                    self.arguments(call)?
                        .get(index)
                        .ok_or_else(|| self.invalid())?
                        .value,
                )?;
                (SlotKey::Temporary { call, index }, root, Storage::Payload)
            }
            Holder::Return => (
                SlotKey::Return,
                self.data(self.checked.typed().result_data())?,
                Storage::Payload,
            ),
        };
        let slot = self.slot(key, root, storage)?;
        Ok(Place {
            slot,
            root,
            data: selected,
            projections: Box::new([]),
        })
    }
    fn place(&mut self, source: &flow::resources::Place, data: TypeId) -> Result<Place, Failure> {
        let mut place = self.root(source.holder(), data, !source.fields().is_root())?;
        place.projections = source
            .fields()
            .fields()
            .iter()
            .copied()
            .map(Projection::Field)
            .collect();
        Ok(place)
    }
    pub(super) fn selected(
        &mut self,
        id: nocter_model::AccessPlaceId,
        data: TypeId,
    ) -> Result<Place, Failure> {
        let source = self
            .checked
            .typed()
            .body()
            .places()
            .get(id)
            .ok_or_else(|| self.invalid())?
            .clone();
        let mut place = self.root(Holder::Local(source.local), data, !source.fields.is_root())?;
        place.projections = source
            .fields
            .fields()
            .iter()
            .copied()
            .map(Projection::Field)
            .collect();
        Ok(place)
    }
    pub(super) fn argument(
        &mut self,
        call: AccessExpressionId,
        index: usize,
    ) -> Result<Place, Failure> {
        let argument = self
            .arguments(call)?
            .get(index)
            .ok_or_else(|| self.invalid())?;
        let data = self.expression(argument.value)?;
        let slot = self.slot(
            SlotKey::Argument { call, index },
            data,
            form(argument.operation.access_mode()),
        )?;
        Ok(Place {
            slot,
            root: data,
            data,
            projections: Box::new([]),
        })
    }
    fn disposal(&self, payload: Payload) -> Result<Disposal, Failure> {
        Ok(match payload.disposal() {
            CheckedDisposal::Trivial => Disposal::Trivial,
            CheckedDisposal::OwnedError => Disposal::Error,
            CheckedDisposal::Record => Disposal::Record,
            CheckedDisposal::OwnedCapture => Disposal::Capture,
            CheckedDisposal::FunctionEnvironment => return Err(self.invalid()),
        })
    }
    #[allow(clippy::too_many_lines)]
    fn effects(
        &mut self,
        steps: &[lifecycle::Step],
        call: Option<AccessExpressionId>,
    ) -> Result<Box<[Effect]>, Failure> {
        let mut output = Vec::new();
        for step in steps {
            match step {
                lifecycle::Step::DisposeEnvironment {
                    argument,
                    part,
                    payload,
                } => {
                    let call = call.ok_or_else(|| self.invalid())?;
                    let mut source = self.argument(call, *argument)?;
                    if *part == EnvironmentPart::Capture {
                        source.projections = Box::new([Projection::Environment]);
                        source.data = payload.data();
                    }
                    output.push(Effect::Dispose {
                        source,
                        disposal: self.disposal(*payload)?,
                    });
                }
                lifecycle::Step::Resource(step) => match step {
                    Step::Capture {
                        source,
                        destination,
                        environment,
                        payload,
                    } => {
                        let source = self.place(source, environment.data())?;
                        let mut destination = self.root(*destination, payload.data(), false)?;
                        destination.projections = Box::new([Projection::Environment]);
                        destination.data = environment.data();
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::AdaptFunction {
                        source,
                        destination,
                        from,
                        payload,
                    } => {
                        let source = self.place(source, from.data())?;
                        let destination = self.root(*destination, payload.data(), false)?;
                        output.push(Effect::Reinterpret {
                            source,
                            destination,
                        });
                    }
                    Step::Assemble {
                        destination,
                        payload,
                        fields,
                    } => {
                        let destination = self.root(*destination, payload.data(), false)?;
                        for (field, source) in fields {
                            let data = self
                                .instances
                                .unit()
                                .program()
                                .fields()
                                .get(*field)
                                .ok_or_else(|| self.invalid())?
                                .data();
                            let source = self.place(source, self.data(data)?)?;
                            let mut selection = destination.clone();
                            selection.projections = Box::new([Projection::Field(*field)]);
                            selection.data = source.data;
                            output.push(Effect::Transfer {
                                source,
                                destination: selection,
                            });
                        }
                        output.push(Effect::Initialize(destination));
                    }
                    Step::Construct {
                        destination,
                        payload,
                    } => {
                        let destination = self.root(*destination, payload.data(), false)?;
                        output.push(Effect::Initialize(destination));
                    }
                    Step::Receive {
                        call,
                        destination,
                        payload,
                    } => {
                        let data = payload.data();
                        let slot = self.slot(SlotKey::Answer(*call), data, Storage::Payload)?;
                        let source = Place {
                            slot,
                            root: data,
                            data,
                            projections: Box::new([]),
                        };
                        let destination = self.root(*destination, data, false)?;
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::Move {
                        source,
                        destination,
                        payload,
                    } => {
                        let source = self.place(source, payload.data())?;
                        let destination = self.place(destination, payload.data())?;
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::Dispose { source, payload } => {
                        let source = self.place(source, payload.data())?;
                        output.push(Effect::Dispose {
                            source,
                            disposal: self.disposal(*payload)?,
                        });
                    }
                    Step::ReleaseContainer { source, payload } => {
                        output.push(Effect::End(self.place(source, payload.data())?));
                    }
                    Step::Invoke {
                        source,
                        call,
                        index,
                        payload,
                    } => {
                        let source = self.place(source, payload.data())?;
                        let destination = self.argument(*call, *index)?;
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::ReplaceInput {
                        local,
                        fields,
                        source,
                        payload,
                    } => {
                        let mut destination =
                            self.root(Holder::Local(*local), payload.data(), !fields.is_root())?;
                        destination.projections = fields
                            .fields()
                            .iter()
                            .copied()
                            .map(Projection::Field)
                            .collect();
                        output.push(Effect::Dispose {
                            source: destination.clone(),
                            disposal: self.disposal(*payload)?,
                        });
                        let source = self.place(source, payload.data())?;
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                },
            }
        }
        Ok(output.into())
    }
    fn call_bindings(&mut self, call: AccessExpressionId) -> Result<Box<[Effect]>, Failure> {
        let arguments = self.arguments(call)?.to_vec();
        let mut effects = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            let destination = self.argument(call, index)?;
            if argument.operation == ValueInputOperation::Take {
                continue;
            }
            let source = match argument.action {
                ArgumentAction::ReserveRead(place) | ArgumentAction::ReserveUpdate(place) => {
                    self.selected(place, destination.data)?
                }
                ArgumentAction::ReserveTransfer { .. } | ArgumentAction::AccessOwnedValue => {
                    self.root(Holder::Temporary { call, index }, destination.data, false)?
                }
                _ => return Err(self.invalid()),
            };
            effects.push(Effect::BindAddress {
                destination: destination.slot,
                source,
            });
        }
        Ok(effects.into())
    }
    fn core(&mut self, operation: &Operation) -> Result<(), Failure> {
        match *operation {
            Operation::Evaluate(id)
            | Operation::BooleanConstant { result: id, .. }
            | Operation::Forward { result: id, .. }
            | Operation::BooleanAnswer { result: id, .. } => {
                if let Some(access) = self
                    .checked
                    .typed()
                    .body()
                    .expressions()
                    .get(id)
                    .and_then(|e| e.ty.normal())
                {
                    let data = self.data(access.data())?;
                    self.slot(SlotKey::Expression(id), data, form(access.mode()))?;
                }
            }
            Operation::Bind { local, .. } => {
                let item = self
                    .checked
                    .typed()
                    .body()
                    .locals()
                    .get(local)
                    .ok_or_else(|| self.invalid())?;
                let data = self.data(item.data)?;
                self.root(Holder::Local(local), data, false)?;
            }
            Operation::PrepareOperand {
                expression,
                operand,
                value,
            } => {
                let data = self.expression(value)?;
                self.slot(
                    SlotKey::Operand {
                        expression,
                        index: operand,
                    },
                    data,
                    Storage::Payload,
                )?;
            }
            Operation::Condition(value) => {
                let data = self.expression(value)?;
                self.slot(SlotKey::Predicate(value), data, Storage::Payload)?;
            }
            Operation::BeginEvaluation(_)
            | Operation::EndEvaluation(_)
            | Operation::PrepareArgument { .. }
            | Operation::CommitCall(_)
            | Operation::CancelCall { .. }
            | Operation::Assign { .. }
            | Operation::CompoundAssign { .. }
            | Operation::Discard(_)
            | Operation::Drop(_)
            | Operation::StageReturn(_)
            | Operation::ExitScope { .. } => {}
        }
        Ok(())
    }
}

fn form(mode: ValueAccessMode) -> Storage {
    match mode {
        ValueAccessMode::Own => Storage::Payload,
        ValueAccessMode::Read | ValueAccessMode::Update => Storage::Address,
    }
}
fn bits(values: &[lifecycle::flags::BitValue]) -> Box<[BitValue]> {
    values
        .iter()
        .map(|value| BitValue {
            word: value.word(),
            mask: value.mask(),
            value: value.value(),
        })
        .collect()
}

pub(super) fn body(
    instances: &InstanceProgram,
    id: AccessInstanceId,
    lifecycle: &lifecycle::Body,
) -> Result<Body, Failure> {
    let invalid = Failure::InvalidBody(id);
    let instance = instances.instances().get(id).ok_or(invalid)?;
    let checked = instances.body(id).ok_or(invalid)?;
    let signature = storage::signature(instances, id).map_err(Failure::Storage)?;
    let mut builder = Builder {
        id,
        instances,
        instance,
        checked,
        slots: ArenaBuilder::new(),
        keys: BTreeMap::new(),
    };
    for input in signature.ordered() {
        builder.root(Holder::Local(input.local), input.data, false)?;
    }
    let mut actions = BTreeMap::new();
    for (&at, action) in lifecycle.actions() {
        let operation = &checked
            .graph()
            .points()
            .get(at.point)
            .and_then(|p| p.actions.get(at.action))
            .ok_or(invalid)?
            .operation;
        builder.core(operation)?;
        let call = if let Operation::CommitCall(call) = *operation {
            Some(call)
        } else {
            None
        };
        let alternatives = action
            .alternatives()
            .iter()
            .map(|alternative| {
                Ok(Alternative {
                    tests: bits(alternative.flags().tests()),
                    before: builder.effects(alternative.before(), call)?,
                    before_writes: bits(alternative.flags().before_writes()),
                    after: builder.effects(alternative.after(), call)?,
                    after_writes: bits(alternative.flags().after_writes()),
                })
            })
            .collect::<Result<Box<[_]>, Failure>>()?;
        let call_bindings = call
            .map(|call| builder.call_bindings(call))
            .transpose()?
            .unwrap_or_default();
        let operations = builder.computation(operation)?;
        actions.insert(
            Location {
                point: at.point,
                action: at.action,
            },
            Action {
                call_bindings,
                operations,
                alternatives,
            },
        );
    }
    let mut terminators = BTreeMap::new();
    for (point, block) in checked.graph().points().iter() {
        if !checked.initialization().reachable(point) {
            continue;
        }
        let terminator = match block.terminator {
            flow::Terminator::Jump(next) => super::Terminator::Jump(next),
            flow::Terminator::Branch { condition, yes, no } => super::Terminator::Branch {
                condition: builder.predicate(condition)?,
                yes,
                no,
            },
            flow::Terminator::Return => {
                let data = signature.result();
                super::Terminator::Return(builder.root(Holder::Return, data, false)?)
            }
            flow::Terminator::Unreachable => super::Terminator::Unreachable,
        };
        terminators.insert(point, terminator);
    }
    let inputs = signature
        .ordered()
        .map(|input| {
            builder
                .keys
                .get(&SlotKey::Local(input.local))
                .copied()
                .ok_or(invalid)
        })
        .collect::<Result<Box<[_]>, _>>()?;
    Ok(Body {
        signature,
        slots: builder.slots.finish(),
        flags: lifecycle.flags().initial_words().into(),
        actions,
        entry: checked.graph().entry(),
        terminators,
        inputs,
    })
}
