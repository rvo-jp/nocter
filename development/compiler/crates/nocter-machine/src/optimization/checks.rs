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
                    let resolved = resolve(index, constants, report);
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
        let resolved = resolve(borrow.index(), constants, report);
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
