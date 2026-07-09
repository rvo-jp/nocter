use crate::diagnostics::Diagnostic;
use crate::ir::{BoolLocation, BoolValue, Function, I32Location, I32Value, Instruction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FunctionFrame {
    Frameless,
    Framed(FrameLayout),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FrameLayout {
    frame_size: u32,
    saved_x30_offset: u32,
    scalar_spill_slots: Vec<ScalarSpillSlot>,
}

impl FrameLayout {
    pub(super) fn frame_size(&self) -> u32 {
        self.frame_size
    }

    pub(super) fn saved_x30_offset(&self) -> u32 {
        self.saved_x30_offset
    }

    pub(super) fn scalar_spill_slots(&self) -> &[ScalarSpillSlot] {
        &self.scalar_spill_slots
    }

    pub(super) fn for_scalar_spill_slot_count(count: usize) -> Result<Self, Vec<Diagnostic>> {
        let scalar_spill_bytes = count.checked_mul(SCALAR_SPILL_SLOT_SIZE).ok_or_else(|| {
            frame_too_large_diagnostic("scalar spill slot count overflows host usize")
        })?;
        let unaligned_frame_size = scalar_spill_bytes
            .checked_add(SAVED_X30_SLOT_SIZE)
            .ok_or_else(|| frame_too_large_diagnostic("frame size overflows host usize"))?;
        let frame_size = align_usize(unaligned_frame_size, STACK_ALIGNMENT);

        if frame_size > ADD_SUB_SP_IMM_MAX as usize {
            return Err(frame_too_large_diagnostic(
                "frame size exceeds ARM64 add/sub immediate range",
            ));
        }

        let saved_x30_offset = frame_size - SAVED_X30_SLOT_SIZE;
        if saved_x30_offset > LDR_STR_X_SP_MAX_BYTE_OFFSET as usize {
            return Err(frame_too_large_diagnostic(
                "saved x30 slot exceeds ARM64 x-register load/store immediate range",
            ));
        }

        let mut scalar_spill_slots = Vec::with_capacity(count);
        for local_index in 0..count {
            let offset = local_index * SCALAR_SPILL_SLOT_SIZE;
            if offset > LDR_STR_W_SP_MAX_BYTE_OFFSET as usize {
                return Err(frame_too_large_diagnostic(
                    "scalar spill slot exceeds ARM64 w-register load/store immediate range",
                ));
            }
            scalar_spill_slots.push(ScalarSpillSlot {
                local_index,
                offset: offset as u32,
            });
        }

        Ok(Self {
            frame_size: frame_size as u32,
            saved_x30_offset: saved_x30_offset as u32,
            scalar_spill_slots,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarSpillSlot {
    local_index: usize,
    offset: u32,
}

impl ScalarSpillSlot {
    pub(super) fn local_index(self) -> usize {
        self.local_index
    }

    pub(super) fn offset(self) -> u32 {
        self.offset
    }
}

pub(super) fn plan_function_frame(function: &Function) -> Result<FunctionFrame, Vec<Diagnostic>> {
    if !function_requires_frame(&function.instructions) {
        return Ok(FunctionFrame::Frameless);
    }

    FrameLayout::for_scalar_spill_slot_count(scalar_spill_slot_count(&function.instructions))
        .map(FunctionFrame::Framed)
}

fn function_requires_frame(instructions: &[Instruction]) -> bool {
    instructions.iter().any(instruction_requires_frame)
}

fn instruction_requires_frame(instruction: &Instruction) -> bool {
    match instruction {
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            function_requires_frame(then_instructions) || function_requires_frame(else_instructions)
        }
        Instruction::CallI32 { .. } => true,
        Instruction::WriteStaticStderr(_)
        | Instruction::SetI32 { .. }
        | Instruction::SetBool { .. }
        | Instruction::AddI32 { .. }
        | Instruction::TailCall { .. }
        | Instruction::Return => false,
    }
}

fn scalar_spill_slot_count(instructions: &[Instruction]) -> usize {
    let mut highest_local_index = None;
    record_instruction_list_scalar_locals(instructions, &mut highest_local_index);
    highest_local_index.map_or(0, |index| index + 1)
}

fn record_instruction_list_scalar_locals(
    instructions: &[Instruction],
    highest_local_index: &mut Option<usize>,
) {
    for instruction in instructions {
        record_instruction_scalar_locals(instruction, highest_local_index);
    }
}

fn record_instruction_scalar_locals(
    instruction: &Instruction,
    highest_local_index: &mut Option<usize>,
) {
    match instruction {
        Instruction::WriteStaticStderr(_) | Instruction::TailCall { .. } | Instruction::Return => {}
        Instruction::SetI32 { destination, value } => {
            record_i32_location(*destination, highest_local_index);
            record_i32_value(value, highest_local_index);
        }
        Instruction::SetBool { destination, value } => {
            record_bool_location(*destination, highest_local_index);
            record_bool_value(value, highest_local_index);
        }
        Instruction::AddI32 {
            destination,
            left,
            right,
        } => {
            record_i32_location(*destination, highest_local_index);
            record_i32_value(left, highest_local_index);
            record_i32_value(right, highest_local_index);
        }
        Instruction::CallI32 {
            destination,
            arguments,
            ..
        } => {
            record_i32_location(*destination, highest_local_index);
            for argument in arguments {
                record_i32_value(argument, highest_local_index);
            }
        }
        Instruction::If {
            condition,
            then_instructions,
            else_instructions,
        } => {
            record_bool_value(condition, highest_local_index);
            record_instruction_list_scalar_locals(then_instructions, highest_local_index);
            record_instruction_list_scalar_locals(else_instructions, highest_local_index);
        }
    }
}

fn record_i32_value(value: &I32Value, highest_local_index: &mut Option<usize>) {
    match value {
        I32Value::Const(_) => {}
        I32Value::Location(location) => record_i32_location(*location, highest_local_index),
    }
}

fn record_i32_location(location: I32Location, highest_local_index: &mut Option<usize>) {
    if let I32Location::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_bool_value(value: &BoolValue, highest_local_index: &mut Option<usize>) {
    match value {
        BoolValue::Const(_) => {}
        BoolValue::Location(location) => record_bool_location(*location, highest_local_index),
        BoolValue::Not(inner) => record_bool_value(inner, highest_local_index),
        BoolValue::Logical { left, right, .. } => {
            record_bool_value(left, highest_local_index);
            record_bool_value(right, highest_local_index);
        }
        BoolValue::I32Comparison { left, right, .. } => {
            record_i32_value(left, highest_local_index);
            record_i32_value(right, highest_local_index);
        }
        BoolValue::BoolComparison { left, right, .. } => {
            record_bool_value(left, highest_local_index);
            record_bool_value(right, highest_local_index);
        }
    }
}

fn record_bool_location(location: BoolLocation, highest_local_index: &mut Option<usize>) {
    if let BoolLocation::Local(index) = location {
        record_scalar_local(index, highest_local_index);
    }
}

fn record_scalar_local(index: usize, highest_local_index: &mut Option<usize>) {
    *highest_local_index = Some(highest_local_index.map_or(index, |highest| highest.max(index)));
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn frame_too_large_diagnostic(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E9005",
        format!("function stack frame is too large for backend v0: {reason}"),
    )]
}

const STACK_ALIGNMENT: usize = 16;
const SCALAR_SPILL_SLOT_SIZE: usize = 4;
const SAVED_X30_SLOT_SIZE: usize = 8;
const ADD_SUB_SP_IMM_MAX: u32 = 0x00ff_f000;
const LDR_STR_W_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 4;
const LDR_STR_X_SP_MAX_BYTE_OFFSET: u32 = 0x0fff * 8;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BoolComparisonOperator, Type};

    #[test]
    fn plans_current_ir_functions_as_frameless() {
        let function = Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetI32 {
                    destination: I32Location::Local(0),
                    value: I32Value::Const(40),
                },
                Instruction::TailCall {
                    function: "answer".to_string(),
                    arguments: vec![I32Value::Location(I32Location::Local(0))],
                },
            ],
        };

        assert_eq!(
            plan_function_frame(&function).unwrap(),
            FunctionFrame::Frameless
        );
    }

    #[test]
    fn computes_aligned_frame_with_saved_x30_only() {
        let layout = FrameLayout::for_scalar_spill_slot_count(0).unwrap();

        assert_eq!(layout.frame_size(), 16);
        assert_eq!(layout.saved_x30_offset(), 8);
        assert!(layout.scalar_spill_slots().is_empty());
    }

    #[test]
    fn computes_scalar_spill_slots_below_saved_x30() {
        let layout = FrameLayout::for_scalar_spill_slot_count(3).unwrap();

        assert_eq!(layout.frame_size(), 32);
        assert_eq!(layout.saved_x30_offset(), 24);
        assert_eq!(
            layout.scalar_spill_slots(),
            &[
                ScalarSpillSlot {
                    local_index: 0,
                    offset: 0
                },
                ScalarSpillSlot {
                    local_index: 1,
                    offset: 4
                },
                ScalarSpillSlot {
                    local_index: 2,
                    offset: 8
                },
            ]
        );
    }

    #[test]
    fn counts_scalar_slots_from_nested_i32_and_bool_locals() {
        let instructions = vec![Instruction::If {
            condition: BoolValue::BoolComparison {
                operator: BoolComparisonOperator::Equal,
                left: Box::new(BoolValue::Location(BoolLocation::Local(1))),
                right: Box::new(BoolValue::Const(true)),
            },
            then_instructions: vec![Instruction::AddI32 {
                destination: I32Location::Local(3),
                left: I32Value::Location(I32Location::Local(0)),
                right: I32Value::Const(1),
            }],
            else_instructions: vec![Instruction::SetBool {
                destination: BoolLocation::Local(2),
                value: BoolValue::Const(false),
            }],
        }];

        assert_eq!(scalar_spill_slot_count(&instructions), 4);
    }

    #[test]
    fn call_i32_requires_frame_and_counts_destination_and_argument_locals() {
        let function = Function {
            name: "main".to_string(),
            return_type: Type::I32,
            instructions: vec![Instruction::CallI32 {
                destination: I32Location::Local(2),
                function: "answer".to_string(),
                arguments: vec![I32Value::Location(I32Location::Local(1))],
            }],
        };

        let frame = plan_function_frame(&function).unwrap();

        assert_eq!(
            frame,
            FunctionFrame::Framed(FrameLayout::for_scalar_spill_slot_count(3).unwrap())
        );
    }

    #[test]
    fn rejects_frame_when_w_spill_offset_is_not_encodable() {
        let error = FrameLayout::for_scalar_spill_slot_count(4097).unwrap_err();

        assert_eq!(error[0].code, "E9005");
    }
}
