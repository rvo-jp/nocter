//! Numeric point equality is conditional evidence; actual unavailable slots and backing own its use.
use super::{NumericFacts, TypedQuery};
use crate::value_access::{
    Referent,
    flow::{
        Location,
        resources::{Place, RetainedIndex},
    },
};
use nocter_model::AccessExpressionId;
use std::collections::BTreeSet;

pub(in crate::value_access::flow) struct Restoration<'a> {
    pub source: RetainedIndex,
    pub retirements: &'a BTreeSet<Location>,
}
impl NumericFacts {
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
        Some(Restoration {
            source: RetainedIndex::for_selection(typed, source)?,
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
        Some(point.source.array(typed)?.project_retained(point.source))
    }
}
