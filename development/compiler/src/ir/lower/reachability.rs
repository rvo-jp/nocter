use crate::ir::{CallTarget, Function, visit_instruction_tree};
use std::collections::VecDeque;

pub(super) fn reachable_call_targets(function: &Function) -> VecDeque<CallTarget> {
    let mut targets = VecDeque::new();
    visit_instruction_tree(&function.instructions, &mut |instruction| {
        if let Some(target) = instruction.effects().call_target() {
            targets.push_back(target.clone());
        }
    });
    targets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BoolLocation, BoolValue, I32Location, Instruction, OutcomeFailureMode, StrLocation, Type,
    };

    fn function(instructions: Vec<Instruction>) -> Function {
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions,
        }
    }

    #[test]
    fn collects_nested_control_flow_targets_in_source_order() {
        let function = function(vec![Instruction::If {
            condition: BoolValue::Const(true),
            then_instructions: vec![Instruction::CallBool {
                destination: BoolLocation::Local(0),
                target: CallTarget::same_file("then"),
                arguments: vec![],
            }],
            else_instructions: vec![Instruction::TailCall {
                target: CallTarget::same_file("else"),
                arguments: vec![],
            }],
        }]);

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            [CallTarget::same_file("then"), CallTarget::same_file("else")]
        );
    }

    #[test]
    fn collects_targets_from_failure_regions() {
        let function = function(vec![Instruction::CallOutcomeI32 {
            destination: I32Location::Return,
            target: CallTarget::same_file("attempt"),
            arguments: vec![],
            failure_mode: OutcomeFailureMode::Catch {
                code: StrLocation::Local(0),
                message: StrLocation::Local(2),
                instructions: vec![Instruction::CallI32 {
                    destination: I32Location::Return,
                    target: CallTarget::same_file("recover"),
                    arguments: vec![],
                }],
                recovers: false,
            },
        }]);

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            [
                CallTarget::same_file("attempt"),
                CallTarget::same_file("recover")
            ]
        );
    }

    #[test]
    fn preserves_imported_target_identity() {
        let source = crate::source::SourceId::new(7);
        let function = function(vec![Instruction::CallI32 {
            destination: I32Location::Return,
            target: CallTarget::imported(source, "answer"),
            arguments: vec![],
        }]);

        assert_eq!(
            reachable_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            [CallTarget::imported(source, "answer")]
        );
    }
}
