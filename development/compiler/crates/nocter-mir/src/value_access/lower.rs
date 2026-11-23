mod scoped;
use super::{
    Action, Alternative, BitValue, Body, Disposal, Effect, Failure, FutureInputDisposal,
    FutureMoveInput, Location, Place, Projection, Slot, SlotKey, Storage,
};
use nocter_checking::value_access::{
    ArgumentAction, ExpressionKind, ValueSourceSlot,
    accepted::{FutureCaptureCases, FutureInputCleanup, FutureSelectorMove},
    flow::{
        self, Operation,
        resources::{
            ActivationAccessSource, ActivationInput, Disposal as CheckedDisposal, Holder, Step,
        },
    },
};
use nocter_model::{AccessExpressionId, AccessInstanceId, MirLocalId, TypeId};
use nocter_model::{ArenaBuilder, ValueAccessMode, ValueInputOperation};
use nocter_runtime_contract::RuntimeDataId;
use nocter_target_program::value_access::{
    Drive, FunctionInput, Instance, InstanceProgram, Payload, lifecycle::EnvironmentPart,
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
    pub(super) fn drive_target(&self) -> Result<&Drive, Failure> {
        self.at
            .and_then(|at| self.instance.drive(at))
            .ok_or_else(|| self.invalid())
    }
    pub(super) fn local_future_expression(&self, id: AccessExpressionId) -> bool {
        self.checked
            .typed()
            .body()
            .expressions()
            .get(id)
            .and_then(|expression| expression.ty.normal())
            .is_some_and(|access| self.local_future_data(access.data()))
    }
    fn local_future_data(&self, data: TypeId) -> bool {
        self.instances
            .unit()
            .local_future_data(self.instance.key().code(), data)
    }
    pub(super) fn future_selector(&mut self, holder: Holder) -> Result<Place, Failure> {
        let data = self
            .instances
            .unit()
            .program()
            .data_type(Drive::SELECTOR_TYPE)
            .ok_or_else(|| self.invalid())?;
        let storage = self.closed_data(data)?;
        self.snapshot(SlotKey::FutureSelector(holder), storage)
    }
    pub(super) fn future_input(
        &mut self,
        owner: Holder,
        index: usize,
        input: FunctionInput,
    ) -> Result<Place, Failure> {
        let data = self.closed_data(input.data)?;
        let slot = self.slot(
            SlotKey::FutureInput {
                owner,
                index,
                data: input.data,
                operation: input.operation,
            },
            data,
            form(input.operation.access_mode()),
        )?;
        Ok(Place {
            slot,
            root: data,
            data,
            projections: Box::new([]),
        })
    }
    fn future_transfers(
        &mut self,
        source: Holder,
        destination: Holder,
        fields: &[FunctionInput],
    ) -> Result<Box<[FutureMoveInput]>, Failure> {
        fields
            .iter()
            .copied()
            .enumerate()
            .map(|(index, input)| {
                Ok(FutureMoveInput {
                    source: self.future_input(source, index, input)?,
                    destination: self.future_input(destination, index, input)?,
                    operation: input.operation,
                })
            })
            .collect()
    }
    fn future_input_cleanup(
        &mut self,
        source: Holder,
        cleanup: &[FutureInputCleanup],
    ) -> Result<Box<[FutureInputDisposal]>, Failure> {
        cleanup
            .iter()
            .map(|input| {
                Ok(FutureInputDisposal {
                    source: self.future_input(
                        source,
                        input.position,
                        FunctionInput {
                            data: input.payload.data(),
                            operation: ValueInputOperation::Take,
                        },
                    )?,
                    disposal: self.disposal(input.payload.disposal())?,
                })
            })
            .collect()
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
    pub(super) fn case_data(
        &self,
        case: &nocter_target_program::value_access::InvocationCase,
    ) -> Result<RuntimeDataId, Failure> {
        self.storage
            .instance(self.instances.unit().program(), &case.callee)
            .map_err(Failure::Storage)
    }
    fn payload_data(&self, payload: &Payload) -> Result<RuntimeDataId, Failure> {
        self.storage
            .instance(self.instances.unit().program(), payload.instance())
            .map_err(Failure::Storage)
    }

    fn receiving_root(&self, holder: Holder) -> Result<Option<RuntimeDataId>, Failure> {
        let Holder::Expression(expression) = holder else {
            return Ok(None);
        };
        self.instance
            .receipt(expression)
            .map(|key| {
                self.storage
                    .instance(self.instances.unit().program(), key)
                    .map_err(Failure::Storage)
            })
            .transpose()
    }
    pub(super) fn receiving_data(
        &self,
        expression: AccessExpressionId,
    ) -> Result<RuntimeDataId, Failure> {
        self.receiving_root(Holder::Expression(expression))?
            .map_or_else(
                || self.definition(ValueSourceSlot::Expression(expression)),
                Ok,
            )
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
    #[allow(
        clippy::too_many_lines,
        reason = "Keep holder roles on one exhaustive storage mapping."
    )]
    pub(super) fn root(
        &mut self,
        holder: Holder,
        selected: RuntimeDataId,
        partial: bool,
    ) -> Result<Place, Failure> {
        // A projection has the data of the selected field but uses the receiving carrier's root.
        let receiving = self.receiving_root(holder)?;
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
                    if !partial {
                        selected
                    } else if let Some(receiving) = receiving {
                        receiving
                    } else {
                        self.closed_data(item.data())?
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
        if receiving
            .is_some_and(|expected| !self.runtime.has_same_value_representation(root, expected))
        {
            return Err(self.invalid());
        }
        let slot = self.slot(key, root, storage)?;
        Ok(Place {
            slot,
            root,
            data: selected,
            projections: Box::new([]),
        })
    }
    fn merge_receipt(
        &mut self,
        destination: &flow::resources::Place,
        source: &Place,
    ) -> Result<Option<Effect>, Failure> {
        let Some(receiving) = self.receiving_root(destination.holder())? else {
            return Ok(None);
        };
        if !destination.fields().is_root() {
            return Err(self.invalid());
        }
        if self
            .runtime
            .has_same_value_representation(source.data, receiving)
        {
            return Ok(None);
        }
        if !self.runtime.permits_value_transport(source.data, receiving) {
            return Err(self.invalid());
        }
        Ok(Some(Effect::Transport {
            source: source.clone(),
            destination: self.root(destination.holder(), receiving, false)?,
        }))
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
        place.projections = self.resource_path(source.fields())?;
        Ok(place)
    }
    fn resource_path(
        &mut self,
        path: &flow::resources::Path,
    ) -> Result<Box<[Projection]>, Failure> {
        path.fields()
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
            .collect()
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
    fn disposal(&self, checked: CheckedDisposal) -> Result<Disposal, Failure> {
        Ok(match checked {
            CheckedDisposal::Trivial => Disposal::Trivial,
            CheckedDisposal::OwnedError => Disposal::Error,
            CheckedDisposal::Aggregate => Disposal::Aggregate,
            CheckedDisposal::OwnedCapture => Disposal::Capture,
            CheckedDisposal::CallableAlternatives => Disposal::CallableAlternatives,
            CheckedDisposal::FunctionEnvironment | CheckedDisposal::UnstartedFuture(_) => {
                return Err(self.invalid());
            }
        })
    }
    fn constrain_geometry(
        &mut self,
        before: RuntimeDataId,
        after: RuntimeDataId,
    ) -> Result<(), Failure> {
        self.constrain_geometry_requirement(before, after, super::GeometryRequirement::Complete)
    }
    fn constrain_geometry_requirement(
        &mut self,
        before: RuntimeDataId,
        after: RuntimeDataId,
        requirement: super::GeometryRequirement,
    ) -> Result<(), Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        self.geometry.insert(super::GeometryConstraint {
            at: Location {
                point: at.point,
                action: at.action,
            },
            before,
            after,
            requirement,
        });
        Ok(())
    }
    fn repack_exclusion(
        &mut self,
        before: RuntimeDataId,
        slot: MirLocalId,
        hole: &flow::resources::Missing,
    ) -> Result<super::RemainderExclusion, Failure> {
        let (path, frontier) = match hole {
            flow::resources::Missing::Value(path) => (path, None),
            flow::resources::Missing::ArrayPrefix { array, frontier } => (array, Some(*frontier)),
        };
        let projections = self.resource_path(path)?;
        let data = super::repack::hole_data(self.runtime, before, &projections)
            .ok_or_else(|| self.invalid())?;
        let place = Place {
            slot,
            root: before,
            projections,
            data,
        };
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
    }
    fn repack(
        &mut self,
        original: &Step<Payload>,
        before: RuntimeDataId,
        source: &Place,
        destination: &Place,
        holes: &[flow::resources::Missing],
    ) -> Result<Option<Effect>, Failure> {
        if before == destination.root {
            return Ok(None);
        }
        let at = self.at.ok_or_else(|| self.invalid())?;
        let Step::Move {
            source: supplied,
            destination: target,
            ..
        } = original
        else {
            return Err(self.invalid());
        };
        let Holder::Local(local) = target.holder() else {
            return Ok(None);
        };
        let Some(Operation::Assign {
            target: referent, ..
        }) = self
            .checked
            .graph()
            .points()
            .get(at.point)
            .and_then(|block| block.actions.get(at.action))
            .map(|action| &action.operation)
        else {
            return Ok(None);
        };
        let access = self.checked.access();
        let root = flow::access::Root::Owned(local);
        let unretained = match referent {
            // The original bounds/selection computation has finished. The resource receipt uses
            // its sealed logical coordinate with the new root, never the staged old pointer.
            nocter_checking::value_access::Referent::Evaluated(_) => {
                access.root_unretained_after(at, root)
            }
            nocter_checking::value_access::Referent::Place(_) => access.root_unretained(at, root),
        };
        if supplied.holder() == target.holder() || !unretained {
            return Ok(None);
        }
        let mut missing = BTreeSet::new();
        for hole in holes {
            let path = match hole {
                flow::resources::Missing::Value(path) => path,
                flow::resources::Missing::ArrayPrefix { array, .. } => array,
            };
            if target.fields().is_prefix_of(path) {
                continue;
            }
            if path.is_prefix_of(target.fields()) {
                return Ok(None);
            }
            missing.insert(self.repack_exclusion(before, destination.slot, hole)?);
        }
        let missing = missing.into_iter().collect::<Box<[_]>>();
        Ok(
            super::repack::preserved(self.runtime, before, destination, &missing).map(
                |preserved| Effect::Repack {
                    source: source.clone(),
                    destination: destination.clone(),
                    before,
                    missing,
                    preserved,
                },
            ),
        )
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
                lifecycle::Step::MoveLocalFuture {
                    source,
                    destination,
                    movement,
                } => {
                    match &movement.inputs {
                        FutureCaptureCases::Direct(fields) => {
                            output.extend(
                                self.future_transfers(*source, *destination, fields)?
                                    .into_vec()
                                    .into_iter()
                                    .map(|input| match input.operation {
                                        ValueInputOperation::Take => Effect::Transfer {
                                            source: input.source,
                                            destination: input.destination,
                                        },
                                        ValueInputOperation::Read | ValueInputOperation::Edit => {
                                            Effect::BindAddress {
                                                source: input.source,
                                                destination: input.destination.slot,
                                            }
                                        }
                                    }),
                            );
                        }
                        FutureCaptureCases::Selected(cases) => {
                            let cases = cases
                                .iter()
                                .map(|(tag, fields)| {
                                    Ok(super::FutureCaptureCase {
                                        tag: *tag,
                                        fields: self.future_transfers(
                                            *source,
                                            *destination,
                                            fields,
                                        )?,
                                    })
                                })
                                .collect::<Result<Box<[_]>, Failure>>()?;
                            output.push(Effect::SelectedFutureMove {
                                selector: self.future_selector(*source)?,
                                cases,
                            });
                        }
                    }
                    match movement.selector {
                        Some(FutureSelectorMove::Known(tag)) => {
                            output.push(Effect::SetFutureSelector {
                                destination: self.future_selector(*destination)?,
                                tag,
                            });
                        }
                        Some(FutureSelectorMove::FromSource) => output.push(Effect::Transfer {
                            source: self.future_selector(*source)?,
                            destination: self.future_selector(*destination)?,
                        }),
                        None => {}
                    }
                }
                lifecycle::Step::CancelLocalFuture { source, captures } => match captures {
                    FutureCaptureCases::Direct(cleanup) => {
                        output.extend(
                            self.future_input_cleanup(*source, cleanup)?
                                .into_vec()
                                .into_iter()
                                .map(|input| Effect::Dispose {
                                    source: input.source,
                                    disposal: input.disposal,
                                }),
                        );
                    }
                    FutureCaptureCases::Selected(cases) => {
                        let cases = cases
                            .iter()
                            .map(|(tag, fields)| {
                                Ok(super::FutureCaptureCase {
                                    tag: *tag,
                                    fields: self.future_input_cleanup(*source, fields)?,
                                })
                            })
                            .collect::<Result<_, Failure>>()?;
                        output.push(Effect::SelectedFutureCleanup {
                            selector: self.future_selector(*source)?,
                            cases,
                        });
                    }
                },
                lifecycle::Step::ActivateLocalFuture {
                    destination,
                    inputs,
                    tag,
                } => {
                    for (index, input) in inputs.iter().enumerate() {
                        let destination =
                            self.future_input(*destination, index, input.contract())?;
                        output.push(match input {
                            ActivationInput::Take { source, .. } => Effect::Transfer {
                                source: self.root(source.holder(), destination.data, false)?,
                                destination,
                            },
                            ActivationInput::Access(access) => Effect::BindAddress {
                                source: match access.source {
                                    ActivationAccessSource::Existing(referent) => {
                                        self.referent(referent, destination.data)?
                                    }
                                    ActivationAccessSource::Temporary { call, index } => self
                                        .root(
                                            Holder::Temporary { call, index },
                                            destination.data,
                                            false,
                                        )?,
                                },
                                destination: destination.slot,
                            },
                        });
                    }
                    if let Some(tag) = *tag {
                        output.push(Effect::SetFutureSelector {
                            destination: self.future_selector(*destination)?,
                            tag,
                        });
                    }
                }
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
                        disposal: self.disposal(payload.disposal())?,
                    });
                }
                lifecycle::Step::Resource(resource) => {
                    let projected = resource.data();
                    match projected.operation() {
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
                                    (nocter_runtime_contract::RuntimeType::Optional(_), []) => {
                                        false
                                    }
                                    (_, [(nocter_model::ValueField::Variant(primary), _)]) => {
                                        *primary
                                    }
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
                        Step::Receive { result, payload } => {
                            let data = self.payload_data(payload)?;
                            let slot =
                                self.slot(SlotKey::Answer(*result), data, Storage::Payload)?;
                            let source = Place {
                                slot,
                                root: data,
                                data,
                                projections: Box::new([]),
                            };
                            let destination =
                                self.root(Holder::Expression(*result), data, false)?;
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
                            let source = self.resource_place(
                                source,
                                self.payload_data(payload)?,
                                projected,
                            )?;
                            if let Some(effect) = self.merge_receipt(destination, &source)? {
                                output.push(effect);
                                continue;
                            }
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
                                // Register both selected storage contexts independently of traversal order.
                                self.root(destination.holder(), before, false)?;
                                let destination = self.place_with_root(
                                    destination,
                                    self.payload_data(payload)?,
                                    Some(after),
                                )?;
                                if let Some(effect) = self.repack(
                                    projected.operation(),
                                    before,
                                    &source,
                                    &destination,
                                    resource
                                        .destination_missing()
                                        .ok_or_else(|| self.invalid())?,
                                )? {
                                    output.push(effect);
                                    continue;
                                }
                                self.constrain_geometry(before, after)?;
                                destination
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
                                // This is the original selected address, retained across the
                                // resource phase. Its payload comes from the ordered projection;
                                // there is no fresh operand occurrence to query here.
                                source: self.root(
                                    Holder::Expression(index.selection()),
                                    data,
                                    false,
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
                            let source = self.resource_place(
                                source,
                                self.payload_data(payload)?,
                                projected,
                            )?;
                            output.push(Effect::Dispose {
                                source,
                                disposal: self.disposal(payload.disposal())?,
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
                            let array = self.resource_place(
                                source,
                                self.payload_data(payload)?,
                                projected,
                            )?;
                            let missing = missing
                                .iter()
                                .map(|hole| {
                                    let (path, frontier) = match hole {
                                        flow::resources::Missing::Value(path) => (path, None),
                                        flow::resources::Missing::ArrayPrefix {
                                            array,
                                            frontier,
                                        } => (array, Some(*frontier)),
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
                            let source = self.resource_place(
                                source,
                                self.payload_data(payload)?,
                                projected,
                            )?;
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
                        Step::ConstructActivation { .. }
                        | Step::DriveFuture { .. }
                        | Step::ReplaceReferent { .. } => {
                            return Err(self.invalid());
                        }
                    }
                }
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
                if !self.local_future_expression(id)
                    && let Some(access) = self
                        .checked
                        .typed()
                        .body()
                        .expressions()
                        .get(id)
                        .and_then(|e| e.ty.normal())
                {
                    let data = self.receiving_data(id)?;
                    self.slot(SlotKey::Expression(id), data, form(access.mode()))?;
                }
            }
            Operation::Bind { local, .. } => {
                let local_data = self
                    .checked
                    .typed()
                    .body()
                    .locals()
                    .get(local)
                    .ok_or_else(|| self.invalid())?
                    .data;
                if self.local_future_data(local_data) {
                    return Ok(());
                }
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
            | Operation::ConstructFuture(_)
            | Operation::DriveFuture { .. }
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
