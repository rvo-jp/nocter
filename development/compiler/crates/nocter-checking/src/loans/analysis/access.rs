use std::collections::BTreeSet;

use nocter_model::{BodyNodeId, BorrowCapability};

use super::{AccessKind, Analyzer};
use crate::loans::liveness::{LivePlace, LiveSlot};
use crate::loans::state::LoanState;
use crate::loans::value::LoanValue;
use crate::{
    BodyCheckInternalError, BodyRelationError, BodyRelationNote, BodyRule, CheckedLoan, LoanId,
    LoanPlace, LoanProjection, LoanRoot, PlaceProjection, PlaceRoot,
};

impl Analyzer<'_> {
    pub(super) fn evaluate_place_indices(
        &mut self,
        place: nocter_model::PlaceId,
        state: &mut LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyRelationError> {
        let nodes = self
            .input
            .body()
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
            .body()
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        Ok(match place.root() {
            PlaceRoot::Value(value) => state.value(&LiveSlot::Node(value)),
            PlaceRoot::Parameter(_) | PlaceRoot::Local(_) | PlaceRoot::Capture(_) => {
                state.value(&LiveSlot::Place(LivePlace::from_checked(place)))
            }
            PlaceRoot::Static(_) => LoanValue::independent(),
        })
    }

    pub(super) fn remove_place(
        &self,
        place: nocter_model::PlaceId,
        state: &mut LoanState,
    ) -> Result<(), BodyCheckInternalError> {
        let place = self
            .input
            .body()
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
    ) -> Result<(), BodyRelationError> {
        let place = self
            .input
            .body()
            .places()
            .get(place)
            .ok_or(BodyCheckInternalError::InvalidMovePlace(place))?;
        let (targets, authorization) = self.access_targets(place, state)?;
        let excluded = self.authorization_closure(&authorization);
        self.check_loan_targets(node, &targets, &excluded, kind, state, extra)
    }

    fn check_loan_targets(
        &self,
        node: BodyNodeId,
        targets: &[LoanPlace],
        excluded: &BTreeSet<LoanId>,
        kind: AccessKind,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<(), BodyRelationError> {
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
                return Err(self.loan_error(rule, node, loan));
            }
        }
        Ok(())
    }

    /// Creates an implicit loan of checked value storage without manufacturing a syntax place.
    ///
    /// Protocol operations such as lending iteration own semantic temporaries that have no
    /// source-level `PlaceId`. They still pass through the same overlap and liveness authority as
    /// authored borrows.
    pub(super) fn issue_value_loan_as(
        &mut self,
        loan: LoanId,
        node: BodyNodeId,
        value: BodyNodeId,
        capability: BorrowCapability,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<LoanValue, BodyRelationError> {
        let places = vec![LoanPlace::new(LoanRoot::Place(PlaceRoot::Value(value)), [])];
        self.check_loan_targets(
            node,
            &places,
            &BTreeSet::new(),
            AccessKind::Borrow(capability),
            state,
            extra,
        )?;
        let checked = CheckedLoan::new(capability, places, []);
        if let Some(current) = self.loans.get_mut(&loan) {
            current
                .merge_with(&checked)
                .map_err(|()| BodyCheckInternalError::LoanAnalysis)?;
        } else {
            self.loans.insert(loan, checked);
        }
        Ok(LoanValue::from_loan(loan))
    }

    /// Reborrows storage reached through an existing borrow carrier.
    ///
    /// The parent loans authorize access to their referents but do not authorize overlap with a
    /// sibling reborrow that remains live. This distinction makes repeated `&+self` calls reject
    /// the second call when the first result still carries its explicit `from self` relation.
    pub(super) fn issue_reborrow_as(
        &mut self,
        loan: LoanId,
        node: BodyNodeId,
        carrier: &LoanValue,
        capability: BorrowCapability,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<LoanValue, BodyRelationError> {
        let parents = carrier.all_loans();
        let mut places = Vec::new();
        for parent in &parents {
            let definition = self
                .loans
                .get(parent)
                .ok_or(BodyCheckInternalError::LoanAnalysis)?;
            places.extend_from_slice(definition.places());
        }
        places.sort_unstable();
        places.dedup();
        let excluded = self.authorization_closure(&parents);
        self.check_loan_targets(
            node,
            &places,
            &excluded,
            AccessKind::Borrow(capability),
            state,
            extra,
        )?;
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

    pub(super) fn issue_loan(
        &mut self,
        node: BodyNodeId,
        place: nocter_model::PlaceId,
        capability: BorrowCapability,
        state: &LoanState,
        extra: &BTreeSet<LoanId>,
    ) -> Result<LoanValue, BodyRelationError> {
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
    ) -> Result<LoanValue, BodyRelationError> {
        self.check_place_access(node, place, AccessKind::Borrow(capability), state, extra)?;
        let place = self
            .input
            .body()
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
            PlaceRoot::Static(_) => LoanValue::independent(),
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
    ) -> BodyRelationError {
        let notes = match conflicting {
            LoanId::Node(loan) | LoanId::Operand { node: loan, .. } => vec![BodyRelationNote::new(
                "conflicting loan is created here",
                loan,
            )],
            LoanId::Parameter(_)
            | LoanId::ClosureParameter { .. }
            | LoanId::ClosureCapture { .. } => Vec::new(),
        };
        self.input.reject_with_notes(rule, node, notes)
    }
}
