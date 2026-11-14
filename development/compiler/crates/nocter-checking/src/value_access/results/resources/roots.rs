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
        self.roots.values()
    }
}

impl ResourceDataProjection<'_, '_> {
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
