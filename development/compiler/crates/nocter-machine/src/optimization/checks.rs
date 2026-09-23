use std::collections::BTreeMap;

use crate::identity::MachineId;
use crate::{
    MachineAddress, MachineAddressExtent, MachineAddressStep, MachineConstant, MachineIndex,
    MachineIndexBorrow, MachineIndexBound, MachineIndexCheck, MachineIndexDomain, MachineOperation,
    MachineOperationId, MachineOperationKind, MachineValueId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CheckOptimizationReport {
    pub(super) indexes_resolved: usize,
    pub(super) bounds_checks_elided: usize,
    pub(super) bounds_traps_proven: usize,
}

/// Resolves constant indexes and owns every static bounds conclusion before Machine is frozen.
///
/// Targets consume `MachineIndexCheck` directly. An in-range fixed index has no dynamic check; an
/// out-of-range fixed index remains an explicit unconditional trap. Dynamic and view-bounded
/// indexes retain the source-visible runtime check.
pub(super) fn resolve_constant_indexes(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    rewrites: &mut super::rewrite::MachineRewriteProof,
) -> CheckOptimizationReport {
    let mut report = CheckOptimizationReport::default();
    resolve_addresses(draft, constants, &mut report);
    resolve_index_borrows(draft, constants, rewrites, &mut report);
    report
}

fn resolve_addresses(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    report: &mut CheckOptimizationReport,
) {
    for address in &mut draft.addresses {
        let mut changed = false;
        let steps = address
            .steps()
            .iter()
            .map(|step| match *step {
                MachineAddressStep::Index {
                    index,
                    stride,
                    bound,
                    check,
                } => {
                    let (resolved, resolved_check) =
                        resolve(index, bound, check, constants, report);
                    changed |= resolved != index || resolved_check != check;
                    MachineAddressStep::Index {
                        index: resolved,
                        stride,
                        bound,
                        check: resolved_check,
                    }
                }
                _ => *step,
            })
            .collect::<Vec<_>>();
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
}

fn resolve_index_borrows(
    draft: &mut crate::program::MachineBodyDraft,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    rewrites: &mut super::rewrite::MachineRewriteProof,
    report: &mut CheckOptimizationReport,
) {
    for (index, operation) in draft.operations.iter_mut().enumerate() {
        let MachineOperationKind::IndexBorrow(borrow) = operation.kind() else {
            continue;
        };
        let borrow = *borrow;
        let bound = match borrow.domain() {
            MachineIndexDomain::Fixed { length, .. } => MachineIndexBound::Fixed(length),
            MachineIndexDomain::View { .. } => MachineIndexBound::CurrentView,
        };
        let (resolved, check) = resolve(borrow.index(), bound, borrow.check(), constants, report);
        if resolved == borrow.index() && check == borrow.check() {
            continue;
        }
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
}

fn resolve(
    index: MachineIndex,
    bound: MachineIndexBound,
    check: MachineIndexCheck,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    report: &mut CheckOptimizationReport,
) -> (MachineIndex, MachineIndexCheck) {
    debug_assert_eq!(check, MachineIndexCheck::Required);
    let index = match index {
        MachineIndex::Value(value) => match constants.get(&value).and_then(constant_index) {
            Some(index) => {
                report.indexes_resolved += 1;
                MachineIndex::Constant(index)
            }
            None => return (MachineIndex::Value(value), MachineIndexCheck::Required),
        },
        MachineIndex::Constant(index) => MachineIndex::Constant(index),
    };
    let (MachineIndex::Constant(index), MachineIndexBound::Fixed(length)) = (index, bound) else {
        return (index, MachineIndexCheck::Required);
    };
    if index < length {
        report.bounds_checks_elided += 1;
        (
            MachineIndex::Constant(index),
            MachineIndexCheck::ProvenInBounds,
        )
    } else {
        report.bounds_traps_proven += 1;
        (MachineIndex::Constant(index), MachineIndexCheck::ProvenTrap)
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
