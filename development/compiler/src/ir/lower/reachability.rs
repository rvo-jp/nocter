use crate::ir::{CallTarget, FallibleFailureMode, Function, Instruction};
use std::collections::VecDeque;

pub(super) fn reachable_call_targets(function: &Function) -> VecDeque<CallTarget> {
    let mut targets = VecDeque::new();
    collect_reachable_call_targets(&function.instructions, &mut targets);
    targets
}

fn collect_reachable_call_targets(
    instructions: &[Instruction],
    targets: &mut VecDeque<CallTarget>,
) {
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { target, .. }
            | Instruction::CallU8 { target, .. }
            | Instruction::CallUsize { target, .. }
            | Instruction::CallBool { target, .. }
            | Instruction::CallStr { target, .. }
            | Instruction::CallSlice { target, .. }
            | Instruction::CallAggregate { target, .. }
            | Instruction::CallDirectAggregate { target, .. }
            | Instruction::CallVoid { target, .. }
            | Instruction::TailCall { target, .. } => {
                targets.push_back(target.clone());
            }
            Instruction::CallFallibleAggregate {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleDirectAggregate {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleI32 {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleU8 {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleUsize {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleBool {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleStr {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleSlice {
                target,
                failure_mode,
                ..
            }
            | Instruction::CallFallibleVoid {
                target,
                failure_mode,
                ..
            } => {
                targets.push_back(target.clone());
                collect_failure_mode_reachable_call_targets(failure_mode, targets);
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                collect_reachable_call_targets(then_instructions, targets);
                collect_reachable_call_targets(else_instructions, targets);
            }
            Instruction::While {
                condition_instructions,
                body_instructions,
                ..
            } => {
                collect_reachable_call_targets(condition_instructions, targets);
                collect_reachable_call_targets(body_instructions, targets);
            }
            Instruction::CheckFailure { failure_mode } => {
                collect_failure_mode_reachable_call_targets(failure_mode, targets);
            }
            Instruction::ReadSlice { failure_mode, .. } => {
                collect_failure_mode_reachable_call_targets(failure_mode, targets);
            }
            Instruction::OpenRead { failure_mode, .. } => {
                collect_failure_mode_reachable_call_targets(failure_mode, targets);
            }
            Instruction::ProcessExit { .. }
            | Instruction::WriteStr { .. }
            | Instruction::WriteSlice { .. }
            | Instruction::CloseFd { .. }
            | Instruction::DarwinSyscall { .. }
            | Instruction::CopyStrToPointer { .. }
            | Instruction::CopyPointerBytes { .. }
            | Instruction::CopyAggregateToPointer { .. }
            | Instruction::CopyPointerToAggregate { .. }
            | Instruction::LoadU8FromPointer { .. }
            | Instruction::LoadI32FromPointer { .. }
            | Instruction::LoadUsizeFromPointer { .. }
            | Instruction::LoadBoolFromPointer { .. }
            | Instruction::LoadStrFromPointer { .. }
            | Instruction::CopySliceElementToAggregate { .. }
            | Instruction::CopyAggregateToSliceElement { .. }
            | Instruction::StoreU8ToPointer { .. }
            | Instruction::StoreI32ToPointer { .. }
            | Instruction::StoreUsizeToPointer { .. }
            | Instruction::StoreBoolToPointer { .. }
            | Instruction::StoreStrToPointer { .. }
            | Instruction::StoreU8ToSliceIndex { .. }
            | Instruction::StoreI32ToSliceIndex { .. }
            | Instruction::StoreUsizeToSliceIndex { .. }
            | Instruction::StoreBoolToSliceIndex { .. }
            | Instruction::StoreStrToSliceIndex { .. }
            | Instruction::ReserveAggregateSlot { .. }
            | Instruction::StoreAggregateUsize { .. }
            | Instruction::StoreAggregateI32 { .. }
            | Instruction::StoreAggregateU16 { .. }
            | Instruction::StoreAggregateU32 { .. }
            | Instruction::StoreAggregateU8 { .. }
            | Instruction::StoreAggregateBool { .. }
            | Instruction::StoreAggregateUsizeIndexed { .. }
            | Instruction::StoreAggregateI32Indexed { .. }
            | Instruction::StoreAggregateU8Indexed { .. }
            | Instruction::StoreAggregateBoolIndexed { .. }
            | Instruction::LoadAggregateUsize { .. }
            | Instruction::LoadAggregateI32 { .. }
            | Instruction::LoadAggregateU8 { .. }
            | Instruction::LoadAggregateBool { .. }
            | Instruction::LoadAggregateUsizeIndexed { .. }
            | Instruction::LoadAggregateI32Indexed { .. }
            | Instruction::LoadAggregateU8Indexed { .. }
            | Instruction::LoadAggregateBoolIndexed { .. }
            | Instruction::CopyAggregate { .. }
            | Instruction::CopyAggregateRange { .. }
            | Instruction::PropagateFailure
            | Instruction::TrapOnFailure
            | Instruction::ReturnFallibleSuccess
            | Instruction::ReturnOptionalNone
            | Instruction::ReturnFallibleFailure { .. }
            | Instruction::SetI32 { .. }
            | Instruction::SetU8 { .. }
            | Instruction::SetUsize { .. }
            | Instruction::RegionEnter { .. }
            | Instruction::SetCurrentAllocationContext { .. }
            | Instruction::RegionRelease { .. }
            | Instruction::SetUsizeFromBorrow { .. }
            | Instruction::SetBool { .. }
            | Instruction::SetStr { .. }
            | Instruction::SetStrRawParts { .. }
            | Instruction::SetSlice { .. }
            | Instruction::SetSliceRawParts { .. }
            | Instruction::AddU8 { .. }
            | Instruction::SubtractU8 { .. }
            | Instruction::MultiplyU8 { .. }
            | Instruction::DivideU8 { .. }
            | Instruction::RemainderU8 { .. }
            | Instruction::ShiftLeftU8 { .. }
            | Instruction::ShiftRightU8 { .. }
            | Instruction::AddI32 { .. }
            | Instruction::SubtractI32 { .. }
            | Instruction::MultiplyI32 { .. }
            | Instruction::DivideI32 { .. }
            | Instruction::RemainderI32 { .. }
            | Instruction::ShiftLeftI32 { .. }
            | Instruction::ShiftRightI32 { .. }
            | Instruction::AddUsize { .. }
            | Instruction::SubtractUsize { .. }
            | Instruction::MultiplyUsize { .. }
            | Instruction::DivideUsize { .. }
            | Instruction::RemainderUsize { .. }
            | Instruction::ShiftLeftUsize { .. }
            | Instruction::ShiftRightUsize { .. }
            | Instruction::Trap
            | Instruction::Break
            | Instruction::Continue
            | Instruction::Return => {}
        }
    }
}

fn collect_failure_mode_reachable_call_targets(
    failure_mode: &FallibleFailureMode,
    targets: &mut VecDeque<CallTarget>,
) {
    match failure_mode {
        FallibleFailureMode::Propagate | FallibleFailureMode::Trap => {}
        FallibleFailureMode::PropagateWithCleanup { instructions, .. }
        | FallibleFailureMode::Handle { instructions }
        | FallibleFailureMode::Recover { instructions }
        | FallibleFailureMode::Catch { instructions, .. } => {
            collect_reachable_call_targets(instructions, targets);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BoolValue, CallTarget, FallibleFailureMode, Function, I32Location, I32Value, Instruction,
        ScalarArgument, StrLocation, Type,
    };

    #[test]
    fn collects_reachable_call_targets_from_nested_instructions_in_order() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![
                Instruction::CallI32 {
                    destination: I32Location::Local(0),
                    target: CallTarget::same_file("first"),
                    arguments: vec![],
                },
                Instruction::If {
                    condition: BoolValue::Const(true),
                    then_instructions: vec![Instruction::CallBool {
                        destination: crate::ir::BoolLocation::Local(0),
                        target: CallTarget::same_file("then_target"),
                        arguments: vec![ScalarArgument::I32(I32Value::Const(1))],
                    }],
                    else_instructions: vec![Instruction::TailCall {
                        target: CallTarget::same_file("else_target"),
                        arguments: vec![ScalarArgument::I32(I32Value::Const(2))],
                    }],
                },
            ],
        };

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                CallTarget::same_file("first"),
                CallTarget::same_file("then_target"),
                CallTarget::same_file("else_target"),
            ]
        );
    }

    #[test]
    fn collects_reachable_call_targets_from_while_conditions_and_bodies() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::While {
                condition_instructions: vec![Instruction::CallBool {
                    destination: crate::ir::BoolLocation::Local(0),
                    target: CallTarget::same_file("condition_target"),
                    arguments: vec![],
                }],
                condition: BoolValue::Location(crate::ir::BoolLocation::Local(0)),
                body_instructions: vec![Instruction::CallVoid {
                    target: CallTarget::same_file("body_target"),
                    arguments: vec![ScalarArgument::I32(I32Value::Const(1))],
                }],
            }],
        };

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                CallTarget::same_file("condition_target"),
                CallTarget::same_file("body_target"),
            ]
        );
    }

    #[test]
    fn keeps_imported_reachable_call_targets() {
        let source = crate::source::SourceId::new(7);
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallI32 {
                destination: I32Location::Local(0),
                target: CallTarget::imported(source, "answer"),
                arguments: vec![],
            }],
        };

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![CallTarget::imported(source, "answer")]
        );
    }

    #[test]
    fn collects_reachable_call_targets_from_aggregate_calls() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::Void,
            instructions: vec![Instruction::CallAggregate {
                destination: crate::ir::AggregateLocation::Slot(0),
                target: CallTarget::same_file("make"),
                arguments: vec![],
            }],
        };

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![CallTarget::same_file("make")]
        );
    }

    #[test]
    fn collects_reachable_call_targets_from_catch_failure_mode() {
        let function = Function {
            name: "main".to_string(),
            target: crate::ir::CallTarget::same_file("main".to_string()),
            return_type: Type::I32,
            instructions: vec![Instruction::CallFallibleI32 {
                destination: I32Location::Return,
                target: CallTarget::same_file("answer"),
                arguments: vec![],
                failure_mode: FallibleFailureMode::Catch {
                    code: StrLocation::Local(0),
                    message: StrLocation::Local(2),
                    instructions: vec![Instruction::CallI32 {
                        destination: I32Location::Return,
                        target: CallTarget::same_file("recover"),
                        arguments: vec![],
                    }],
                },
            }],
        };

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                CallTarget::same_file("answer"),
                CallTarget::same_file("recover"),
            ]
        );
    }
}
