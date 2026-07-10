use crate::ir::{Function, Instruction};
use std::collections::VecDeque;

pub(super) fn same_file_call_targets(function: &Function) -> VecDeque<String> {
    let mut targets = VecDeque::new();
    collect_same_file_call_targets(&function.instructions, &mut targets);
    targets
}

fn collect_same_file_call_targets(instructions: &[Instruction], targets: &mut VecDeque<String>) {
    for instruction in instructions {
        match instruction {
            Instruction::CallI32 { target, .. }
            | Instruction::CallBool { target, .. }
            | Instruction::TailCall { target, .. } => {
                if let Some(name) = target.same_file_name() {
                    targets.push_back(name.to_string());
                }
            }
            Instruction::If {
                then_instructions,
                else_instructions,
                ..
            } => {
                collect_same_file_call_targets(then_instructions, targets);
                collect_same_file_call_targets(else_instructions, targets);
            }
            Instruction::WriteStaticStderr(_)
            | Instruction::SetI32 { .. }
            | Instruction::SetBool { .. }
            | Instruction::AddI32 { .. }
            | Instruction::SubtractI32 { .. }
            | Instruction::MultiplyI32 { .. }
            | Instruction::DivideI32 { .. }
            | Instruction::RemainderI32 { .. }
            | Instruction::ShiftLeftI32 { .. }
            | Instruction::ShiftRightI32 { .. }
            | Instruction::Return => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BoolValue, CallTarget, Function, I32Location, I32Value, Instruction, Type};

    #[test]
    fn collects_same_file_call_targets_from_nested_instructions_in_order() {
        let function = Function {
            name: "main".to_string(),
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
                        arguments: vec![I32Value::Const(1)],
                    }],
                    else_instructions: vec![Instruction::TailCall {
                        target: CallTarget::same_file("else_target"),
                        arguments: vec![I32Value::Const(2)],
                    }],
                },
            ],
        };

        assert_eq!(
            same_file_call_targets(&function)
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                "first".to_string(),
                "then_target".to_string(),
                "else_target".to_string(),
            ]
        );
    }
}
