use std::collections::BTreeSet;

use nocter_diagnostics::DiagnosticNote;
use nocter_model::{BodyNodeId, BorrowCapability};
use nocter_source_index::SourceOrigin;

use super::{AccessKind, Analyzer};
use crate::loans::liveness::{LivePlace, LiveSlot};
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    BodyCheckError, BodyCheckInternalError, BodyRule, CheckedLoan, LoanId, LoanPlace,
    LoanProjection, LoanRoot, PlaceProjection, PlaceRoot,
};

impl Analyzer<'_, '_> {
    pub(super) fn evaluate_place_indices(
        &mut self,
        place: nocter_model::PlaceId,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        let nodes = self
            .input
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?
            .evaluation_nodes()
            .collect::<Vec<_>>();
        for node in nodes {
            if !self.evaluate(node, state, extra)?.1 {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn read_place(
        &self,
        place: nocter_model::PlaceId,
        state: &LoanState,
    ) -> Result<LoanValue, BodyCheckInternalError> {
        let place = self
            .input
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        Ok(match place.root() {
            PlaceRoot::Value(value) => state.value(&LiveSlot::Node(value)),
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Capture(_) => {
                state.value(&LiveSlot::Place(LivePlace::from_checked(place)))
            }
        })
    }

    pub(super) fn remove_place(
        &self,
        place: nocter_model::PlaceId,
        state: &mut LoanState,
    ) -> Result<(), BodyCheckInternalError> {
        let place = self
            .input
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        state.remove_place(&LivePlace::from_checked(place));
        Ok(())
    }

    pub(super) fn check_place_access(
        &self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        kind: AccessKind,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyCheckError> {
        let place = self
            .input
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let (targets, authorization) = self.access_targets(place, state)?;
        let excluded = self.authorization_closure(&authorization);
        let active = self.active_loans(node, state, extra);
        for loan in active {
            if excluded.contains(&loan) {
                continue;
            }
            let definition = self
                .loans
                .get(&loan)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            let incompatible = match kind {
                AccessKind::Read => definition.capability() == BorrowCapability::ReadWrite,
                AccessKind::Write | AccessKind::Borrow(BorrowCapability::ReadWrite) => true,
                AccessKind::Borrow(BorrowCapability::Readonly) => {
                    definition.capability() == BorrowCapability::ReadWrite
                }
            };
            if incompatible
                && targets.iter().any(|target| {
                    definition
                        .places()
                        .iter()
                        .any(|borrowed| target.overlaps(borrowed))
                })
            {
                let rule = match kind {
                    AccessKind::Borrow(_) => BodyRule::ConflictingLoan,
                    AccessKind::Read | AccessKind::Write => BodyRule::BorrowedPlaceMutation,
                };
                return Err(self.loan_error(rule, node, loan)?);
            }
        }
        Ok(())
    }

    pub(super) fn issue_loan(
        &mut self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        capability: BorrowCapability,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<LoanValue, BodyCheckError> {
        self.issue_loan_as(LoanId::Node(node), node, place, capability, state, extra)
    }

    pub(super) fn issue_loan_as(
        &mut self,
        loan: LoanId,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        capability: BorrowCapability,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<LoanValue, BodyCheckError> {
        self.check_place_access(node, place, AccessKind::Borrow(capability), state, extra)?;
        let place = self
            .input
            .body
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let (places, parents) = self.access_targets(place, state)?;
        let checked = CheckedLoan::new(
            capability,
            places,
            parents.iter().copied().collect::<Vec<_>>(),
        );
        if let Some(current) = self.loans.get_mut(&loan) {
            current
                .merge_with(&checked)
                .map_err(|()| BodyCheckInternalError::LoanAnalysis)?;
        } else {
            self.loans.insert(loan, checked);
        }
        Ok(LoanValue::from_loan(loan))
    }

    pub(super) fn access_targets(
        &self,
        place: &crate::CheckedPlace,
        state: &LoanState,
    ) -> Result<(Vec<LoanPlace>, BTreeSet<LoanId>), BodyCheckInternalError> {
        let dereference = place
            .projections()
            .iter()
            .position(|projection| matches!(projection, PlaceProjection::BorrowDeref { .. }));
        let Some(dereference) = dereference else {
            return Ok((vec![Self::direct_loan_place(place)], BTreeSet::new()));
        };
        let prefix = place.projections()[..dereference]
            .iter()
            .map_while(|projection| match projection {
                PlaceProjection::Field { field, .. } => Some(LoanProjection::Field(*field)),
                PlaceProjection::TupleElement { index, .. } => {
                    Some(LoanProjection::TupleElement(*index))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let carrier = match place.root() {
            PlaceRoot::Value(value) => state.value(&LiveSlot::Node(value)),
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Capture(_) => state.value(
                &LiveSlot::Place(LivePlace::from_parts(place.root(), prefix)),
            ),
        };
        let parents = carrier.all_loans();
        let suffix = Self::loan_projections(&place.projections()[dereference + 1..]);
        let mut targets = Vec::new();
        for parent in &parents {
            let definition = self
                .loans
                .get(parent)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            for borrowed in definition.places() {
                let mut projections = borrowed.projections().to_vec();
                projections.extend_from_slice(&suffix);
                targets.push(LoanPlace::new(borrowed.root(), projections));
            }
        }
        if targets.is_empty() {
            return Ok((
                vec![LoanPlace::new(LoanRoot::External(place.root()), suffix)],
                BTreeSet::new(),
            ));
        }
        targets.sort_unstable();
        targets.dedup();
        Ok((targets, parents))
    }

    pub(super) fn direct_loan_place(place: &crate::CheckedPlace) -> LoanPlace {
        LoanPlace::new(
            LoanRoot::Place(place.root()),
            Self::loan_projections(place.projections()),
        )
    }

    pub(super) fn loan_projections(projections: &[PlaceProjection]) -> Vec<LoanProjection> {
        projections
            .iter()
            .map(|projection| match projection {
                PlaceProjection::Field { field, .. } => LoanProjection::Field(*field),
                PlaceProjection::TupleElement { index, .. } => LoanProjection::TupleElement(*index),
                PlaceProjection::BorrowDeref { .. }
                | PlaceProjection::BuiltinIndex { .. }
                | PlaceProjection::CoercedBuiltinIndex { .. }
                | PlaceProjection::SelectedIndex { .. } => LoanProjection::Opaque,
            })
            .collect()
    }

    pub(super) fn authorization_closure(&self, roots: &BTreeSet<LoanId>) -> BTreeSet<LoanId> {
        let mut closure = roots.clone();
        let mut pending = roots.iter().copied().collect::<Vec<_>>();
        while let Some(loan) = pending.pop() {
            if let Some(definition) = self.loans.get(&loan) {
                for parent in definition.parents() {
                    if closure.insert(*parent) {
                        pending.push(*parent);
                    }
                }
            }
        }
        closure
    }

    pub(super) fn loan_error(
        &self,
        rule: BodyRule,
        node: BodyNodeId,
        conflicting: LoanId,
    ) -> Result<BodyCheckError, BodyCheckInternalError> {
        let primary = self.node_origin(node)?;
        let notes = match conflicting {
            LoanId::Node(loan) | LoanId::Operand { node: loan, .. } => vec![DiagnosticNote::new(
                "conflicting loan is created here",
                self.node_origin(loan)?,
            )],
            LoanId::Parameter(_)
            | LoanId::ClosureParameter { .. }
            | LoanId::ClosureCapture { .. } => Vec::new(),
        };
        Ok(BodyCheckError::from_rule(
            rule,
            rule.diagnostic_with_notes(primary, notes),
        ))
    }

    pub(super) fn node_origin(
        &self,
        node: BodyNodeId,
    ) -> Result<SourceOrigin, BodyCheckInternalError> {
        self.input
            .origins
            .get(&node)
            .copied()
            .ok_or(BodyCheckInternalError::MissingNodeOrigin(node))
    }
}
