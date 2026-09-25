use std::collections::BTreeMap;

use nocter_model::{BodyNodeId, LocalBindingId, ParameterId};

use super::BodyChecker;
use crate::{
    CheckedOperation, IndexBoundsCheck, LocalBindingKind, PlaceRoot, PrimitiveComparisonRelation,
};

/// One stable scalar storage identity admitted into local safety reasoning.
///
/// Mutable locals, captures, statics, and projected places are deliberately excluded. Their value
/// can change through assignment or aliasing, while an ordinary value parameter or immutable local
/// cannot change during its checked lifetime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum SafetyValue {
    Parameter(ParameterId),
    ImmutableLocal(LocalBindingId),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct IntegerRange {
    lower_inclusive: Option<i128>,
    upper_exclusive: Option<i128>,
}

impl IntegerRange {
    fn refine_lower(&mut self, lower: i128) {
        self.lower_inclusive = Some(
            self.lower_inclusive
                .map_or(lower, |current| current.max(lower)),
        );
    }

    fn refine_upper(&mut self, upper: i128) {
        self.upper_exclusive = Some(
            self.upper_exclusive
                .map_or(upper, |current| current.min(upper)),
        );
    }

    fn is_empty(self) -> bool {
        matches!(
            (self.lower_inclusive, self.upper_exclusive),
            (Some(lower), Some(upper)) if lower >= upper
        )
    }

    fn join(self, incoming: Self) -> Self {
        Self {
            lower_inclusive: match (self.lower_inclusive, incoming.lower_inclusive) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (None, _) | (_, None) => None,
            },
            upper_exclusive: match (self.upper_exclusive, incoming.upper_exclusive) {
                (Some(left), Some(right)) => Some(left.max(right)),
                (None, _) | (_, None) => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SafetyCondition {
    value: SafetyValue,
    relation: PrimitiveComparisonRelation,
    constant: i128,
}

impl SafetyCondition {
    const fn new(
        value: SafetyValue,
        relation: PrimitiveComparisonRelation,
        constant: i128,
    ) -> Self {
        Self {
            value,
            relation,
            constant,
        }
    }

    fn refinement(self, outcome: bool) -> Option<RangeRefinement> {
        let relation = if outcome {
            self.relation
        } else {
            complement(self.relation)
        };
        let bound_after = || self.constant.checked_add(1);
        match relation {
            PrimitiveComparisonRelation::Equal => Some(RangeRefinement::Exact(self.constant)),
            PrimitiveComparisonRelation::NotEqual => None,
            PrimitiveComparisonRelation::Less => Some(RangeRefinement::Upper(self.constant)),
            PrimitiveComparisonRelation::LessEqual => bound_after().map(RangeRefinement::Upper),
            PrimitiveComparisonRelation::Greater => bound_after().map(RangeRefinement::Lower),
            PrimitiveComparisonRelation::GreaterEqual => {
                Some(RangeRefinement::Lower(self.constant))
            }
        }
    }
}

#[derive(Clone, Copy)]
enum RangeRefinement {
    Lower(i128),
    Upper(i128),
    Exact(i128),
}

/// Checking-owned path facts for one point in a body.
///
/// A state contains only facts valid on every execution reaching that point. Branch refinement
/// intersects facts; a join forms the weakest interval containing every reachable incoming range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SafetyFlowState {
    reachable: bool,
    ranges: BTreeMap<SafetyValue, IntegerRange>,
}

impl Default for SafetyFlowState {
    fn default() -> Self {
        Self {
            reachable: true,
            ranges: BTreeMap::new(),
        }
    }
}

impl SafetyFlowState {
    pub(super) fn branch(&self, condition: Option<SafetyCondition>, outcome: bool) -> Self {
        let mut branch = self.clone();
        let Some(condition) = condition else {
            return branch;
        };
        let Some(refinement) = condition.refinement(outcome) else {
            return branch;
        };
        let range = branch.ranges.entry(condition.value).or_default();
        match refinement {
            RangeRefinement::Lower(lower) => range.refine_lower(lower),
            RangeRefinement::Upper(upper) => range.refine_upper(upper),
            RangeRefinement::Exact(value) => {
                range.refine_lower(value);
                if let Some(upper) = value.checked_add(1) {
                    range.refine_upper(upper);
                }
            }
        }
        branch.reachable &= !range.is_empty();
        branch
    }

    pub(super) fn join(states: impl IntoIterator<Item = Self>) -> Self {
        let mut reachable = states.into_iter().filter(|state| state.reachable);
        let Some(mut joined) = reachable.next() else {
            return Self {
                reachable: false,
                ranges: BTreeMap::new(),
            };
        };
        for incoming in reachable {
            joined.ranges.retain(|value, range| {
                let Some(incoming) = incoming.ranges.get(value) else {
                    return false;
                };
                *range = range.join(*incoming);
                true
            });
        }
        joined
    }

    fn bounds_disposition(&self, value: SafetyValue, length: u64) -> IndexBoundsCheck {
        if !self.reachable {
            return IndexBoundsCheck::ProvenInBounds;
        }
        let length = i128::from(length);
        let range = self.ranges.get(&value).copied().unwrap_or_default();
        if range.upper_exclusive.is_some_and(|upper| upper <= length) {
            IndexBoundsCheck::ProvenInBounds
        } else if length == 0 || range.lower_inclusive.is_some_and(|lower| lower >= length) {
            IndexBoundsCheck::ProvenTrap
        } else {
            IndexBoundsCheck::Required
        }
    }
}

impl BodyChecker<'_, '_> {
    pub(super) fn safety_condition(&self, node: BodyNodeId) -> Option<SafetyCondition> {
        let CheckedOperation::Comparison(comparison) = self.builder.node(node)?.operation() else {
            return None;
        };
        let relation = comparison.primitive_relation()?;
        let left_value = self.safety_value(comparison.left().value());
        let right_value = self.safety_value(comparison.right().value());
        let left_constant = self.integer_constant(comparison.left().value());
        let right_constant = self.integer_constant(comparison.right().value());
        match (left_value, right_constant, left_constant, right_value) {
            (Some(value), Some(constant), _, None) => {
                Some(SafetyCondition::new(value, relation, constant))
            }
            (None, _, Some(constant), Some(value)) => {
                Some(SafetyCondition::new(value, reverse(relation), constant))
            }
            _ => None,
        }
    }

    pub(super) fn flow_index_bounds(
        &self,
        index: BodyNodeId,
        length: u64,
    ) -> Option<IndexBoundsCheck> {
        self.safety_value(index)
            .map(|value| self.safety_flow.bounds_disposition(value, length))
    }

    fn safety_value(&self, node: BodyNodeId) -> Option<SafetyValue> {
        let place = match self.builder.node(node)?.operation() {
            CheckedOperation::Place(place) | CheckedOperation::Copy(place) => *place,
            _ => return None,
        };
        let place = self.builder.place(place)?;
        if !place.projections().is_empty() {
            return None;
        }
        match place.root() {
            PlaceRoot::Parameter(parameter) => Some(SafetyValue::Parameter(parameter)),
            PlaceRoot::Local(local)
                if self.names.locals().get(local)?.kind() == LocalBindingKind::Immutable =>
            {
                Some(SafetyValue::ImmutableLocal(local))
            }
            PlaceRoot::Local(_)
            | PlaceRoot::Capture(_)
            | PlaceRoot::Static(_)
            | PlaceRoot::Value(_) => None,
        }
    }

    fn integer_constant(&self, node: BodyNodeId) -> Option<i128> {
        match self.builder.node(node)?.operation() {
            CheckedOperation::Literal(crate::ConstantValue::Integer(value)) => Some(*value),
            CheckedOperation::DeclaredConstant(constant) => {
                match self.constants.constant(*constant) {
                    Some(crate::ConstantValue::Integer(value)) => Some(*value),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

const fn complement(relation: PrimitiveComparisonRelation) -> PrimitiveComparisonRelation {
    match relation {
        PrimitiveComparisonRelation::Equal => PrimitiveComparisonRelation::NotEqual,
        PrimitiveComparisonRelation::NotEqual => PrimitiveComparisonRelation::Equal,
        PrimitiveComparisonRelation::Less => PrimitiveComparisonRelation::GreaterEqual,
        PrimitiveComparisonRelation::LessEqual => PrimitiveComparisonRelation::Greater,
        PrimitiveComparisonRelation::Greater => PrimitiveComparisonRelation::LessEqual,
        PrimitiveComparisonRelation::GreaterEqual => PrimitiveComparisonRelation::Less,
    }
}

const fn reverse(relation: PrimitiveComparisonRelation) -> PrimitiveComparisonRelation {
    match relation {
        PrimitiveComparisonRelation::Equal => PrimitiveComparisonRelation::Equal,
        PrimitiveComparisonRelation::NotEqual => PrimitiveComparisonRelation::NotEqual,
        PrimitiveComparisonRelation::Less => PrimitiveComparisonRelation::Greater,
        PrimitiveComparisonRelation::LessEqual => PrimitiveComparisonRelation::GreaterEqual,
        PrimitiveComparisonRelation::Greater => PrimitiveComparisonRelation::Less,
        PrimitiveComparisonRelation::GreaterEqual => PrimitiveComparisonRelation::LessEqual,
    }
}

#[cfg(test)]
mod tests {
    use super::{SafetyCondition, SafetyFlowState, SafetyValue};
    use crate::{IndexBoundsCheck, PrimitiveComparisonRelation};
    use nocter_model::{ArenaBuilder, LocalBindingId};

    fn value() -> SafetyValue {
        SafetyValue::ImmutableLocal(ArenaBuilder::<LocalBindingId, ()>::new().insert(()))
    }

    #[test]
    fn branch_refinement_proves_both_sides_of_one_bound() {
        let condition = SafetyCondition::new(value(), PrimitiveComparisonRelation::Less, 4);
        let entry = SafetyFlowState::default();

        assert_eq!(
            entry
                .branch(Some(condition), true)
                .bounds_disposition(value(), 4),
            IndexBoundsCheck::ProvenInBounds
        );
        assert_eq!(
            entry
                .branch(Some(condition), false)
                .bounds_disposition(value(), 4),
            IndexBoundsCheck::ProvenTrap
        );
    }

    #[test]
    fn join_retains_only_a_bound_valid_on_every_reachable_path() {
        let strict = SafetyCondition::new(value(), PrimitiveComparisonRelation::Less, 2);
        let broad = SafetyCondition::new(value(), PrimitiveComparisonRelation::LessEqual, 2);
        let entry = SafetyFlowState::default();
        let joined = SafetyFlowState::join([
            entry.branch(Some(strict), true),
            entry.branch(Some(broad), true),
        ]);

        assert_eq!(
            joined.bounds_disposition(value(), 3),
            IndexBoundsCheck::ProvenInBounds
        );
        assert_eq!(
            joined.bounds_disposition(value(), 2),
            IndexBoundsCheck::Required
        );
    }

    #[test]
    fn unreachable_incoming_state_cannot_weaken_a_join() {
        let lower = SafetyCondition::new(value(), PrimitiveComparisonRelation::GreaterEqual, 4);
        let upper = SafetyCondition::new(value(), PrimitiveComparisonRelation::Less, 4);
        let safe = SafetyFlowState::default().branch(Some(upper), true);
        let unreachable = safe.branch(Some(lower), true);
        let joined = SafetyFlowState::join([safe, unreachable]);

        assert_eq!(
            joined.bounds_disposition(value(), 4),
            IndexBoundsCheck::ProvenInBounds
        );
    }
}
