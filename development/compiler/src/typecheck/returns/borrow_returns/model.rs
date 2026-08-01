use super::*;

pub(in crate::typecheck::returns) type BorrowReturnEnvironment = ProvenanceEnvironment;
pub(in crate::typecheck::returns) type BorrowReturnFlow = ProvenanceFlow;
pub(in crate::typecheck::returns) type BorrowReturnProvenance = ValueProvenance;
pub(in crate::typecheck::returns) type BorrowReturnSummaries = ProvenanceSummaries;

pub(in crate::typecheck::returns) fn borrow_return_fallible_provenance(
    success: Option<BorrowReturnProvenance>,
    error: Option<BorrowReturnProvenance>,
) -> Option<BorrowReturnProvenance> {
    fallible_provenance(success, error)
}

pub(in crate::typecheck::returns) fn merge_borrow_return_provenance(
    provenance: &mut Option<BorrowReturnProvenance>,
    next: Option<BorrowReturnProvenance>,
) {
    merge_provenance(provenance, next);
}
