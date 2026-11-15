//! Allocate only slot metadata from common ordered data projection, never values or owners.
use super::{Builder, INVALID, Key, Result};
use crate::value_access::ValueSourceSlot;
use crate::value_access::flow::{
    self,
    resources::{Holder, InstantiatedPayload, PayloadSite, Step},
};
use nocter_constant_evaluation::state as plan;
use nocter_declarations::value_access::DataInstanceKey;

pub(super) struct Projected {
    pub(super) before: Vec<crate::value_access::ResourceDataStep>,
    pub(super) after: Vec<crate::value_access::ResourceDataStep>,
    pub(super) roots: Vec<Roots>,
}
pub(super) struct Roots {
    destination: crate::value_access::flow::resources::Place,
    before: DataInstanceKey,
    after: DataInstanceKey,
    carriers: crate::value_access::ReferentCarrierContracts,
}

fn source_holder(slot: ValueSourceSlot) -> Result<Holder> {
    match slot {
        ValueSourceSlot::Local(local) => Ok(Holder::Local(local)),
        ValueSourceSlot::Expression(expression) => Ok(Holder::Expression(expression)),
        ValueSourceSlot::Argument { .. } | ValueSourceSlot::Return => Err(INVALID),
    }
}

impl Builder<'_, '_> {
    pub(super) fn prepare_value_data(&mut self) -> Result<()> {
        let definitions = self
            .values
            .definitions()
            .map(|((_, slot), payload)| {
                let holder = source_holder(slot)?;
                Ok((holder, payload.instance().clone()))
            })
            .collect::<Result<Vec<_>>>()?;
        for (holder, data) in definitions {
            self.register_holder(holder, &data)?;
        }
        Ok(())
    }
    pub(super) fn prepare_resource_data(&mut self) -> Result<()> {
        let query = self.query;
        for (point, block) in query.graph().points().iter() {
            if !query.initialization().reachable(point) {
                continue;
            }
            for action in 0..block.actions.len() {
                let at = flow::Location { point, action };
                let mut alternatives = Vec::new();
                for transition in query.access().resources().at(at) {
                    let mut projection =
                        self.checked
                            .project_resource_data(self.instance, at, self.work)?;
                    let roots = transition
                        .before_steps()
                        .iter()
                        .chain(transition.after_steps())
                        .filter_map(|step| match step {
                            Step::ReplaceReferent { destination, .. } => Some(destination),
                            _ => None,
                        })
                        .map(|destination| {
                            let (target, carriers) =
                                projection.replacement_carriers(destination)?;
                            Ok(Roots {
                                destination: destination.clone(),
                                before: self.values.root(at, target).ok_or(INVALID)?.clone(),
                                after: self.values.root_after(at, target).ok_or(INVALID)?.clone(),
                                carriers,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let before = projection.steps_with_roots(transition.before_steps())?;
                    let after = projection.after_steps_with_roots(transition.after_steps())?;
                    drop(projection);
                    // Locals may observe multiple referent contexts, but source-boundary
                    // constraints and actual receipts must still match the plan's exact phases.
                    // Registration supplies no initialized value or enclosing-allocation geometry.
                    for root in &roots {
                        self.register_holder(root.destination.holder(), &root.before)?;
                        self.register_holder(root.destination.holder(), &root.after)?;
                        for carrier in root
                            .carriers
                            .before()
                            .iter()
                            .chain(root.carriers.after().into_iter().flatten())
                        {
                            self.register_holder(
                                source_holder(carrier.selection().slot)?,
                                carrier.instance(),
                            )?;
                        }
                    }
                    for step in before.iter().chain(&after) {
                        self.resource_data(step.operation())?;
                        if let Some(roots) = step.destination_roots() {
                            self.register_holder(roots.destination().holder(), roots.before())?;
                            self.register_holder(roots.destination().holder(), roots.after())?;
                        }
                    }
                    alternatives.push(Projected {
                        before,
                        after,
                        roots,
                    });
                }
                if self.resources.insert(at, alternatives).is_some() {
                    return Err(INVALID);
                }
            }
        }
        Ok(())
    }
    fn register_holder(&mut self, holder: Holder, key: &DataInstanceKey) -> Result<()> {
        let (_, storage) = self.holder_contract(holder)?;
        let data = self.types.get_instance(self.program, key.clone())?;
        self.slot_data(Key::Resource(holder), data, storage)?;
        Ok(())
    }
    fn root_context(&mut self, holder: Holder, data: &DataInstanceKey) -> Result<plan::Effect> {
        Ok(plan::Effect::Context {
            place: plan::Place::root(self.holder(holder)?),
            data: self.types.get_instance(self.program, data.clone())?,
        })
    }
    pub(super) fn source_contexts(
        &mut self,
        projected: &Projected,
    ) -> Result<(Vec<plan::Effect>, Vec<plan::Effect>)> {
        // These are source-action entry/completion boundaries, enclosing all resource phases;
        // they neither guess a step's phase nor prove initialized backing or stable addresses.
        let mut entry = Vec::new();
        let mut completion = Vec::new();
        for root in &projected.roots {
            entry.push(self.root_context(root.destination.holder(), &root.before)?);
            completion.push(self.root_context(root.destination.holder(), &root.after)?);
            for carrier in root.carriers.before() {
                entry.push(
                    self.root_context(
                        source_holder(carrier.selection().slot)?,
                        carrier.instance(),
                    )?,
                );
            }
            for carrier in root.carriers.after().ok_or(INVALID)? {
                completion.push(
                    self.root_context(
                        source_holder(carrier.selection().slot)?,
                        carrier.instance(),
                    )?,
                );
            }
        }
        Ok((entry, completion))
    }
    pub(super) fn effects(
        &mut self,
        original: &[Step],
        projected: &[crate::value_access::ResourceDataStep],
        roots: &[Roots],
    ) -> Result<Vec<plan::Effect>> {
        if original.len() != projected.len() {
            return Err(INVALID);
        }
        let mut effects = Vec::new();
        for (original, selected) in original.iter().zip(projected) {
            let projected = selected.operation();
            let mut before = Vec::new();
            let mut after = Vec::new();
            let actual = projected.try_map_payloads_at::<super::Failure, ()>(|site, payload| {
                let data = self
                    .types
                    .get_instance(self.program, payload.instance().clone())?;
                match site {
                    PayloadSite::Source(place) => before.push(plan::Effect::Context {
                        place: self.resource(place)?,
                        data,
                    }),
                    PayloadSite::Destination(holder) => after.push(plan::Effect::Context {
                        place: plan::Place::root(self.holder(holder)?),
                        data,
                    }),
                }
                Ok(())
            })?;
            let expected = original.try_map_payloads_at::<super::Failure, ()>(|_, _| Ok(()))?;
            if actual != expected {
                return Err(INVALID);
            }
            let mut effect = self.effect(original)?;
            if let Step::Move {
                destination,
                payload,
                ..
            } = projected
            {
                let data = self
                    .types
                    .get_instance(self.program, payload.instance().clone())?;
                if let plan::Effect::Move {
                    source,
                    destination: target,
                } = &effect
                    && self.holder_contract(destination.holder())?.1 == plan::Storage::Owned
                {
                    if let Some(roots) = selected.destination_roots() {
                        if roots.destination() != destination {
                            return Err(INVALID);
                        }
                        before.push(self.root_context(destination.holder(), roots.before())?);
                        let root = self
                            .types
                            .get_instance(self.program, roots.after().clone())?;
                        after.push(self.root_context(destination.holder(), roots.after())?);
                        effect = plan::Effect::MoveRoot {
                            source: source.clone(),
                            destination: target.clone(),
                            data: root,
                        };
                    } else if target.fields.is_empty() {
                        effect = plan::Effect::MoveRoot {
                            source: source.clone(),
                            destination: target.clone(),
                            data,
                        };
                    }
                }
                after.push(plan::Effect::Context {
                    place: self.resource(destination)?,
                    data,
                });
            } else if let Step::ReplaceReferent {
                destination,
                payload,
                ..
            } = projected
            {
                let mut matched = roots.iter().filter(|root| root.destination == *destination);
                let root = matched.next().ok_or(INVALID)?;
                if matched.next().is_some() {
                    return Err(INVALID);
                }
                if root.before != root.after {
                    effect = self.referent_receipt(&effect, root)?;
                }
                after.push(plan::Effect::Context {
                    place: self.resource(destination)?,
                    data: self
                        .types
                        .get_instance(self.program, payload.instance().clone())?,
                });
            }
            effects.extend(before);
            effects.push(effect);
            effects.extend(after);
        }
        Ok(effects)
    }
    fn referent_receipt(&mut self, effect: &plan::Effect, root: &Roots) -> Result<plan::Effect> {
        let plan::Effect::Replace {
            source,
            destination,
        } = effect
        else {
            return Err(INVALID);
        };
        let [before] = root.carriers.before() else {
            return Err(INVALID);
        };
        let [after] = root.carriers.after().ok_or(INVALID)? else {
            return Err(INVALID);
        };
        if before.selection().slot != after.selection().slot
            || before.selection().path != after.selection().path
        {
            return Err(INVALID);
        }
        let owner = source_holder(before.selection().slot)?;
        if self.holder_contract(owner)?.1 != plan::Storage::Owned {
            return Err(INVALID);
        }
        let fields = self.carrier_fields(before)?;
        Ok(plan::Effect::ReplaceReferent {
            source: source.clone(),
            destination: destination.clone(),
            owner: plan::Place {
                slot: self.holder(owner)?,
                fields: fields
                    .into_iter()
                    .chain(destination.fields.iter().copied())
                    .collect(),
            },
            owner_data: self
                .types
                .get_instance(self.program, after.instance().clone())?,
            view_data: self.types.get_instance(self.program, root.after.clone())?,
        })
    }
    fn carrier_fields(
        &self,
        carrier: &crate::value_access::ReferentCarrierData,
    ) -> Result<Vec<plan::Projection>> {
        use crate::value_access::IndexFact;
        use nocter_declarations::value_access::InstanceSchema;
        use nocter_model::{ValueField, ValueProjection};
        let mut instance = carrier.instance().clone();
        let mut fields = Vec::new();
        for projection in carrier.selection().path.fields() {
            let field = match *projection {
                ValueProjection::Record(id) => ValueField::Record(id),
                ValueProjection::Tuple(index) => ValueField::Tuple(index),
                ValueProjection::Variant(primary) => ValueField::Variant(primary),
                ValueProjection::Array(()) => {
                    let InstanceSchema::Array(array) =
                        instance.schema(self.program).map_err(|_| INVALID)?
                    else {
                        return Err(INVALID);
                    };
                    let IndexFact::Known(index) =
                        IndexFact::Unknown.on_normal_continuation(array.length())
                    else {
                        return Err(INVALID);
                    };
                    ValueField::Array(index)
                }
            };
            let position = match field {
                ValueField::Array(index) => usize::try_from(index).map_err(|_| INVALID)?,
                _ => self.component_position(field)?,
            };
            fields.push(plan::Projection::Field(position));
            instance = instance
                .component(self.program, field)
                .map_err(|_| INVALID)?;
        }
        Ok(fields)
    }
    fn resource_data(&mut self, step: &Step<InstantiatedPayload>) -> Result<()> {
        step.try_map_payloads_at::<super::Failure, ()>(|site, payload| {
            match site {
                PayloadSite::Source(place) if place.fields().is_root() => {
                    self.register_holder(place.holder(), payload.instance())?;
                }
                PayloadSite::Destination(holder) => {
                    self.register_holder(holder, payload.instance())?;
                }
                PayloadSite::Source(_) => {}
            }
            Ok(())
        })?;
        match step {
            Step::Move {
                destination,
                payload,
                ..
            }
            | Step::ReplaceReferent {
                destination,
                payload,
                ..
            } if destination.fields().is_root() => {
                self.register_holder(destination.holder(), payload.instance())?;
            }
            Step::MoveArrayElement {
                index,
                destination,
                payload,
                ..
            } => {
                self.register_holder(Holder::Expression(index.selection()), payload.instance())?;
                self.register_holder(*destination, payload.instance())?;
            }
            Step::Assemble {
                payload, fields, ..
            } => {
                for (field, source) in fields {
                    if source.fields().is_root() {
                        let key = payload
                            .instance()
                            .component(self.program, *field)
                            .map_err(|_| INVALID)?;
                        self.register_holder(source.holder(), &key)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
