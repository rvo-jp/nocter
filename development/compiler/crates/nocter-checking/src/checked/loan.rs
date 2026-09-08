use std::collections::BTreeMap;

use nocter_model::{
    Arena, BodyId, BodyNodeId, BorrowCapability, CaptureId, ClosureId, FieldId, LocalBindingId,
    ParameterId, ParameterOrigin,
};

use super::PlaceRoot;

/// Root storage addressed by a loan.
///
/// `External` represents storage reached through an input borrow carrier. It is deliberately
/// distinct from the place that stores that carrier: assigning or moving the carrier does not
/// mutate the referent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoanRoot {
    Place(PlaceRoot),
    External(PlaceRoot),
}

/// Stable identity of a source-level loan within one checked body.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoanId {
    Parameter(ParameterId),
    ClosureParameter {
        closure: ClosureId,
        origin: ParameterOrigin,
    },
    ClosureCapture {
        closure: ClosureId,
        capture: CaptureId,
    },
    Node(BodyNodeId),
    /// An implicit loan created for one operand of a compound checked operation.
    Operand {
        node: BodyNodeId,
        position: u16,
    },
}

/// Source storage whose address must remain stable while one deferred body is suspended.
///
/// Loan analysis owns this classification. Later lowering stages translate these semantic
/// identities into their own storage identities without rediscovering which loans cross an
/// `await`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SuspensionStorage {
    Parameter(ParameterId),
    Local(LocalBindingId),
    Capture(CaptureId),
    Value(BodyNodeId),
}

/// Canonical place projection used for borrow overlap.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoanProjection {
    Field(FieldId),
    TupleElement(usize),
    /// An index, dereference, payload, or other projection that cannot prove disjointness.
    Opaque,
}

/// One storage place borrowed by a loan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LoanPlace {
    root: LoanRoot,
    projections: Box<[LoanProjection]>,
}

impl LoanPlace {
    pub(crate) fn new(root: LoanRoot, projections: impl Into<Box<[LoanProjection]>>) -> Self {
        Self {
            root,
            projections: projections.into(),
        }
    }

    #[must_use]
    pub const fn root(&self) -> LoanRoot {
        self.root
    }

    #[must_use]
    pub const fn projections(&self) -> &[LoanProjection] {
        &self.projections
    }

    /// Only different statically named fields establish disjointness.
    #[must_use]
    pub fn overlaps(&self, another: &Self) -> bool {
        if self.root != another.root {
            return false;
        }
        for (left, right) in self.projections.iter().zip(another.projections.iter()) {
            if left == right {
                continue;
            }
            return !matches!(
                (left, right),
                (LoanProjection::Field(_), LoanProjection::Field(_))
                    | (
                        LoanProjection::TupleElement(_),
                        LoanProjection::TupleElement(_)
                    )
            );
        }
        true
    }
}

/// One checked source-level loan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedLoan {
    capability: BorrowCapability,
    places: Box<[LoanPlace]>,
    parents: Box<[LoanId]>,
}

impl CheckedLoan {
    pub(crate) fn new(
        capability: BorrowCapability,
        places: impl Into<Box<[LoanPlace]>>,
        parents: impl Into<Box<[LoanId]>>,
    ) -> Self {
        Self {
            capability,
            places: places.into(),
            parents: parents.into(),
        }
    }

    #[must_use]
    pub const fn capability(&self) -> BorrowCapability {
        self.capability
    }

    #[must_use]
    pub const fn places(&self) -> &[LoanPlace] {
        &self.places
    }

    #[must_use]
    pub const fn parents(&self) -> &[LoanId] {
        &self.parents
    }

    pub(crate) fn merge_with(&mut self, another: &Self) -> Result<(), ()> {
        if self.capability != another.capability {
            return Err(());
        }
        let mut places = self.places.to_vec();
        places.extend_from_slice(&another.places);
        places.sort_unstable();
        places.dedup();
        self.places = places.into_boxed_slice();
        let mut parents = self.parents.to_vec();
        parents.extend_from_slice(&another.parents);
        parents.sort_unstable();
        parents.dedup();
        self.parents = parents.into_boxed_slice();
        Ok(())
    }
}

/// Complete loan authority for one checked body.
#[derive(Clone, Debug)]
pub struct CheckedBodyLoans {
    loans: BTreeMap<LoanId, CheckedLoan>,
    live_before: Arena<BodyNodeId, Box<[LoanId]>>,
    suspension_storage: BTreeMap<BodyNodeId, Box<[SuspensionStorage]>>,
}

impl CheckedBodyLoans {
    pub(crate) const fn new(
        loans: BTreeMap<LoanId, CheckedLoan>,
        live_before: Arena<BodyNodeId, Box<[LoanId]>>,
        suspension_storage: BTreeMap<BodyNodeId, Box<[SuspensionStorage]>>,
    ) -> Self {
        Self {
            loans,
            live_before,
            suspension_storage,
        }
    }

    #[must_use]
    pub const fn loans(&self) -> &BTreeMap<LoanId, CheckedLoan> {
        &self.loans
    }

    #[must_use]
    pub const fn live_before(&self) -> &Arena<BodyNodeId, Box<[LoanId]>> {
        &self.live_before
    }

    /// Stable source storage required by one checked suspension.
    #[must_use]
    pub fn suspension_storage(&self, await_node: BodyNodeId) -> Option<&[SuspensionStorage]> {
        self.suspension_storage.get(&await_node).map(Box::as_ref)
    }

    #[must_use]
    pub const fn suspensions(&self) -> &BTreeMap<BodyNodeId, Box<[SuspensionStorage]>> {
        &self.suspension_storage
    }
}

/// Dense program-wide source-loan authority.
#[derive(Clone, Debug)]
pub struct LoanTable {
    bodies: Arena<BodyId, CheckedBodyLoans>,
}

impl LoanTable {
    pub(crate) const fn new(bodies: Arena<BodyId, CheckedBodyLoans>) -> Self {
        Self { bodies }
    }

    #[must_use]
    pub fn body(&self, body: BodyId) -> Option<&CheckedBodyLoans> {
        self.bodies.get(body)
    }

    #[must_use]
    pub const fn bodies(&self) -> &Arena<BodyId, CheckedBodyLoans> {
        &self.bodies
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, FieldId, LocalBindingId};

    use super::{LoanPlace, LoanProjection, LoanRoot};
    use crate::PlaceRoot;

    #[test]
    fn only_distinct_named_fields_prove_disjointness() {
        let mut locals = ArenaBuilder::<LocalBindingId, _>::new();
        let local = locals.insert(());
        let _ = locals.finish();
        let mut fields = ArenaBuilder::<FieldId, _>::new();
        let left = fields.insert(());
        let right = fields.insert(());
        let _ = fields.finish();
        let root = LoanRoot::Place(PlaceRoot::Local(local));
        let whole = LoanPlace::new(root, []);
        let left = LoanPlace::new(root, [LoanProjection::Field(left)]);
        let right = LoanPlace::new(root, [LoanProjection::Field(right)]);
        let indexed = LoanPlace::new(root, [LoanProjection::Opaque]);

        assert!(whole.overlaps(&left));
        assert!(!left.overlaps(&right));
        assert!(left.overlaps(&indexed));
    }
}
