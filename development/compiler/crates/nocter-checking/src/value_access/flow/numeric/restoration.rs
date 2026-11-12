//! Numeric point equality is conditional evidence; actual unavailable slots and backing own its use.
use super::{NumericFacts, TypedQuery};
use crate::value_access::{
    Referent,
    flow::{
        Location,
        resources::{Place, RetainedIndex},
    },
};
use nocter_model::{AccessExpressionId, AccessLocalId};
use std::collections::BTreeSet;

pub(in crate::value_access::flow) struct Restoration<'a> {
    pub source: AccessExpressionId,
    pub retirements: &'a BTreeSet<Location>,
}
pub(in crate::value_access::flow) struct ConditionalRestoration {
    pub array: Place,
    pub local: AccessLocalId,
    pub retirements: BTreeSet<Location>,
}
impl NumericFacts {
    pub(in crate::value_access::flow) fn conditional_restoration(
        &self,
        typed: &TypedQuery,
        target: Referent,
    ) -> Option<ConditionalRestoration> {
        let Referent::Evaluated(selection) = target else {
            return None;
        };
        let (current, sources) = self.conditional_restoration.as_ref()?;
        if *current != selection {
            return None;
        }
        let first = RetainedIndex::for_selection(typed, *sources.first()?)?;
        let array = first.array(typed)?;
        let crate::value_access::flow::resources::Holder::Local(local) = array.holder() else {
            return None;
        };
        let mut retirements = BTreeSet::new();
        for &source in sources {
            let point = RetainedIndex::for_selection(typed, source)?;
            if point.array(typed)? != array {
                return None;
            }
            retirements.extend(self.cells.histories.retirement_sites(source)?);
        }
        Some(ConditionalRestoration {
            array,
            local,
            retirements,
        })
    }
    pub(in crate::value_access::flow) fn restoration(
        &self,
        typed: &TypedQuery,
        selection: AccessExpressionId,
    ) -> Option<Restoration<'_>> {
        let (source, retirements) = self.cells.histories.matching_point(
            &self.cells,
            &self.equalities,
            &self.point_sources,
            selection,
        )?;
        let (_, source_suffix) = RetainedIndex::for_referent(typed, Referent::Evaluated(source))?;
        let (_, target_suffix) =
            RetainedIndex::for_referent(typed, Referent::Evaluated(selection))?;
        if source_suffix != target_suffix {
            return None;
        }
        Some(Restoration {
            source,
            retirements,
        })
    }
    pub(in crate::value_access::flow) fn restoration_path(
        &self,
        typed: &TypedQuery,
        target: Referent,
    ) -> Option<Place> {
        let Referent::Evaluated(selection) = target else {
            return None;
        };
        let point = self.restoration(typed, selection)?;
        let (index, suffix) =
            RetainedIndex::for_referent(typed, Referent::Evaluated(point.source))?;
        let destination = index.array(typed)?.project_retained(index);
        let fields = destination.fields().concat_fields(&suffix);
        Some(destination.with_fields(fields))
    }
}
