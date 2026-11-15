//! One late constant-index refinement for availability, frozen grants and resource paths.
use super::{NumericFacts, TypedQuery};
use crate::value_access::{AccessPath, IndexFact, Referent, flow::resources};
use nocter_model::{AccessExpressionId, AccessLocalId};

pub(in crate::value_access::flow) fn index_fact(
    typed: &TypedQuery,
    facts: Option<&NumericFacts>,
    selection: AccessExpressionId,
    operand: AccessExpressionId,
) -> Option<IndexFact> {
    let original = typed.body().expressions().get(operand)?.index_fact();
    if original != IndexFact::Unknown {
        return Some(original);
    }
    // These facts choose a valid concrete projection, not a replacement bounds classifier.
    let known = facts.and_then(|facts| facts.constant(operand));
    let selected = resources::RetainedIndex::for_selection(typed, selection);
    let fact = known
        .filter(|&value| selected.is_some_and(|index| value < index.bound()))
        .map_or(IndexFact::Unknown, IndexFact::Known);
    Some(selected.map_or(fact, |index| fact.on_normal_continuation(index.bound())))
}

pub(in crate::value_access::flow) fn path(
    typed: &TypedQuery,
    facts: Option<&NumericFacts>,
    reference: Referent,
) -> Option<(AccessLocalId, AccessPath)> {
    reference.projection_path(
        |id| typed.body().places().get(id),
        |id| typed.body().expressions().get(id),
        |id| typed.body().blocks().get(id),
        IndexFact::Known,
        |selection, operand| index_fact(typed, facts, selection, operand),
    )
}

pub(in crate::value_access::flow) fn resource_path(
    typed: &TypedQuery,
    facts: Option<&NumericFacts>,
    reference: Referent,
) -> Option<(AccessLocalId, resources::Path)> {
    resources::Path::for_referent_with(typed, reference, |selection, operand| {
        index_fact(typed, facts, selection, operand)
    })
}

pub(in crate::value_access::flow) fn initialization_paths(
    typed: &TypedQuery,
    facts: Option<&NumericFacts>,
    reference: Referent,
) -> Option<Vec<(AccessLocalId, AccessPath)>> {
    reference.projection_paths(
        |id| typed.body().places().get(id),
        |id| typed.body().expressions().get(id),
        |id| typed.body().blocks().get(id),
        crate::value_access::paths::ProjectionRoots::Unforwarded,
        IndexFact::Known,
        |selection, operand| index_fact(typed, facts, selection, operand),
    )
}
