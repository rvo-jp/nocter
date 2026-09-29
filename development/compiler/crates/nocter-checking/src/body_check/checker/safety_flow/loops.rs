use std::collections::BTreeSet;

use nocter_model::{BorrowCapability, TypeKind};

use super::{BodyChecker, SafetyFlowState, SafetyValue};
use crate::{LocalBindingKind, PlaceRoot};

impl SafetyFlowState {
    fn retain_loop_stable(&mut self, mut stable: impl FnMut(&SafetyValue) -> bool) {
        self.ranges.retain(|value, _| stable(value));
        self.order.retain_symbols(stable);
    }
}

impl BodyChecker<'_, '_> {
    /// Retains only facts whose value cannot change between loop iterations.
    ///
    /// A repeated condition and its body are checked from this state, not from facts that happen to
    /// hold before the first iteration. The condition may then add fresh facts that are valid for
    /// every entered iteration.
    pub(in crate::body_check::checker) fn loop_safety_invariant(
        &self,
        entry: &SafetyFlowState,
    ) -> SafetyFlowState {
        let mut invariant = entry.clone();
        invariant.retain_loop_stable(|value| self.loop_value_is_stable(value));
        invariant
    }

    fn loop_value_is_stable(&self, value: &SafetyValue) -> bool {
        let ty = match value.root {
            PlaceRoot::Parameter(parameter) => self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .and_then(|parameter| parameter.value_type(self.types)),
            PlaceRoot::Local(local)
                if self
                    .names
                    .locals()
                    .get(local)
                    .is_some_and(|local| local.kind() == LocalBindingKind::Immutable) =>
            {
                self.builder.local_type(local)
            }
            PlaceRoot::Local(_)
            | PlaceRoot::Capture(_)
            | PlaceRoot::Static(_)
            | PlaceRoot::Value(_) => return false,
        };
        if matches!(
            ty.and_then(|ty| self.types.get(ty)),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::ReadWrite,
                ..
            })
        ) {
            return false;
        }
        !self.type_may_carry_mutable_alias(ty, &mut BTreeSet::default())
    }
}
