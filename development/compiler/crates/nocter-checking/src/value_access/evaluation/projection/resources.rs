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
    pub(super) before: Vec<Step<InstantiatedPayload>>,
    pub(super) after: Vec<Step<InstantiatedPayload>>,
    roots: Vec<Roots>,
}
struct Roots {
    holder: Holder,
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
                                holder: destination.holder(),
                                before: self.values.root(at, target).ok_or(INVALID)?.clone(),
                                after: self.values.root_after(at, target).ok_or(INVALID)?.clone(),
                                carriers,
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let before = projection.steps(transition.before_steps())?;
                    let after = projection.after_steps(transition.after_steps())?;
                    drop(projection);
                    // Reference views remain fixed. Owning-local metadata may observe multiple
                    // contexts, but source-boundary constraints must still match the actual plan.
                    // Registration supplies no initialized value or enclosing-allocation geometry.
                    for root in &roots {
                        self.register_holder(root.holder, &root.before)?;
                        self.register_holder(root.holder, &root.after)?;
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
                        self.resource_data(step)?;
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
            entry.push(self.root_context(root.holder, &root.before)?);
            completion.push(self.root_context(root.holder, &root.after)?);
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
        projected: &[Step<InstantiatedPayload>],
    ) -> Result<Vec<plan::Effect>> {
        if original.len() != projected.len() {
            return Err(INVALID);
        }
        let mut effects = Vec::new();
        for (original, projected) in original.iter().zip(projected) {
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
                    && target.fields.is_empty()
                    && self.holder_contract(destination.holder())?.1 == plan::Storage::Owned
                {
                    effect = plan::Effect::MoveRoot {
                        source: source.clone(),
                        destination: target.slot,
                        data,
                    };
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
