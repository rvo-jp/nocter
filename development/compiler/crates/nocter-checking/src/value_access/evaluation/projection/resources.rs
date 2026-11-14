//! Allocate only slot metadata from common ordered data projection, never values or owners.
use super::{Builder, INVALID, Key, Result};
use crate::value_access::flow::{
    self,
    resources::{Holder, InstantiatedPayload, PayloadSite, Step},
};
use nocter_declarations::value_access::DataInstanceKey;

impl Builder<'_, '_> {
    pub(super) fn prepare_value_data(&mut self) -> Result<()> {
        use crate::value_access::ValueSourceSlot;
        let definitions = self
            .values
            .definitions()
            .map(|((_, slot), payload)| {
                let holder = match slot {
                    ValueSourceSlot::Local(local) => Holder::Local(local),
                    ValueSourceSlot::Expression(expression) => Holder::Expression(expression),
                    ValueSourceSlot::Argument { .. } | ValueSourceSlot::Return => {
                        return Err(INVALID);
                    }
                };
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
                            let target = projection.replacement_target(destination)?;
                            Ok((
                                destination.holder(),
                                self.values.root(at, target).ok_or(INVALID)?.clone(),
                                self.values.root_after(at, target).ok_or(INVALID)?.clone(),
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    let before = projection.steps(transition.before_steps())?;
                    let after = projection.steps(transition.after_steps())?;
                    drop(projection);
                    // Slots currently have one shape. Both actual virtual roots must agree with
                    // that shape; a later write cannot silently reuse its binding-time snapshot.
                    // This prepares no value and proves no enclosing-allocation geometry.
                    for (holder, before, after) in roots {
                        self.register_holder(holder, &before)?;
                        self.register_holder(holder, &after)?;
                    }
                    for step in before.iter().chain(&after) {
                        self.resource_data(step)?;
                    }
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
