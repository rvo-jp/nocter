use std::collections::BTreeSet;

use nocter_model::{BodyNodeId, TypeId, TypeKind};

use super::Analyzer;
use crate::loans::liveness::{LivePlace, LiveSlot};
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    BodyCheckError, BodyCheckInternalError, BodyRule, CleanupAction, CleanupTarget, LoanId,
    LoanPlace, LoanProjection, LoanRoot,
};

impl Analyzer<'_, '_> {
    /// Checks destruction in its scheduled order. A type-owned drop body observes every loan
    /// stored in its value until that action runs; structural destruction without such a body does
    /// not invent a read of non-owning fields.
    pub(super) fn check_cleanup_conflicts(
        &self,
        node: BodyNodeId,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        for schedule in self
            .input
            .body
            .cleanups()
            .schedules(node)
            .unwrap_or_default()
        {
            let actions = schedule.actions();
            let observer_loans = self.cleanup_observer_loans(actions, state)?;
            let mut ordinary = self.active_loans(node, state, extra);
            ordinary.retain(|loan| !observer_loans.contains(loan));
            for (position, action) in actions.iter().enumerate() {
                let mut active = ordinary.clone();
                active.extend(self.cleanup_observer_loans(&actions[position..], state)?);
                for target in self.cleanup_target_places(action, state)? {
                    for loan in &active {
                        let definition = self
                            .loans
                            .get(loan)
                            .ok_or(BodyCheckInternalError::LoanAnalysis)?;
                        if definition
                            .places()
                            .iter()
                            .any(|borrowed| target.overlaps(borrowed))
                        {
                            return Err(self.loan_error(
                                BodyRule::BorrowedPlaceMutation,
                                node,
                                *loan,
                            )?);
                        }
                    }
                }
                if let CleanupTarget::Region { parent, .. } = action.target() {
                    for ended in state.value(&LiveSlot::Node(*parent)).all_loans() {
                        ordinary.remove(&ended);
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn check_scope_exit_conflicts(
        &self,
        node: BodyNodeId,
        scopes: impl IntoIterator<Item = nocter_model::BodyScopeId>,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        let mut locals = Vec::new();
        for scope in scopes {
            let mut scope_locals = self
                .input
                .body
                .locals()
                .iter()
                .filter(|(_, local)| local.declaration().scope() == scope)
                .map(|(local, definition)| (local, definition.ty()))
                .collect::<Vec<_>>();
            scope_locals.reverse();
            locals.extend(scope_locals);
        }
        let all_observers = self.local_observer_loans(&locals, state);
        let mut ordinary = self.active_loans(node, state, extra);
        ordinary.retain(|loan| !all_observers.contains(loan));
        for (position, (local, _)) in locals.iter().enumerate() {
            let mut active = ordinary.clone();
            active.extend(self.local_observer_loans(&locals[position..], state));
            let target = LoanPlace::new(LoanRoot::Place(crate::PlaceRoot::Local(*local)), []);
            self.check_cleanup_target(node, &target, &active)?;
        }
        Ok(())
    }

    fn local_observer_loans(
        &self,
        locals: &[(nocter_model::LocalBindingId, TypeId)],
        state: &LoanState,
    ) -> BTreeSet<LoanId> {
        let mut loans = BTreeSet::new();
        for (local, ty) in locals {
            if self.has_observing_drop(*ty) {
                loans.extend(
                    state
                        .value(&LiveSlot::Place(LivePlace::from_parts(
                            crate::PlaceRoot::Local(*local),
                            Box::new([]),
                        )))
                        .all_loans(),
                );
            }
        }
        loans
    }

    fn check_cleanup_target(
        &self,
        node: BodyNodeId,
        target: &LoanPlace,
        active: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        for loan in active {
            let definition = self
                .loans
                .get(loan)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            if definition
                .places()
                .iter()
                .any(|borrowed| target.overlaps(borrowed))
            {
                return Err(self.loan_error(BodyRule::BorrowedPlaceMutation, node, *loan)?);
            }
        }
        Ok(())
    }

    fn cleanup_observer_loans(
        &self,
        actions: &[CleanupAction],
        state: &LoanState,
    ) -> Result<BTreeSet<LoanId>, BodyCheckInternalError> {
        let mut loans = BTreeSet::new();
        for action in actions {
            if cleanup_type(action.target()).is_some_and(|ty| self.has_observing_drop(ty)) {
                loans.extend(
                    self.cleanup_target_value(action.target(), state)?
                        .all_loans(),
                );
            }
        }
        Ok(loans)
    }

    fn cleanup_target_value(
        &self,
        target: &CleanupTarget,
        state: &LoanState,
    ) -> Result<LoanValue, BodyCheckInternalError> {
        Ok(match target {
            CleanupTarget::Path(path) => state.value(&LiveSlot::Place(LivePlace::from_parts(
                path.root(),
                path.fields().iter().copied().map(Into::into).collect(),
            ))),
            CleanupTarget::Place { place, .. } => {
                let place = self
                    .input
                    .body
                    .places()
                    .get(*place)
                    .ok_or(BodyCheckInternalError::InvalidMovePlace(*place))?;
                state.value(&LiveSlot::Place(LivePlace::from_checked(place)))
            }
            CleanupTarget::Value { node, .. }
            | CleanupTarget::EnumResidual { subject: node, .. } => {
                state.value(&LiveSlot::Node(*node))
            }
            CleanupTarget::Region { binding, .. } => state.value(&LiveSlot::Place(
                LivePlace::from_parts(crate::PlaceRoot::Local(*binding), Box::new([])),
            )),
        })
    }

    fn cleanup_target_places(
        &self,
        action: &CleanupAction,
        state: &LoanState,
    ) -> Result<Vec<LoanPlace>, BodyCheckInternalError> {
        Ok(match action.target() {
            CleanupTarget::Path(path) => vec![LoanPlace::new(
                LoanRoot::Place(path.root()),
                path.fields()
                    .iter()
                    .copied()
                    .map(|field| LoanProjection::Field(field.into()))
                    .collect::<Vec<_>>(),
            )],
            CleanupTarget::Place { place, .. } => {
                let place = self
                    .input
                    .body
                    .places()
                    .get(*place)
                    .ok_or(BodyCheckInternalError::InvalidMovePlace(*place))?;
                self.access_targets(place, state)?.0
            }
            CleanupTarget::Region { binding, .. } => vec![LoanPlace::new(
                LoanRoot::Place(crate::PlaceRoot::Local(*binding)),
                [],
            )],
            CleanupTarget::Value { .. } | CleanupTarget::EnumResidual { .. } => Vec::new(),
        })
    }

    fn has_observing_drop(&self, ty: TypeId) -> bool {
        matches!(
            self.types.get(ty),
            Some(TypeKind::Nominal { definition, .. }) if self.drops.get(*definition).is_some()
        )
    }
}

fn cleanup_type(target: &CleanupTarget) -> Option<TypeId> {
    match target {
        CleanupTarget::Path(path) => Some(path.ty()),
        CleanupTarget::Place { ty, .. }
        | CleanupTarget::Value { ty, .. }
        | CleanupTarget::EnumResidual { ty, .. } => Some(*ty),
        CleanupTarget::Region { .. } => None,
    }
}
