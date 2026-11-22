//! Qualified local scoped receipts preserve retained endpoints and initialized siblings.
use super::{
    BTreeSet, Builder, Effect, Failure, Holder, Place, Projection, RuntimeDataId, SlotKey, Step,
    ValueSourceSlot, lifecycle,
};
use nocter_checking::value_access::flow::access::{IndexFact, Root};
use nocter_model::{ValueField, ValuePath, ValueProjection};

impl Builder<'_> {
    pub(super) fn scoped_replacement(
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
        if let Some(effects) = self.scoped_repack(replacement)? {
            return Ok(effects);
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
            disposal: self.disposal(retired.disposal())?,
        };
        let supplied = self.payload_data(payload)?;
        let transfer = Effect::Transfer {
            source: self.resource_place(source, supplied, replacement.data())?,
            destination: self.place_with_root(destination, supplied, Some(after))?,
        };
        Ok([retired, transfer])
    }

    fn scoped_repack(
        &mut self,
        replacement: &lifecycle::ScopedReplacement,
    ) -> Result<Option<[Effect; 2]>, Failure> {
        let Step::ReplaceReferent {
            destination,
            source,
            retired,
            payload,
        } = replacement.operation()
        else {
            return Err(self.invalid());
        };
        if !destination.fields().is_root() {
            return Ok(None);
        }
        let Some(carrier) = stable_local_carrier(replacement) else {
            return Ok(None);
        };
        let (local, old, new) = (carrier.local, carrier.before, carrier.after);
        let holder = Holder::Local(local);
        let Some(paths) = self.retained_offsets(local)? else {
            return Ok(None);
        };
        let program = self.instances.unit().program();
        let before = self
            .storage
            .instance(program, old.instance())
            .map_err(Failure::Storage)?;
        let after = self
            .storage
            .instance(program, new.instance())
            .map_err(Failure::Storage)?;
        if before == after {
            return Ok(None);
        }
        let virtual_before = self
            .storage
            .instance(program, replacement.root_before())
            .map_err(Failure::Storage)?;
        let virtual_after = self
            .storage
            .instance(program, replacement.root_after())
            .map_err(Failure::Storage)?;
        let Some((projections, old_leaf, new_leaf)) =
            self.scoped_path(before, after, &old.selection().path)
        else {
            return Ok(None);
        };
        if old_leaf != virtual_before || new_leaf != virtual_after {
            return Ok(None);
        }
        let Some(&slot) = self.keys.get(&SlotKey::Local(local)) else {
            return Ok(None);
        };
        let supplied = self.payload_data(payload)?;
        let target = Place {
            slot,
            root: after,
            data: supplied,
            projections,
        };
        let missing = carrier
            .missing
            .iter()
            .map(|hole| self.repack_exclusion(before, slot, hole))
            .collect::<Result<BTreeSet<_>, _>>()?
            .into_iter()
            .collect::<Box<[_]>>();
        let Some(preserved) =
            super::super::repack::preserved(self.runtime, before, &target, &missing)
        else {
            return Ok(None);
        };
        // One owning slot reserves all represented roots; both states keep that base address.
        if self.root(holder, before, false)?.slot != slot
            || self.root(holder, after, false)?.slot != slot
        {
            return Err(self.invalid());
        }
        let retired = Effect::Dispose {
            source: self.place_with_root(
                destination,
                self.payload_data(retired)?,
                Some(virtual_before),
            )?,
            disposal: self.disposal(retired.disposal())?,
        };
        let source = self.resource_place(source, supplied, replacement.data())?;
        self.constrain_geometry_requirement(
            before,
            after,
            super::super::GeometryRequirement::ProjectionOffsets(paths.into_iter().collect()),
        )?;
        Ok(Some([
            retired,
            Effect::Repack {
                source,
                destination: target,
                before,
                missing,
                preserved,
            },
        ]))
    }

    fn retained_offsets(
        &self,
        local: nocter_model::AccessLocalId,
    ) -> Result<Option<BTreeSet<ValuePath<Option<u64>>>>, Failure> {
        let at = self.at.ok_or_else(|| self.invalid())?;
        let Some(regions) = self.checked.access().retained_regions(at) else {
            return Ok(None);
        };
        let paths = regions
            .before()
            .chain(regions.after())
            .filter(|region| region.root == Root::Owned(local))
            .map(|region| {
                ValuePath::new(region.fields.fields().iter().map(|field| match *field {
                    ValueProjection::Record(field) => ValueProjection::Record(field),
                    ValueProjection::Tuple(index) => ValueProjection::Tuple(index),
                    ValueProjection::Variant(primary) => ValueProjection::Variant(primary),
                    ValueProjection::Array(index) => ValueProjection::Array(match index {
                        IndexFact::Known(index) => Some(index),
                        IndexFact::Unknown => None,
                    }),
                }))
            })
            .collect::<BTreeSet<_>>();
        if paths.is_empty() {
            return Ok(None);
        }
        Ok(Some(paths))
    }

    fn scoped_path(
        &self,
        mut before: RuntimeDataId,
        mut after: RuntimeDataId,
        path: &ValuePath<()>,
    ) -> Option<(Box<[Projection]>, RuntimeDataId, RuntimeDataId)> {
        let mut projections = Vec::new();
        for field in path.fields() {
            let field = match *field {
                ValueProjection::Record(field) => ValueField::Record(field),
                ValueProjection::Tuple(index) => ValueField::Tuple(index),
                ValueProjection::Variant(primary) => ValueField::Variant(primary),
                ValueProjection::Array(()) => {
                    if self.runtime.types().get(before)?.array()?.1 != 1
                        || self.runtime.types().get(after)?.array()?.1 != 1
                    {
                        return None;
                    }
                    ValueField::Array(0)
                }
            };
            before = self.runtime.value_field(before, field)?;
            after = self.runtime.value_field(after, field)?;
            projections.push(Projection::Field(field));
        }
        Some((projections.into(), before, after))
    }
}

struct LocalCarrier<'a> {
    local: nocter_model::AccessLocalId,
    missing: &'a [nocter_checking::value_access::flow::resources::Missing],
    before: &'a nocter_checking::value_access::ReferentCarrierData,
    after: &'a nocter_checking::value_access::ReferentCarrierData,
}
fn stable_local_carrier(replacement: &lifecycle::ScopedReplacement) -> Option<LocalCarrier<'_>> {
    let Step::ReplaceReferent { source, .. } = replacement.operation() else {
        return None;
    };
    let [old] = replacement.carriers().before() else {
        return None;
    };
    let [new] = replacement.carriers().after()? else {
        return None;
    };
    let ValueSourceSlot::Local(local) = old.selection().slot else {
        return None;
    };
    let holder = Holder::Local(local);
    if old.selection().slot != new.selection().slot
        || old.selection().path != new.selection().path
        || old.instance().data() != new.instance().data()
        || source.holder() == holder
    {
        return None;
    }
    Some(LocalCarrier {
        local,
        missing: replacement.carrier_missing(holder)?,
        before: old,
        after: new,
    })
}
