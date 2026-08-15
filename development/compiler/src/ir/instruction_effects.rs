//! Structural machine-IR instruction effects used by generic analyses.
//!
//! Code generation still matches concrete instructions. Analyses that only
//! need calls or nested instruction regions use this projection, so adding a
//! control-flow or call instruction has one exhaustive maintenance point.

use super::{CallTarget, Instruction, OutcomeFailureMode, ScalarArgument};

#[derive(Debug, Clone, Copy)]
pub(crate) struct InstructionEffects<'a> {
    call_target: Option<&'a CallTarget>,
    call_argument_words: usize,
    nested: [Option<&'a [Instruction]>; 3],
}

impl<'a> InstructionEffects<'a> {
    pub(crate) fn call_target(&self) -> Option<&CallTarget> {
        self.call_target
    }

    pub(crate) fn call_argument_words(&self) -> usize {
        self.call_argument_words
    }

    fn nested(&self) -> [Option<&'a [Instruction]>; 3] {
        self.nested
    }
}

impl Instruction {
    pub(crate) fn effects(&self) -> InstructionEffects<'_> {
        let none = [None, None, None];
        match self {
            Self::CallI32 {
                target, arguments, ..
            }
            | Self::CallU8 {
                target, arguments, ..
            }
            | Self::CallUsize {
                target, arguments, ..
            }
            | Self::CallBorrow {
                target, arguments, ..
            }
            | Self::CallBool {
                target, arguments, ..
            }
            | Self::CallStr {
                target, arguments, ..
            }
            | Self::CallSlice {
                target, arguments, ..
            }
            | Self::CallAggregate {
                target, arguments, ..
            }
            | Self::CallDirectAggregate {
                target, arguments, ..
            }
            | Self::CallVoid {
                target, arguments, ..
            }
            | Self::CallStoredOutcome {
                target, arguments, ..
            }
            | Self::TailCall { target, arguments } => InstructionEffects {
                call_target: Some(target),
                call_argument_words: argument_words(arguments),
                nested: none,
            },
            Self::CallOutcomeI32 {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeU8 {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeUsize {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeBorrow {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeBool {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeStr {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeSlice {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeDirectAggregate {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeAggregate {
                target,
                arguments,
                failure_mode,
                ..
            }
            | Self::CallOutcomeVoid {
                target,
                arguments,
                failure_mode,
            } => InstructionEffects {
                call_target: Some(target),
                call_argument_words: argument_words(arguments),
                nested: [failure_instructions(failure_mode), None, None],
            },
            Self::CallComposedOutcome {
                target,
                arguments,
                outer_mode,
                inner_mode,
                ..
            } => InstructionEffects {
                call_target: Some(target),
                call_argument_words: argument_words(arguments),
                nested: [
                    failure_instructions(outer_mode),
                    failure_instructions(inner_mode),
                    None,
                ],
            },
            Self::If {
                then_instructions,
                else_instructions,
                ..
            } => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: [Some(then_instructions), Some(else_instructions), None],
            },
            Self::IfStoredOutcomeTag {
                success_instructions,
                outcome_instructions,
                ..
            } => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: [Some(success_instructions), Some(outcome_instructions), None],
            },
            Self::CheckStoredFallible {
                success_instructions,
                failure_mode,
                ..
            } => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: [
                    Some(success_instructions),
                    failure_instructions(failure_mode),
                    None,
                ],
            },
            Self::While {
                condition_instructions,
                body_instructions,
                ..
            } => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: [Some(condition_instructions), Some(body_instructions), None],
            },
            Self::CheckFailure { failure_mode }
            | Self::ReadSlice { failure_mode, .. }
            | Self::OpenRead { failure_mode, .. } => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: [failure_instructions(failure_mode), None, None],
            },
            Self::DarwinSyscall { arguments, .. } => InstructionEffects {
                call_target: None,
                call_argument_words: arguments.len() + 1,
                nested: none,
            },
            Self::ProcessExit { .. }
            | Self::WriteStr { .. }
            | Self::WriteSlice { .. }
            | Self::CloseFd { .. }
            | Self::CopyStrToPointer { .. }
            | Self::CopyPointerBytes { .. }
            | Self::CopyAggregateToPointer { .. }
            | Self::CopyPointerToAggregate { .. }
            | Self::LoadU8FromPointer { .. }
            | Self::LoadI32FromPointer { .. }
            | Self::LoadUsizeFromPointer { .. }
            | Self::LoadIntegerFromPointer { .. }
            | Self::LoadBoolFromPointer { .. }
            | Self::LoadStrFromPointer { .. }
            | Self::CopySliceElementToAggregate { .. }
            | Self::CopyAggregateToSliceElement { .. }
            | Self::StoreU8ToPointer { .. }
            | Self::StoreI32ToPointer { .. }
            | Self::StoreUsizeToPointer { .. }
            | Self::StoreIntegerToPointer { .. }
            | Self::StoreBoolToPointer { .. }
            | Self::StoreStrToPointer { .. }
            | Self::StoreU8ToSliceIndex { .. }
            | Self::StoreI32ToSliceIndex { .. }
            | Self::StoreUsizeToSliceIndex { .. }
            | Self::StoreIntegerToSliceIndex { .. }
            | Self::StoreBoolToSliceIndex { .. }
            | Self::StoreStrToSliceIndex { .. }
            | Self::ReserveAggregateSlot { .. }
            | Self::StoreAggregateUsize { .. }
            | Self::StoreAggregateInteger { .. }
            | Self::StoreAggregateIntegerIndexed { .. }
            | Self::StoreAggregateI32 { .. }
            | Self::StoreAggregateU16 { .. }
            | Self::StoreAggregateU32 { .. }
            | Self::StoreAggregateU8 { .. }
            | Self::StoreAggregateBool { .. }
            | Self::StoreAggregateUsizeIndexed { .. }
            | Self::StoreAggregateI32Indexed { .. }
            | Self::StoreAggregateU8Indexed { .. }
            | Self::StoreAggregateBoolIndexed { .. }
            | Self::LoadAggregateUsize { .. }
            | Self::LoadAggregateInteger { .. }
            | Self::LoadAggregateIntegerIndexed { .. }
            | Self::LoadAggregateI32 { .. }
            | Self::LoadAggregateU8 { .. }
            | Self::LoadAggregateBool { .. }
            | Self::LoadAggregateUsizeIndexed { .. }
            | Self::LoadAggregateI32Indexed { .. }
            | Self::LoadAggregateU8Indexed { .. }
            | Self::LoadAggregateBoolIndexed { .. }
            | Self::CopyAggregate { .. }
            | Self::CopyAggregateRange { .. }
            | Self::CopyAggregateProjected { .. }
            | Self::PropagateFailure
            | Self::TrapOnFailure
            | Self::ReturnOutcomeSuccess
            | Self::ReturnOptionalNone
            | Self::ReturnFallibleFailure { .. }
            | Self::LoadStoredOutcomePayload { .. }
            | Self::ReturnStoredOutcome { .. }
            | Self::SetI32 { .. }
            | Self::SetU8 { .. }
            | Self::SetUsize { .. }
            | Self::RegionEnter { .. }
            | Self::SetCurrentAllocationContext { .. }
            | Self::RegionRelease { .. }
            | Self::SetUsizeFromBorrow { .. }
            | Self::SetBool { .. }
            | Self::SetStr { .. }
            | Self::SetStrSubview { .. }
            | Self::SetStrRawParts { .. }
            | Self::SetSlice { .. }
            | Self::SetSliceRawParts { .. }
            | Self::U8Binary { .. }
            | Self::I32Binary { .. }
            | Self::UsizeBinary { .. }
            | Self::IntegerBinary { .. }
            | Self::Trap
            | Self::Break
            | Self::Continue
            | Self::Return => InstructionEffects {
                call_target: None,
                call_argument_words: 0,
                nested: none,
            },
        }
    }
}

pub(crate) fn visit_instruction_tree<'a>(
    instructions: &'a [Instruction],
    visitor: &mut impl FnMut(&'a Instruction),
) {
    for instruction in instructions {
        visitor(instruction);
        let effects = instruction.effects();
        for nested in effects.nested().into_iter().flatten() {
            visit_instruction_tree(nested, visitor);
        }
    }
}

fn argument_words(arguments: &[ScalarArgument]) -> usize {
    arguments.iter().map(ScalarArgument::abi_word_count).sum()
}

fn failure_instructions(mode: &OutcomeFailureMode) -> Option<&[Instruction]> {
    match mode {
        OutcomeFailureMode::Propagate | OutcomeFailureMode::Trap => None,
        OutcomeFailureMode::PropagateWithCleanup { instructions, .. }
        | OutcomeFailureMode::Handle { instructions }
        | OutcomeFailureMode::Recover { instructions }
        | OutcomeFailureMode::Catch { instructions, .. } => Some(instructions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BoolLocation, BoolValue};

    #[test]
    fn visits_nested_regions_in_source_order() {
        let instructions = [Instruction::If {
            condition: BoolValue::Const(true),
            then_instructions: vec![Instruction::SetBool {
                destination: BoolLocation::Local(1),
                value: BoolValue::Const(true),
            }],
            else_instructions: vec![Instruction::SetBool {
                destination: BoolLocation::Local(2),
                value: BoolValue::Const(false),
            }],
        }];
        let mut visited = Vec::new();
        visit_instruction_tree(&instructions, &mut |instruction| {
            if let Instruction::SetBool {
                destination: BoolLocation::Local(index),
                ..
            } = instruction
            {
                visited.push(*index);
            }
        });
        assert_eq!(visited, [1, 2]);
    }
}
