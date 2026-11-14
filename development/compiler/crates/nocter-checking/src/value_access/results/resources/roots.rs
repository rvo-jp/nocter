//! Containing representation receipts selected by the original ordered data projection.
use super::{
    DataInstanceKey, Failure, INVALID, Payload, PayloadSite, Place, ResourceDataProjection,
    ResourceStep, source_slot,
};
use std::collections::{BTreeMap, BTreeSet};

/// One original resource operation and the dependent containing data selected before it.
/// These representations provide neither ownership nor initialized storage or stable geometry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDataStep {
    pub(super) operation: ResourceStep<Payload>,
    pub(super) roots: BTreeMap<Place, DataInstanceKey>,
    pub(super) destination: Option<Box<PartialRootContracts>>,
}
/// Original whole-container contracts around one matched partial owning destination.
/// These select metadata; the resource operation still proves receipt and ownership separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartialRootContracts {
    destination: Place,
    before: DataInstanceKey,
    after: DataInstanceKey,
}
impl PartialRootContracts {
    #[must_use]
    pub const fn destination(&self) -> &Place {
        &self.destination
    }
    #[must_use]
    pub const fn before(&self) -> &DataInstanceKey {
        &self.before
    }
    #[must_use]
    pub const fn after(&self) -> &DataInstanceKey {
        &self.after
    }
}
impl ResourceDataStep {
    #[must_use]
    pub const fn operation(&self) -> &ResourceStep<Payload> {
        &self.operation
    }
    #[must_use]
    pub fn root(&self, place: &Place) -> Option<&DataInstanceKey> {
        self.roots.get(place)
    }
    pub fn roots(&self) -> impl Iterator<Item = &DataInstanceKey> {
        self.roots.values().chain(
            self.destination
                .iter()
                .flat_map(|roots| [&roots.before, &roots.after]),
        )
    }
    #[must_use]
    pub fn destination_roots(&self) -> Option<&PartialRootContracts> {
        self.destination.as_deref()
    }
}

impl ResourceDataProjection<'_, '_> {
    pub(super) fn partial_destination(
        &mut self,
        step: &ResourceStep,
    ) -> Result<Option<PartialRootContracts>, Failure> {
        let ResourceStep::Move { destination, .. } = step else {
            return Ok(None);
        };
        if destination.fields().is_root() {
            return Ok(None);
        }
        let data = source_slot(destination.holder())?
            .data(self.body.typed())
            .map_err(|_| INVALID)?
            .ok_or(INVALID)?;
        if self.closed(data).is_some() {
            return Ok(None);
        }
        let target = self.write_target()?;
        let crate::value_access::Referent::Place(id) = target else {
            return Err(Failure::Instantiation(
                super::InstantiationFailure::Unresolved,
            ));
        };
        let place = self.body.typed().body().places().get(id).ok_or(INVALID)?;
        let local = self
            .body
            .typed()
            .body()
            .locals()
            .get(place.local)
            .ok_or(INVALID)?;
        if local.access().mode() != nocter_model::ValueAccessMode::Own
            || super::Holder::Local(place.local)
                .place()
                .project_path(&place.fields)
                != *destination
        {
            return Err(INVALID);
        }
        let before = super::super::values::root(
            self.program,
            self.evidence,
            self.body,
            self.frame,
            self.at,
            target,
            self.work,
        )
        .map_err(Failure::Instantiation)?;
        let after = super::super::values::root_after(
            self.program,
            self.evidence,
            self.body,
            self.frame,
            self.at,
            target,
            self.work,
        )
        .map_err(Failure::Instantiation)?;
        Ok(Some(PartialRootContracts {
            destination: destination.clone(),
            before,
            after,
        }))
    }
    pub(super) fn containing_roots(
        &mut self,
        step: &ResourceStep,
    ) -> Result<BTreeMap<Place, DataInstanceKey>, Failure> {
        let mut places = BTreeSet::new();
        step.try_map_payloads_at::<Failure, ()>(|site, _| {
            if let PayloadSite::Source(place) = site
                && !place.fields().is_root()
                && !matches!(step, ResourceStep::ReplaceReferent { destination, .. } if destination == place)
            {
                places.insert(place.clone());
            }
            Ok(())
        })?;
        if let ResourceStep::Assemble { fields, .. } = step {
            places.extend(
                fields
                    .iter()
                    .map(|(_, source)| source)
                    .filter(|source| !source.fields().is_root())
                    .cloned(),
            );
        }
        let mut roots = BTreeMap::new();
        for place in places {
            let data = source_slot(place.holder())?
                .data(self.body.typed())
                .map_err(|_| INVALID)?
                .ok_or(INVALID)?;
            if self.closed(data).is_none() {
                let root = self.source(&place.holder().place(), data)?;
                roots.insert(place, root);
            }
        }
        Ok(roots)
    }
}
