use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{BodyNodeId, BuiltinType, LocalBindingId, ParameterId, TypeId, TypeKind};
use nocter_toolchain_contract::StandardDeclarationRole;

use super::BodyChecker;
use crate::{
    ArithmeticTrapCheck, CallTarget, CheckedOperation, IndexBoundsCheck, LocalBindingKind,
    PlaceProjection, PlaceRoot, PrimitiveBinary, PrimitiveComparisonRelation, StaticDispatch,
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

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct LengthFact {
    index: SafetyValue,
    storage: SafetyValue,
    relation: LengthRelation,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LengthRelation {
    Below,
    AtOrAbove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SafetyLimit {
    Constant(i128),
    Length(SafetyValue),
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
    limit: SafetyLimit,
}

impl SafetyCondition {
    const fn new(
        value: SafetyValue,
        relation: PrimitiveComparisonRelation,
        limit: SafetyLimit,
    ) -> Self {
        Self {
            value,
            relation,
            limit,
        }
    }

    fn integer_refinement(self, outcome: bool) -> Option<RangeRefinement> {
        let SafetyLimit::Constant(constant) = self.limit else {
            return None;
        };
        let relation = if outcome {
            self.relation
        } else {
            complement(self.relation)
        };
        let bound_after = || constant.checked_add(1);
        match relation {
            PrimitiveComparisonRelation::Equal => Some(RangeRefinement::Exact(constant)),
            PrimitiveComparisonRelation::NotEqual => None,
            PrimitiveComparisonRelation::Less => Some(RangeRefinement::Upper(constant)),
            PrimitiveComparisonRelation::LessEqual => bound_after().map(RangeRefinement::Upper),
            PrimitiveComparisonRelation::Greater => bound_after().map(RangeRefinement::Lower),
            PrimitiveComparisonRelation::GreaterEqual => Some(RangeRefinement::Lower(constant)),
        }
    }

    fn length_fact(self, outcome: bool) -> Option<LengthFact> {
        let SafetyLimit::Length(storage) = self.limit else {
            return None;
        };
        let relation = if outcome {
            self.relation
        } else {
            complement(self.relation)
        };
        let relation = match relation {
            PrimitiveComparisonRelation::Less => LengthRelation::Below,
            PrimitiveComparisonRelation::Equal
            | PrimitiveComparisonRelation::Greater
            | PrimitiveComparisonRelation::GreaterEqual => LengthRelation::AtOrAbove,
            PrimitiveComparisonRelation::LessEqual | PrimitiveComparisonRelation::NotEqual => {
                return None;
            }
        };
        Some(LengthFact {
            index: self.value,
            storage,
            relation,
        })
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
    length_facts: BTreeSet<LengthFact>,
}

impl Default for SafetyFlowState {
    fn default() -> Self {
        Self {
            reachable: true,
            ranges: BTreeMap::new(),
            length_facts: BTreeSet::new(),
        }
    }
}

impl SafetyFlowState {
    pub(super) fn branch(&self, condition: Option<SafetyCondition>, outcome: bool) -> Self {
        let mut branch = self.clone();
        let Some(condition) = condition else {
            return branch;
        };
        if let Some(refinement) = condition.integer_refinement(outcome) {
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
        }
        if let Some(fact) = condition.length_fact(outcome) {
            let opposite = LengthFact {
                relation: match fact.relation {
                    LengthRelation::Below => LengthRelation::AtOrAbove,
                    LengthRelation::AtOrAbove => LengthRelation::Below,
                },
                ..fact
            };
            branch.reachable &= !branch.length_facts.contains(&opposite);
            branch.length_facts.insert(fact);
        }
        branch
    }

    pub(super) fn join(states: impl IntoIterator<Item = Self>) -> Self {
        let mut reachable = states.into_iter().filter(|state| state.reachable);
        let Some(mut joined) = reachable.next() else {
            return Self {
                reachable: false,
                ranges: BTreeMap::new(),
                length_facts: BTreeSet::new(),
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
            joined
                .length_facts
                .retain(|fact| incoming.length_facts.contains(fact));
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

    fn view_bounds_disposition(
        &self,
        index: SafetyValue,
        storage: SafetyValue,
    ) -> IndexBoundsCheck {
        if !self.reachable
            || self.length_facts.contains(&LengthFact {
                index,
                storage,
                relation: LengthRelation::Below,
            })
        {
            IndexBoundsCheck::ProvenInBounds
        } else if self.length_facts.contains(&LengthFact {
            index,
            storage,
            relation: LengthRelation::AtOrAbove,
        }) {
            IndexBoundsCheck::ProvenTrap
        } else {
            IndexBoundsCheck::Required
        }
    }
}

impl BodyChecker<'_, '_> {
    pub(super) fn negation_check(&self, operand: BodyNodeId, ty: TypeId) -> ArithmeticTrapCheck {
        let Some((minimum, maximum, _, true)) = integer_domain(self.types.get(ty)) else {
            return ArithmeticTrapCheck::NotRequired;
        };
        let range = self.arithmetic_range(operand, minimum, maximum);
        if range.lower_inclusive == Some(minimum) && range.upper_exclusive == minimum.checked_add(1)
        {
            ArithmeticTrapCheck::ProvenTrap
        } else if range.lower_inclusive.is_some_and(|lower| lower > minimum) {
            ArithmeticTrapCheck::ProvenSafe
        } else {
            ArithmeticTrapCheck::Required
        }
    }

    pub(super) fn arithmetic_check(
        &self,
        operation: PrimitiveBinary,
        left: BodyNodeId,
        right: BodyNodeId,
        ty: TypeId,
    ) -> ArithmeticTrapCheck {
        let Some((minimum, maximum, bits, signed)) = integer_domain(self.types.get(ty)) else {
            return ArithmeticTrapCheck::NotRequired;
        };
        let left = self.arithmetic_range(left, minimum, maximum);
        let right = self.arithmetic_range(right, minimum, maximum);
        match operation {
            PrimitiveBinary::Add | PrimitiveBinary::Subtract | PrimitiveBinary::Multiply => {
                arithmetic_result_check(operation, left, right, minimum, maximum)
            }
            PrimitiveBinary::Divide | PrimitiveBinary::Remainder => {
                division_check(left, right, minimum, signed)
            }
            PrimitiveBinary::ShiftLeft
            | PrimitiveBinary::ShiftRightSigned
            | PrimitiveBinary::ShiftRightUnsigned => shift_check(right, bits),
        }
    }

    fn arithmetic_range(&self, node: BodyNodeId, minimum: i128, maximum: i128) -> IntegerRange {
        if let Some(value) = self.integer_constant(node) {
            return IntegerRange {
                lower_inclusive: Some(value),
                upper_exclusive: value.checked_add(1),
            };
        }
        let Some(value) = self.safety_value(node) else {
            return IntegerRange {
                lower_inclusive: Some(minimum),
                upper_exclusive: maximum.checked_add(1),
            };
        };
        let flow = self
            .safety_flow
            .ranges
            .get(&value)
            .copied()
            .unwrap_or_default();
        IntegerRange {
            lower_inclusive: Some(flow.lower_inclusive.unwrap_or(minimum).max(minimum)),
            upper_exclusive: Some(flow.upper_exclusive.unwrap_or(maximum + 1).min(maximum + 1)),
        }
    }

    pub(super) fn safety_condition(&self, node: BodyNodeId) -> Option<SafetyCondition> {
        let CheckedOperation::Comparison(comparison) = self.builder.node(node)?.operation() else {
            return None;
        };
        let relation = comparison.primitive_relation()?;
        let left_value = self.safety_value(comparison.left().value());
        let right_value = self.safety_value(comparison.right().value());
        let left_limit = self.safety_limit(comparison.left().value());
        let right_limit = self.safety_limit(comparison.right().value());
        match (left_value, right_limit, left_limit, right_value) {
            (Some(value), Some(limit), _, None) => {
                Some(SafetyCondition::new(value, relation, limit))
            }
            (None, _, Some(limit), Some(value)) => {
                Some(SafetyCondition::new(value, reverse(relation), limit))
            }
            _ => None,
        }
    }

    pub(super) fn flow_fixed_index_bounds(
        &self,
        index: BodyNodeId,
        length: u64,
    ) -> Option<IndexBoundsCheck> {
        self.safety_value(index)
            .map(|value| self.safety_flow.bounds_disposition(value, length))
    }

    pub(super) fn flow_view_index_bounds(
        &self,
        index: BodyNodeId,
        root: PlaceRoot,
        projections: &[PlaceProjection],
    ) -> Option<IndexBoundsCheck> {
        let index = self.safety_value(index)?;
        let storage = self.safety_place(root, projections)?;
        Some(self.safety_flow.view_bounds_disposition(index, storage))
    }

    fn safety_value(&self, node: BodyNodeId) -> Option<SafetyValue> {
        let place = match self.builder.node(node)?.operation() {
            CheckedOperation::Place(place) | CheckedOperation::Copy(place) => *place,
            _ => return None,
        };
        let place = self.builder.place(place)?;
        self.safety_place(place.root(), place.projections())
    }

    fn safety_place(
        &self,
        root: PlaceRoot,
        projections: &[PlaceProjection],
    ) -> Option<SafetyValue> {
        if projections
            .iter()
            .any(|projection| !matches!(projection, PlaceProjection::BorrowDeref { .. }))
        {
            return None;
        }
        match root {
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

    fn safety_limit(&self, node: BodyNodeId) -> Option<SafetyLimit> {
        self.integer_constant(node)
            .map(SafetyLimit::Constant)
            .or_else(|| self.view_length(node).map(SafetyLimit::Length))
    }

    fn view_length(&self, node: BodyNodeId) -> Option<SafetyValue> {
        let CheckedOperation::Call(call) = self.builder.node(node)?.operation() else {
            return None;
        };
        let CallTarget::Static(selection) = call.target() else {
            return None;
        };
        let StaticDispatch::Direct(callable) = selection.dispatch() else {
            return None;
        };
        let slice = self
            .standard_semantics
            .callable(StandardDeclarationRole::SliceLengthMethod);
        let string = self
            .standard_semantics
            .callable(StandardDeclarationRole::StringViewLengthMethod);
        if Some(callable) != slice && Some(callable) != string {
            return None;
        }
        let receiver = call.receiver()?;
        if receiver.coercion().is_some() {
            return None;
        }
        self.safety_value(receiver.value())
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

fn integer_domain(kind: Option<&TypeKind>) -> Option<(i128, i128, u8, bool)> {
    let builtin = match kind? {
        TypeKind::Builtin(builtin) => *builtin,
        _ => return None,
    };
    let (bits, signed) = match builtin {
        BuiltinType::I8 => (8, true),
        BuiltinType::I16 => (16, true),
        BuiltinType::I32 => (32, true),
        BuiltinType::I64 | BuiltinType::Isize => (64, true),
        BuiltinType::U8 => (8, false),
        BuiltinType::U16 => (16, false),
        BuiltinType::U32 => (32, false),
        BuiltinType::U64 | BuiltinType::Usize => (64, false),
        _ => return None,
    };
    let (minimum, maximum) = if signed {
        let magnitude = 1_i128 << (bits - 1);
        (-magnitude, magnitude - 1)
    } else {
        (0, (1_i128 << bits) - 1)
    };
    Some((minimum, maximum, bits, signed))
}

fn arithmetic_result_check(
    operation: PrimitiveBinary,
    left: IntegerRange,
    right: IntegerRange,
    minimum: i128,
    maximum: i128,
) -> ArithmeticTrapCheck {
    let (Some(left_min), Some(left_end), Some(right_min), Some(right_end)) = (
        left.lower_inclusive,
        left.upper_exclusive,
        right.lower_inclusive,
        right.upper_exclusive,
    ) else {
        return ArithmeticTrapCheck::Required;
    };
    let left_max = left_end - 1;
    let right_max = right_end - 1;
    let extrema = match operation {
        PrimitiveBinary::Add => [(left_min, right_min), (left_max, right_max)]
            .map(|(left, right)| left.checked_add(right)),
        PrimitiveBinary::Subtract => [(left_min, right_max), (left_max, right_min)]
            .map(|(left, right)| left.checked_sub(right)),
        PrimitiveBinary::Multiply => {
            let products = [
                left_min.checked_mul(right_min),
                left_min.checked_mul(right_max),
                left_max.checked_mul(right_min),
                left_max.checked_mul(right_max),
            ];
            let Some(mut low) = products[0] else {
                return ArithmeticTrapCheck::Required;
            };
            let mut high = low;
            for product in products.into_iter().skip(1) {
                let Some(product) = product else {
                    return ArithmeticTrapCheck::Required;
                };
                low = low.min(product);
                high = high.max(product);
            }
            return if low >= minimum && high <= maximum {
                ArithmeticTrapCheck::ProvenSafe
            } else if high < minimum || low > maximum {
                ArithmeticTrapCheck::ProvenTrap
            } else {
                ArithmeticTrapCheck::Required
            };
        }
        _ => return ArithmeticTrapCheck::Required,
    };
    let [Some(low), Some(high)] = extrema else {
        return ArithmeticTrapCheck::Required;
    };
    if low >= minimum && high <= maximum {
        ArithmeticTrapCheck::ProvenSafe
    } else if high < minimum || low > maximum {
        ArithmeticTrapCheck::ProvenTrap
    } else {
        ArithmeticTrapCheck::Required
    }
}

fn division_check(
    left: IntegerRange,
    right: IntegerRange,
    minimum: i128,
    signed: bool,
) -> ArithmeticTrapCheck {
    let excludes_zero = excludes_value(right, 0);
    let excludes_minus_one = !signed || excludes_value(right, -1);
    let left_excludes_minimum = !signed
        || left.lower_inclusive.is_some_and(|lower| lower > minimum)
        || left.upper_exclusive.is_some_and(|upper| upper <= minimum);
    if excludes_zero && (excludes_minus_one || left_excludes_minimum) {
        ArithmeticTrapCheck::ProvenSafe
    } else if right.lower_inclusive == Some(0) && right.upper_exclusive == Some(1)
        || signed
            && left.lower_inclusive == Some(minimum)
            && left.upper_exclusive == minimum.checked_add(1)
            && right.lower_inclusive == Some(-1)
            && right.upper_exclusive == Some(0)
    {
        ArithmeticTrapCheck::ProvenTrap
    } else {
        ArithmeticTrapCheck::Required
    }
}

fn excludes_value(range: IntegerRange, value: i128) -> bool {
    range.upper_exclusive.is_some_and(|upper| upper <= value)
        || range.lower_inclusive.is_some_and(|lower| lower > value)
}

fn shift_check(count: IntegerRange, bits: u8) -> ArithmeticTrapCheck {
    let bits = i128::from(bits);
    if count.lower_inclusive.is_some_and(|lower| lower >= 0)
        && count.upper_exclusive.is_some_and(|upper| upper <= bits)
    {
        ArithmeticTrapCheck::ProvenSafe
    } else if count.upper_exclusive.is_some_and(|upper| upper <= 0)
        || count.lower_inclusive.is_some_and(|lower| lower >= bits)
    {
        ArithmeticTrapCheck::ProvenTrap
    } else {
        ArithmeticTrapCheck::Required
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
        let condition = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::Less,
            super::SafetyLimit::Constant(4),
        );
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
        let strict = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::Less,
            super::SafetyLimit::Constant(2),
        );
        let broad = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::LessEqual,
            super::SafetyLimit::Constant(2),
        );
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
        let lower = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::GreaterEqual,
            super::SafetyLimit::Constant(4),
        );
        let upper = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::Less,
            super::SafetyLimit::Constant(4),
        );
        let safe = SafetyFlowState::default().branch(Some(upper), true);
        let unreachable = safe.branch(Some(lower), true);
        let joined = SafetyFlowState::join([safe, unreachable]);

        assert_eq!(
            joined.bounds_disposition(value(), 4),
            IndexBoundsCheck::ProvenInBounds
        );
    }

    #[test]
    fn view_length_facts_refine_and_join_by_semantic_identity() {
        let storage = SafetyValue::Parameter(
            nocter_model::ArenaBuilder::<nocter_model::ParameterId, ()>::new().insert(()),
        );
        let condition = SafetyCondition::new(
            value(),
            PrimitiveComparisonRelation::Less,
            super::SafetyLimit::Length(storage),
        );
        let entry = SafetyFlowState::default();
        let safe = entry.branch(Some(condition), true);
        let trapped = entry.branch(Some(condition), false);

        assert_eq!(
            safe.view_bounds_disposition(value(), storage),
            IndexBoundsCheck::ProvenInBounds
        );
        assert_eq!(
            trapped.view_bounds_disposition(value(), storage),
            IndexBoundsCheck::ProvenTrap
        );
        assert_eq!(
            SafetyFlowState::join([safe, trapped]).view_bounds_disposition(value(), storage),
            IndexBoundsCheck::Required
        );
    }
}
