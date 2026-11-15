use super::{
    Action, Alternative, BitValue, Body, Disposal, Effect, Failure, Location, Place, Projection,
    Slot, SlotKey, Storage,
};
use nocter_checking::value_access::{
    ArgumentAction, ExpressionKind, ValueSourceSlot,
    flow::{
        self, Operation,
        resources::{Disposal as CheckedDisposal, Holder, Step},
    },
};
use nocter_model::{AccessExpressionId, AccessInstanceId, MirLocalId, TypeId};
use nocter_model::{ArenaBuilder, ValueAccessMode, ValueInputOperation};
use nocter_runtime_contract::RuntimeDataId;
use nocter_target_program::value_access::{
    Instance, InstanceProgram, Payload, lifecycle::EnvironmentPart,
};
use nocter_target_program::value_access::{lifecycle, storage};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
mod tests;

pub(super) struct Builder<'a> {
    pub(super) runtime: &'a nocter_runtime_contract::RuntimeEnvironment,
    storage: &'a storage::Prepared,
    pub(super) id: AccessInstanceId,
    pub(super) instances: &'a InstanceProgram,
    pub(super) instance: &'a Instance,
    pub(super) checked: &'a flow::Query,
    at: Option<flow::Location>,
    slots: ArenaBuilder<MirLocalId, Slot>,
    keys: BTreeMap<SlotKey, MirLocalId>,
    geometry: BTreeSet<super::GeometryConstraint>,
}
impl Builder<'_> {
    fn control(
        &mut self,
        result: RuntimeDataId,
    ) -> Result<BTreeMap<nocter_model::AccessFlowPointId, super::Terminator>, Failure> {
        let checked = self.checked;
        let mut terminators = BTreeMap::new();
        for (point, _) in checked.graph().points().iter() {
            let Some(selected) = self.instance.control().terminator(point) else {
                continue;
            };
            let terminator = match selected {
                flow::Terminator::Jump(next) => super::Terminator::Jump(next),
                flow::Terminator::Branch { condition, yes, no } => super::Terminator::Branch {
                    condition: self.predicate(condition)?,
                    yes,
                    no,
                },
                flow::Terminator::Return => {
                    let data = result;
                    super::Terminator::Return(self.root(Holder::Return, data, false)?)
                }
                flow::Terminator::Unreachable => super::Terminator::Unreachable,
            };
            terminators.insert(point, terminator);
        }
        Ok(terminators)
    }
    fn static_views(&self) -> Box<[(MirLocalId, nocter_model::ValueStatic)]> {
        self.checked
            .typed()
            .body()
            .static_views()
            .filter_map(|(local, id)| {
                self.keys
                    .get(&SlotKey::Local(local))
                    .map(|&slot| (slot, id))
            })
            .collect()
    }
    pub(super) fn invalid(&self) -> Failure {
        Failure::InvalidBody(self.id)
    }
    pub(super) fn closed_data(&self, source: TypeId) -> Result<RuntimeDataId, Failure> {
        let program = self.instances.unit().program();
        let key = program
            .instantiate_closed_data(source)
            .map_err(|_| self.invalid())?;
        self.storage
            .instance(program, &key)
            .map_err(Failure::Storage)
    }
    pub(super) fn definition(&self, slot: ValueSourceSlot) -> Result<RuntimeDataId, Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        self.payload_data(
            self.instance
                .definition(at, slot)
                .ok_or_else(|| self.invalid())?,
        )
    }
    fn payload_data(&self, payload: &Payload) -> Result<RuntimeDataId, Failure> {
        self.storage
            .instance(self.instances.unit().program(), payload.instance())
            .map_err(Failure::Storage)
    }

    pub(super) fn capture_data(&self, data: RuntimeDataId) -> Result<RuntimeDataId, Failure> {
        let Some(nocter_runtime_contract::RuntimeType::StaticCallable {
            environment: Some(environment),
        }) = self.runtime.types().get(data)
        else {
            return Err(self.invalid());
        };
        Ok(*environment)
    }
    fn index_data(&self) -> Result<RuntimeDataId, Failure> {
        let source = self
            .checked
            .typed()
            .program()
            .data_type(nocter_model::BuiltinType::Usize)
            .ok_or_else(|| self.invalid())?;
        self.closed_data(source)
    }
    pub(super) fn expression(&self, id: AccessExpressionId) -> Result<RuntimeDataId, Failure> {
        if let Some(at) = self.at
            && let Some(payload) = self.instance.operand(at, id).or_else(|| {
                self.instance
                    .definition(at, ValueSourceSlot::Expression(id))
            })
        {
            return self.payload_data(payload);
        }
        let access = self
            .checked
            .typed()
            .body()
            .expressions()
            .get(id)
            .and_then(|e| e.ty.access())
            .ok_or_else(|| self.invalid())?;
        // Detached scalar snapshots and retained selectors can request declaration-closed
        // metadata. A missing contextual occurrence must not fall back to the frame type cache.
        self.closed_data(access.data())
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
        data: RuntimeDataId,
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
        selected: RuntimeDataId,
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
                    if !partial {
                        selected
                    } else if let Some(payload) = self.instance.input(local) {
                        self.payload_data(payload)?
                    } else {
                        self.closed_data(item.data)?
                    },
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
                    if partial {
                        self.closed_data(item.data())?
                    } else {
                        selected
                    },
                    form(item.mode()),
                )
            }
            Holder::Prepared { call, index } => {
                let root = if partial {
                    self.prepared_data(call, index)?
                } else {
                    selected
                };
                (SlotKey::Prepared { call, index }, root, Storage::Payload)
            }
            Holder::Temporary { call, index } => (
                SlotKey::Temporary { call, index },
                self.prepared_data(call, index)?,
                Storage::Payload,
            ),
            Holder::IndexSnapshot(index) => {
                if flow::resources::RetainedIndex::for_selection(
                    self.checked.typed(),
                    index.selection(),
                ) != Some(index)
                {
                    return Err(self.invalid());
                }
                (
                    SlotKey::IndexSnapshot(index),
                    self.index_data()?,
                    Storage::Payload,
                )
            }
            Holder::ArrayFrontier(frontier) => {
                frontier
                    .array(self.checked.typed())
                    .ok_or_else(|| self.invalid())?;
                (
                    SlotKey::ArrayFrontier(frontier),
                    self.index_data()?,
                    Storage::Payload,
                )
            }
            Holder::Return => (
                SlotKey::Return,
                self.payload_data(
                    &self
                        .instance
                        .result()
                        .payload()
                        .ok_or_else(|| self.invalid())?,
                )?,
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
    fn place(
        &mut self,
        source: &flow::resources::Place,
        data: RuntimeDataId,
    ) -> Result<Place, Failure> {
        self.place_with_root(source, data, None)
    }
    fn place_with_root(
        &mut self,
        source: &flow::resources::Place,
        data: RuntimeDataId,
        root: Option<RuntimeDataId>,
    ) -> Result<Place, Failure> {
        let mut place = if let Some(root) = root {
            let mut place = self.root(source.holder(), root, false)?;
            place.data = data;
            place
        } else {
            self.root(source.holder(), data, !source.fields().is_root())?
        };
        place.projections = source
            .fields()
            .fields()
            .iter()
            .map(|field| {
                Ok(match *field {
                    nocter_model::ValueProjection::Record(id) => {
                        Projection::Field(nocter_model::ValueField::Record(id))
                    }
                    nocter_model::ValueProjection::Variant(primary) => {
                        Projection::Field(nocter_model::ValueField::Variant(primary))
                    }
                    nocter_model::ValueProjection::Tuple(index) => {
                        Projection::Field(nocter_model::ValueField::Tuple(index))
                    }
                    nocter_model::ValueProjection::Array(flow::resources::Index::Constant(
                        index,
                    )) => Projection::Field(nocter_model::ValueField::Array(index)),
                    nocter_model::ValueProjection::Array(flow::resources::Index::Retained(
                        index,
                    )) => {
                        let data = self.expression(index.operand())?;
                        Projection::Index(
                            self.root(Holder::IndexSnapshot(index), data, false)?.slot,
                        )
                    }
                })
            })
            .collect::<Result<_, Failure>>()?;
        Ok(place)
    }
    fn resource_place(
        &mut self,
        source: &flow::resources::Place,
        data: RuntimeDataId,
        step: &nocter_checking::value_access::ResourceDataStep,
    ) -> Result<Place, Failure> {
        let root = step
            .root(source)
            .map(|key| {
                self.storage
                    .instance(self.instances.unit().program(), key)
                    .map_err(Failure::Storage)
            })
            .transpose()?;
        self.place_with_root(source, data, root)
    }
    pub(super) fn selected(
        &mut self,
        id: nocter_model::AccessPlaceId,
        data: RuntimeDataId,
    ) -> Result<Place, Failure> {
        let source = self
            .checked
            .typed()
            .body()
            .places()
            .get(id)
            .ok_or_else(|| self.invalid())?
            .clone();
        let root = if source.fields.is_root() {
            data
        } else {
            self.referent_root(nocter_checking::value_access::Referent::Place(id))?
        };
        let mut place = self.root(Holder::Local(source.local), root, false)?;
        place.data = data;
        place.projections = source
            .fields
            .fields()
            .iter()
            .copied()
            .map(Projection::Field)
            .collect();
        Ok(place)
    }
    pub(super) fn referent_data(
        &self,
        referent: nocter_checking::value_access::Referent,
    ) -> Result<RuntimeDataId, Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        let root = self
            .instance
            .referent_root(at, referent)
            .ok_or_else(|| self.invalid())?;
        let selected = match referent {
            nocter_checking::value_access::Referent::Place(id) => {
                let place = self
                    .checked
                    .typed()
                    .body()
                    .places()
                    .get(id)
                    .ok_or_else(|| self.invalid())?;
                root.project_path(self.instances.unit().program(), &place.fields)
                    .map_err(|_| self.invalid())?
            }
            nocter_checking::value_access::Referent::Evaluated(_) => root.clone(),
        };
        self.storage
            .instance(self.instances.unit().program(), &selected)
            .map_err(Failure::Storage)
    }
    pub(super) fn referent_root(
        &self,
        referent: nocter_checking::value_access::Referent,
    ) -> Result<RuntimeDataId, Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        let key = self
            .instance
            .referent_root(at, referent)
            .ok_or_else(|| self.invalid())?;
        self.storage
            .instance(self.instances.unit().program(), key)
            .map_err(Failure::Storage)
    }
    fn prepared_data(
        &self,
        call: AccessExpressionId,
        index: usize,
    ) -> Result<RuntimeDataId, Failure> {
        self.payload_data(
            self.instance
                .prepared(call, index)
                .ok_or_else(|| self.invalid())?,
        )
    }
    pub(super) fn argument(
        &mut self,
        call: AccessExpressionId,
        index: usize,
    ) -> Result<Place, Failure> {
        let data = self.prepared_data(call, index)?;
        self.argument_with_data(call, index, data)
    }
    fn argument_with_data(
        &mut self,
        call: AccessExpressionId,
        index: usize,
        data: RuntimeDataId,
    ) -> Result<Place, Failure> {
        let argument = self
            .arguments(call)?
            .get(index)
            .ok_or_else(|| self.invalid())?;
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
    fn disposal(&self, payload: &Payload) -> Result<Disposal, Failure> {
        Ok(match payload.disposal() {
            CheckedDisposal::Trivial => Disposal::Trivial,
            CheckedDisposal::OwnedError => Disposal::Error,
            CheckedDisposal::Aggregate => Disposal::Aggregate,
            CheckedDisposal::OwnedCapture => Disposal::Capture,
            CheckedDisposal::FunctionEnvironment => return Err(self.invalid()),
        })
    }
    fn constrain_geometry(
        &mut self,
        before: RuntimeDataId,
        after: RuntimeDataId,
    ) -> Result<(), Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        self.geometry.insert(super::GeometryConstraint {
            at: Location {
                point: at.point,
                action: at.action,
            },
            before,
            after,
        });
        Ok(())
    }
    fn scoped_replacement(
        &mut self,
        replacement: &lifecycle::ScopedReplacement,
    ) -> Result<[Effect; 2], Failure> {
        let Step::ReplaceReferent {
            destination,
            source,
            retired,
            payload,
        } = replacement.operation()
        else {
            return Err(self.invalid());
        };
        if replacement.root_before().data() != replacement.root_after().data() {
            return Err(self.invalid());
        }
        let program = self.instances.unit().program();
        let carriers = replacement.carriers();
        let after = carriers.after().ok_or_else(|| self.invalid())?;
        if carriers.before().len() != after.len() {
            return Err(self.invalid());
        }
        for (before, after) in carriers.before().iter().zip(after) {
            if before.selection().slot != after.selection().slot
                || before.selection().path != after.selection().path
                || before.instance().data() != after.instance().data()
            {
                return Err(self.invalid());
            }
            self.constrain_geometry(
                self.storage
                    .instance(program, before.instance())
                    .map_err(Failure::Storage)?,
                self.storage
                    .instance(program, after.instance())
                    .map_err(Failure::Storage)?,
            )?;
        }
        let before = self
            .storage
            .instance(program, replacement.root_before())
            .map_err(Failure::Storage)?;
        let after = self
            .storage
            .instance(program, replacement.root_after())
            .map_err(Failure::Storage)?;
        self.constrain_geometry(before, after)?;
        let retired = Effect::Dispose {
            source: self.place_with_root(destination, self.payload_data(retired)?, Some(before))?,
            disposal: self.disposal(retired)?,
        };
        let supplied = self.payload_data(payload)?;
        let transfer = Effect::Transfer {
            source: self.resource_place(source, supplied, replacement.data())?,
            destination: self.place_with_root(destination, supplied, Some(after))?,
        };
        Ok([retired, transfer])
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
                lifecycle::Step::ScopedReplacement(replacement) => {
                    output.extend(self.scoped_replacement(replacement)?);
                }
                lifecycle::Step::DisposeEnvironment {
                    argument,
                    part,
                    carrier,
                    payload,
                } => {
                    let call = call.ok_or_else(|| self.invalid())?;
                    let before = self.argument(call, *argument)?;
                    let data = self.payload_data(carrier)?;
                    self.constrain_geometry(before.root, data)?;
                    let mut source = self.argument_with_data(call, *argument, data)?;
                    if *part == EnvironmentPart::Capture {
                        source.projections = Box::new([Projection::Environment]);
                        source.data = self.capture_data(source.data)?;
                    }
                    if source.data != self.payload_data(payload)? {
                        return Err(self.invalid());
                    }
                    output.push(Effect::Dispose {
                        source,
                        disposal: self.disposal(payload)?,
                    });
                }
                lifecycle::Step::Resource(projected) => match projected.operation() {
                    Step::Capture {
                        source,
                        destination,
                        payload,
                        ..
                    } => {
                        let mut destination =
                            self.root(*destination, self.payload_data(payload)?, false)?;
                        let environment = self.capture_data(destination.data)?;
                        let source = self.resource_place(source, environment, projected)?;
                        destination.projections = Box::new([Projection::Environment]);
                        destination.data = environment;
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
                        let source =
                            self.resource_place(source, self.payload_data(from)?, projected)?;
                        let destination =
                            self.root(*destination, self.payload_data(payload)?, false)?;
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
                        let destination =
                            self.root(*destination, self.payload_data(payload)?, false)?;
                        for (field, source) in fields {
                            let data = self
                                .runtime
                                .value_field(destination.data, *field)
                                .ok_or_else(|| self.invalid())?;
                            let source = self.resource_place(source, data, projected)?;
                            let mut selection = destination.clone();
                            selection.projections = Box::new([Projection::Field(*field)]);
                            selection.data = source.data;
                            output.push(Effect::Transfer {
                                source,
                                destination: selection,
                            });
                        }
                        if let Some(
                            kind @ (nocter_runtime_contract::RuntimeType::Optional(_)
                            | nocter_runtime_contract::RuntimeType::Fallible(_)),
                        ) = self.runtime.types().get(destination.data)
                        {
                            let primary = match (kind, fields.as_ref()) {
                                (nocter_runtime_contract::RuntimeType::Optional(_), []) => false,
                                (_, [(nocter_model::ValueField::Variant(primary), _)]) => *primary,
                                _ => return Err(self.invalid()),
                            };
                            output.push(Effect::VariantTag {
                                destination: destination.clone(),
                                primary,
                            });
                        }
                        output.push(Effect::Initialize(destination));
                    }
                    Step::Construct {
                        destination,
                        payload,
                    } => {
                        let destination =
                            self.root(*destination, self.payload_data(payload)?, false)?;
                        output.push(Effect::Initialize(destination));
                    }
                    Step::Receive {
                        call,
                        destination,
                        payload,
                    } => {
                        let data = self.payload_data(payload)?;
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
                        let source =
                            self.resource_place(source, self.payload_data(payload)?, projected)?;
                        let destination = if let Some(roots) = projected.destination_roots() {
                            if roots.destination() != destination {
                                return Err(self.invalid());
                            }
                            let program = self.instances.unit().program();
                            let before = self
                                .storage
                                .instance(program, roots.before())
                                .map_err(Failure::Storage)?;
                            let after = self
                                .storage
                                .instance(program, roots.after())
                                .map_err(Failure::Storage)?;
                            self.constrain_geometry(before, after)?;
                            self.place_with_root(
                                destination,
                                self.payload_data(payload)?,
                                Some(after),
                            )?
                        } else {
                            self.place(destination, self.payload_data(payload)?)?
                        };
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::MoveArrayElement {
                        index,
                        destination,
                        payload,
                        ..
                    } => {
                        let data = self.payload_data(payload)?;
                        output.push(Effect::Transfer {
                            source: self.referent(
                                nocter_checking::value_access::Referent::Evaluated(
                                    index.selection(),
                                ),
                                data,
                            )?,
                            destination: self.root(*destination, data, false)?,
                        });
                    }
                    Step::ConsolidateArrayPrefix {
                        array,
                        payload,
                        frontier,
                    } => {
                        let data = self.index_data()?;
                        output.push(Effect::ConsolidateArrayPrefix {
                            source: self.resource_place(
                                array,
                                self.payload_data(payload)?,
                                projected,
                            )?,
                            endpoint: self
                                .root(Holder::ArrayFrontier(*frontier), data, false)?
                                .slot,
                        });
                    }
                    Step::Dispose { source, payload } => {
                        let source =
                            self.resource_place(source, self.payload_data(payload)?, projected)?;
                        output.push(Effect::Dispose {
                            source,
                            disposal: self.disposal(payload)?,
                        });
                    }
                    Step::DisposeArrayElements {
                        source,
                        payload,
                        range,
                    } => {
                        output.push(Effect::DisposeArrayElements {
                            source: self.resource_place(
                                source,
                                self.payload_data(payload)?,
                                projected,
                            )?,
                            range: range.try_map_index(|index| {
                                let data = self.index_data()?;
                                Ok(self.root(index.holder(), data, false)?.slot)
                            })?,
                        });
                    }
                    Step::DisposeArrayRemainder {
                        source,
                        payload,
                        missing,
                    } => {
                        let array =
                            self.resource_place(source, self.payload_data(payload)?, projected)?;
                        let missing = missing
                            .iter()
                            .map(|hole| {
                                let (path, frontier) = match hole {
                                    flow::resources::Missing::Value(path) => (path, None),
                                    flow::resources::Missing::ArrayPrefix { array, frontier } => {
                                        (array, Some(*frontier))
                                    }
                                };
                                let data = self
                                    .runtime
                                    .value_path(array.root, path.fields(), |index, length| {
                                        match *index {
                                            flow::resources::Index::Constant(index) => {
                                                index < length
                                            }
                                            flow::resources::Index::Retained(snapshot) => {
                                                snapshot.bound() <= length
                                            }
                                        }
                                    })
                                    .ok_or_else(|| self.invalid())?;
                                let place = self.place_with_root(
                                    &source.clone().with_fields(path.clone()),
                                    data,
                                    Some(array.root),
                                )?;
                                Ok(if let Some(frontier) = frontier {
                                    let data = self.index_data()?;
                                    super::RemainderExclusion::ArrayPrefix {
                                        array: place,
                                        endpoint: self
                                            .root(Holder::ArrayFrontier(frontier), data, false)?
                                            .slot,
                                    }
                                } else {
                                    super::RemainderExclusion::Value(place)
                                })
                            })
                            .collect::<Result<_, Failure>>()?;
                        output.push(Effect::DisposeArrayRemainder {
                            source: array,
                            missing,
                        });
                    }
                    Step::ReleaseContainer { source, payload } => {
                        output.push(Effect::End(self.resource_place(
                            source,
                            self.payload_data(payload)?,
                            projected,
                        )?));
                    }
                    Step::Invoke {
                        source,
                        call,
                        index,
                        payload,
                    } => {
                        let source =
                            self.resource_place(source, self.payload_data(payload)?, projected)?;
                        let destination = self.argument(*call, *index)?;
                        output.push(Effect::Transfer {
                            source,
                            destination,
                        });
                    }
                    Step::Transport {
                        source,
                        destination,
                        from,
                        payload,
                    } => {
                        let source =
                            self.resource_place(source, self.payload_data(from)?, projected)?;
                        let destination =
                            self.root(*destination, self.payload_data(payload)?, false)?;
                        output.push(Effect::Transport {
                            source,
                            destination,
                        });
                    }
                    Step::ReplaceReferent { .. } => {
                        return Err(self.invalid());
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
                    self.referent(place, destination.data)?
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
        let at = self.at.ok_or_else(|| self.invalid())?;
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
                    let payload = self
                        .instance
                        .definition(at, ValueSourceSlot::Expression(id))
                        .ok_or_else(|| self.invalid())?;
                    let data = self.payload_data(payload)?;
                    self.slot(SlotKey::Expression(id), data, form(access.mode()))?;
                }
            }
            Operation::Bind { local, .. } => {
                let payload = self
                    .instance
                    .definition(at, ValueSourceSlot::Local(local))
                    .ok_or_else(|| self.invalid())?;
                let data = self.payload_data(payload)?;
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
            Operation::IndexFailure(_)
            | Operation::BeginEvaluation(_)
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
    storage: &storage::Prepared,
    id: AccessInstanceId,
    lifecycle: &lifecycle::Body,
) -> Result<Body, Failure> {
    let invalid = Failure::InvalidBody(id);
    let instance = instances.instances().get(id).ok_or(invalid)?;
    let checked = instances.body(id).ok_or(invalid)?;
    let signature = storage::signature(instances, storage, id).map_err(Failure::Storage)?;
    let mut builder = Builder {
        runtime: storage.environment(),
        storage,
        id,
        instances,
        instance,
        checked,
        at: None,
        slots: ArenaBuilder::new(),
        keys: BTreeMap::new(),
        geometry: BTreeSet::new(),
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
        builder.at = Some(at);
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
    builder.at = None;
    let terminators = builder.control(signature.result())?;
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
    let statics = builder.static_views();
    Ok(Body {
        statics,
        signature,
        slots: builder.slots.finish(),
        flags: lifecycle.flags().initial_words().into(),
        actions,
        entry: checked.graph().entry(),
        terminators,
        inputs,
        geometry: builder.geometry.into_iter().collect(),
    })
}
