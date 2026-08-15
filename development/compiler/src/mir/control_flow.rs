//! Executable edges for validation without changing structural MIR topology.

use super::{BasicBlockId, CallContinuation, Operand, ScalarType, Terminator};

pub(super) fn successors(terminator: &Terminator) -> Vec<BasicBlockId> {
    match terminator {
        Terminator::Goto { target } | Terminator::Drop { target, .. } => vec![*target],
        Terminator::Switch {
            condition,
            then_target,
            else_target,
            ..
        } => switch_targets(condition, *then_target, *else_target),
        Terminator::Call { continuation, .. } => match continuation {
            CallContinuation::Continue { target } | CallContinuation::Return { target, .. } => {
                vec![*target]
            }
            CallContinuation::Outcome {
                success, failure, ..
            }
            | CallContinuation::OutcomeEffect {
                success, failure, ..
            } => distinct_targets(*success, *failure),
            CallContinuation::Never => Vec::new(),
        },
        Terminator::InspectOutcome {
            success, failure, ..
        } => distinct_targets(*success, *failure),
        Terminator::Trap
        | Terminator::PropagateFailure
        | Terminator::ReturnOutcome { .. }
        | Terminator::ReturnFailure { .. }
        | Terminator::ReturnOutcomeSuccess { .. }
        | Terminator::ReturnOptionalNone
        | Terminator::ReturnValue { .. }
        | Terminator::Return => Vec::new(),
    }
}

pub(super) fn switch_targets(
    condition: &Operand,
    then_target: BasicBlockId,
    else_target: BasicBlockId,
) -> Vec<BasicBlockId> {
    let Operand::Constant(constant) = condition else {
        return distinct_targets(then_target, else_target);
    };
    if constant.scalar != ScalarType::Bool || constant.value > 1 {
        return distinct_targets(then_target, else_target);
    }
    vec![if constant.value == 1 {
        then_target
    } else {
        else_target
    }]
}

fn distinct_targets(then_target: BasicBlockId, else_target: BasicBlockId) -> Vec<BasicBlockId> {
    if then_target == else_target {
        vec![then_target]
    } else {
        vec![then_target, else_target]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_only_the_executable_constant_boolean_edge() {
        let condition = Operand::Constant(crate::mir::Constant {
            ty: crate::semantic::TyId::from_index(0),
            scalar: ScalarType::Bool,
            value: 0,
        });
        assert_eq!(
            switch_targets(
                &condition,
                BasicBlockId::from_index(1),
                BasicBlockId::from_index(2),
            ),
            vec![BasicBlockId::from_index(2)]
        );
    }

    #[test]
    fn retains_both_edges_for_a_runtime_boolean() {
        let condition = Operand::Copy(crate::mir::Place::local(crate::mir::LocalId::from_index(0)));
        assert_eq!(
            switch_targets(
                &condition,
                BasicBlockId::from_index(1),
                BasicBlockId::from_index(2),
            ),
            vec![BasicBlockId::from_index(1), BasicBlockId::from_index(2)]
        );
    }
}
