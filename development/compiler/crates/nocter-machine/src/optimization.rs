use std::collections::BTreeMap;
use std::fmt;

use crate::effect::MachineOperationEffect;
use crate::{
    MachineBinaryOperation, MachineBranchTarget, MachineConstant, MachineOperation,
    MachineOperationKind, MachineSwitchValue, MachineTerminator, MachineUnaryOperation,
    MachineValueId,
};

mod prune;

/// Structural changes made by the one Machine body optimization pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MachineOptimizationReport {
    operations_folded: usize,
    terminators_folded: usize,
    blocks_removed: usize,
    operations_removed: usize,
    values_removed: usize,
    stack_objects_removed: usize,
    addresses_removed: usize,
    drop_flags_removed: usize,
    packs_removed: usize,
}

impl MachineOptimizationReport {
    #[must_use]
    pub const fn operations_folded(self) -> usize {
        self.operations_folded
    }

    #[must_use]
    pub const fn terminators_folded(self) -> usize {
        self.terminators_folded
    }

    #[must_use]
    pub const fn blocks_removed(self) -> usize {
        self.blocks_removed
    }

    #[must_use]
    pub const fn operations_removed(self) -> usize {
        self.operations_removed
    }

    #[must_use]
    pub const fn values_removed(self) -> usize {
        self.values_removed
    }

    #[must_use]
    pub const fn stack_objects_removed(self) -> usize {
        self.stack_objects_removed
    }

    #[must_use]
    pub const fn addresses_removed(self) -> usize {
        self.addresses_removed
    }

    #[must_use]
    pub const fn drop_flags_removed(self) -> usize {
        self.drop_flags_removed
    }

    #[must_use]
    pub const fn packs_removed(self) -> usize {
        self.packs_removed
    }

    #[must_use]
    pub const fn changed(self) -> bool {
        self.operations_folded != 0
            || self.terminators_folded != 0
            || self.blocks_removed != 0
            || self.operations_removed != 0
            || self.values_removed != 0
            || self.stack_objects_removed != 0
            || self.addresses_removed != 0
            || self.drop_flags_removed != 0
            || self.packs_removed != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineOptimizationError {
    UnknownBlock(crate::MachineBlockId),
    UnknownStack(crate::MachineStackId),
    UnknownAddress(crate::MachineAddressId),
    UnknownOperation(crate::MachineOperationId),
    UnknownValue(crate::MachineValueId),
    UnknownDropFlag(crate::MachineDropFlagId),
    UnknownPack(crate::MachinePackId),
}

impl fmt::Display for MachineOptimizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine optimization failed: {self:?}")
    }
}

impl std::error::Error for MachineOptimizationError {}

/// Optimizes the complete mutable draft before immutable tables and liveness are constructed.
/// No later stage can observe the draft or stale pre-optimization dataflow.
pub(crate) fn optimize(
    draft: &mut crate::program::MachineBodyDraft,
    execution: &mut crate::MachineFunctionExecution,
) -> Result<MachineOptimizationReport, MachineOptimizationError> {
    let mut report = MachineOptimizationReport::default();
    let constants = fold_operations(&mut draft.operations, &mut report);
    fold_terminators(&mut draft.blocks, &constants, &mut report);
    prune::unreachable_and_unused(draft, execution, &mut report)?;
    Ok(report)
}

fn fold_operations(
    operations: &mut [MachineOperation],
    report: &mut MachineOptimizationReport,
) -> BTreeMap<MachineValueId, MachineConstant> {
    let mut constants = BTreeMap::new();
    let mut changed = true;
    while changed {
        changed = false;
        for operation in &mut *operations {
            let Some(result) = operation.result() else {
                continue;
            };
            let folded = fold_operation(operation.kind(), &constants);
            let constant = match (operation.kind(), folded) {
                (MachineOperationKind::Constant(constant), _) => Some(*constant),
                (_, Some(constant)) => {
                    *operation = MachineOperation::new(
                        MachineOperationKind::Constant(constant),
                        Some(result),
                    );
                    report.operations_folded += 1;
                    changed = true;
                    Some(constant)
                }
                (_, None) => None,
            };
            if let Some(constant) = constant {
                constants.insert(result, constant);
            }
        }
    }
    constants
}

fn fold_operation(
    operation: &MachineOperationKind,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
) -> Option<MachineConstant> {
    if operation.effect() != MachineOperationEffect::Pure {
        return None;
    }
    match operation {
        MachineOperationKind::Unary {
            operation: MachineUnaryOperation::LogicalNot,
            operand,
        } => match constants.get(operand) {
            Some(MachineConstant::Bool(value)) => Some(MachineConstant::Bool(!value)),
            _ => None,
        },
        MachineOperationKind::Binary {
            operation: MachineBinaryOperation::Equal,
            left,
            right,
        } => match (constants.get(left), constants.get(right)) {
            (Some(MachineConstant::Bool(left)), Some(MachineConstant::Bool(right))) => {
                Some(MachineConstant::Bool(left == right))
            }
            _ => None,
        },
        _ => None,
    }
}

fn fold_terminators(
    blocks: &mut [crate::MachineBlock],
    constants: &BTreeMap<MachineValueId, MachineConstant>,
    report: &mut MachineOptimizationReport,
) {
    for block in blocks {
        let Some(terminator) = fold_terminator(block.terminator(), constants) else {
            continue;
        };
        *block = crate::MachineBlock::new(
            block.parameters().to_vec(),
            block.operations().to_vec(),
            terminator,
        );
        report.terminators_folded += 1;
    }
}

fn fold_terminator(
    terminator: &MachineTerminator,
    constants: &BTreeMap<MachineValueId, MachineConstant>,
) -> Option<MachineTerminator> {
    match terminator {
        MachineTerminator::Branch {
            condition,
            then_target,
            else_target,
        } => match constants.get(condition) {
            Some(MachineConstant::Bool(true)) => Some(MachineTerminator::Goto(then_target.clone())),
            Some(MachineConstant::Bool(false)) => {
                Some(MachineTerminator::Goto(else_target.clone()))
            }
            _ if then_target == else_target => Some(MachineTerminator::Goto(then_target.clone())),
            _ => None,
        },
        MachineTerminator::SwitchValue {
            subject,
            cases,
            fallback,
        } => match constants.get(subject) {
            Some(MachineConstant::Integer(subject)) => Some(MachineTerminator::Goto(
                cases
                    .iter()
                    .find(|case| case.value() == MachineSwitchValue::Integer(*subject))
                    .map_or(fallback, crate::MachineSwitchCase::target)
                    .clone(),
            )),
            _ => common_switch_target(cases, fallback).map(MachineTerminator::Goto),
        },
        _ => None,
    }
}

fn common_switch_target(
    cases: &[crate::MachineSwitchCase],
    fallback: &MachineBranchTarget,
) -> Option<MachineBranchTarget> {
    cases
        .iter()
        .all(|case| case.target() == fallback)
        .then(|| fallback.clone())
}

#[cfg(test)]
mod tests {
    use super::{MachineOptimizationReport, fold_operation, fold_terminator};
    use std::collections::BTreeMap;

    use crate::identity::MachineId;
    use crate::{
        MachineBinaryOperation, MachineBlockId, MachineBranchTarget, MachineConstant,
        MachineOperationKind, MachineTerminator, MachineUnaryOperation, MachineValueId,
    };

    #[test]
    fn folds_boolean_values_without_touching_trapping_arithmetic() {
        let source = MachineValueId::new(0);
        let mut constants = BTreeMap::new();
        constants.insert(source, MachineConstant::Bool(true));
        assert_eq!(
            fold_operation(
                &MachineOperationKind::Unary {
                    operation: MachineUnaryOperation::LogicalNot,
                    operand: source,
                },
                &constants,
            ),
            Some(MachineConstant::Bool(false))
        );
        assert_eq!(
            fold_operation(
                &MachineOperationKind::Binary {
                    operation: MachineBinaryOperation::Add,
                    left: source,
                    right: source,
                },
                &constants,
            ),
            None
        );
    }

    #[test]
    fn folds_constant_branch_to_the_exact_selected_edge() {
        let condition = MachineValueId::new(0);
        let then_target = MachineBranchTarget::new(MachineBlockId::new(1), []);
        let else_target = MachineBranchTarget::new(MachineBlockId::new(2), []);
        let mut constants = BTreeMap::new();
        constants.insert(condition, MachineConstant::Bool(false));
        let folded = fold_terminator(
            &MachineTerminator::Branch {
                condition,
                then_target,
                else_target: else_target.clone(),
            },
            &constants,
        );
        assert_eq!(folded, Some(MachineTerminator::Goto(else_target)));
    }

    #[test]
    fn report_changed_requires_an_actual_transformation() {
        assert!(!MachineOptimizationReport::default().changed());
    }
}
