use std::collections::BTreeMap;

use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressExtent, MachineAddressStep, MachineConstant, MachineIndex,
    MachineIndexBorrow, MachineIndexCheck, MachineOperation, MachineOperationId,
    MachineOperationKind, MachineValueId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckOptimizationReport {
    pub(super) indexes_resolved: usize,
}

/// Resolves the machine representation of constant indexes without re-proving source safety.
///
/// Bounds dispositions arrive frozen from semantic checking through MIR. Machine may replace an
/// SSA index with its constant representation, but it must preserve the existing disposition.
pub(super) fn resolve_constant_indexes(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    rewrites: &mut super::rewrite::MachineRewriteProof,
) -> Result<CheckOptimizationReport, super::MachineOptimizationError> {
    let mut report = CheckOptimizationReport::default();
    resolve_addresses(draft, constants, &mut report)?;
    resolve_index_borrows(draft, constants, rewrites, &mut report)?;
    Ok(report)
}

fn resolve_addresses(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    report: &mut CheckOptimizationReport,
) -> Result<(), super::MachineOptimizationError> {
    for address in &mut draft.addresses {
        let mut changed = false;
        let steps = address
            .steps()
            .iter()
            .map(|step| -> Result<_, super::MachineOptimizationError> {
                Ok(match *step {
                    MachineAddressStep::Index {
                        index,
                        stride,
                        bound,
                        check,
                    } => {
                        let resolved = resolve(index, constants, report);
                        validate_disposition(resolved, bound, check)?;
                        changed |= resolved != index;
                        MachineAddressStep::Index {
                            index: resolved,
                            stride,
                            bound,
                            check,
                        }
                    }
                    _ => *step,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !changed {
            continue;
        }
        *address = match address.extent() {
            MachineAddressExtent::Stored { size, alignment } => {
                MachineAddress::new(address.ty(), size, alignment, address.root(), steps)
            }
            MachineAddressExtent::View => {
                MachineAddress::new_view(address.ty(), address.root(), steps)
            }
        };
    }
    Ok(())
}

fn resolve_index_borrows(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    rewrites: &mut super::rewrite::MachineRewriteProof,
    report: &mut CheckOptimizationReport,
) -> Result<(), super::MachineOptimizationError> {
    for (index, operation) in draft.operations.iter_mut().enumerate() {
        let MachineOperationKind::IndexBorrow(borrow) = operation.kind() else {
            continue;
        };
        let borrow = *borrow;
        let resolved = resolve(borrow.index(), constants, report);
        let bound = match borrow.domain() {
            crate::MachineIndexDomain::Fixed { length, .. } => {
                crate::MachineIndexBound::Fixed(length)
            }
            crate::MachineIndexDomain::View { .. } => crate::MachineIndexBound::CurrentView,
        };
        validate_disposition(resolved, bound, borrow.check())?;
        if resolved == borrow.index() {
            continue;
        }
        let check = borrow.check();
        let result = operation.result();
        *operation = MachineOperation::new(
            MachineOperationKind::IndexBorrow(MachineIndexBorrow::new(
                borrow.receiver(),
                resolved,
                borrow.domain(),
                check,
            )),
            result,
        );
        if check == MachineIndexCheck::ProvenInBounds {
            rewrites.prove_pure(MachineOperationId::new(index));
        }
    }
    Ok(())
}

fn validate_disposition(
    index: MachineIndex,
    bound: crate::MachineIndexBound,
    check: MachineIndexCheck,
) -> Result<(), super::MachineOptimizationError> {
    let (MachineIndex::Constant(index), crate::MachineIndexBound::Fixed(length)) = (index, bound)
    else {
        return Ok(());
    };
    let valid = match check {
        MachineIndexCheck::Required => true,
        MachineIndexCheck::ProvenInBounds => index < length,
        MachineIndexCheck::ProvenTrap => index >= length,
    };
    if valid {
        Ok(())
    } else {
        Err(super::MachineOptimizationError::InvalidIndexDisposition {
            index,
            length,
            check,
        })
    }
}

fn resolve(
    index: MachineIndex,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    report: &mut CheckOptimizationReport,
) -> MachineIndex {
    match index {
        MachineIndex::Value(value) => match constants.get(&value).and_then(constant_index) {
            Some(index) => {
                report.indexes_resolved += 1;
                MachineIndex::Constant(index)
            }
            None => MachineIndex::Value(value),
        },
        MachineIndex::Constant(index) => MachineIndex::Constant(index),
    }
}

fn constant_index(constant: &MachineConstant) -> Option<u64> {
    match constant {
        MachineConstant::Integer(value) => u64::try_from(*value).ok(),
        MachineConstant::Bool(_)
        | MachineConstant::Character(_)
        | MachineConstant::Float32(_)
        | MachineConstant::Float64(_)
        | MachineConstant::Text(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::validate_disposition;
    use crate::{MachineIndex, MachineIndexBound, MachineIndexCheck, MachineOptimizationError};

    #[test]
    fn fixed_bounds_validate_frozen_dispositions_without_inference() {
        assert!(
            validate_disposition(
                MachineIndex::Constant(1),
                MachineIndexBound::Fixed(2),
                MachineIndexCheck::ProvenInBounds,
            )
            .is_ok()
        );
        assert!(
            validate_disposition(
                MachineIndex::Constant(2),
                MachineIndexBound::Fixed(2),
                MachineIndexCheck::ProvenTrap,
            )
            .is_ok()
        );
        assert!(
            validate_disposition(
                MachineIndex::Constant(1),
                MachineIndexBound::Fixed(2),
                MachineIndexCheck::Required,
            )
            .is_ok()
        );

        assert!(matches!(
            validate_disposition(
                MachineIndex::Constant(2),
                MachineIndexBound::Fixed(2),
                MachineIndexCheck::ProvenInBounds,
            ),
            Err(MachineOptimizationError::InvalidIndexDisposition { .. })
        ));
        assert!(matches!(
            validate_disposition(
                MachineIndex::Constant(1),
                MachineIndexBound::Fixed(2),
                MachineIndexCheck::ProvenTrap,
            ),
            Err(MachineOptimizationError::InvalidIndexDisposition { .. })
        ));
    }
}
