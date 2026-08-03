use super::control_flow::branch_condition_for_true_comparison;
use super::*;
use crate::abi::ValueLayout;
use crate::ir::{
    AggregateArgumentSource, AggregateLocation, BoolLocation, BoolValue, BorrowArgument,
    BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, Function,
    I32ComparisonOperator, I32Location, I32Value, ScalarArgument, SliceElementAddressKind,
    SliceElementIndex, SliceLocation, SliceValue, StrLocation, StrValue, Type, U8Location, U8Value,
    UsizeLocation, UsizeValue,
};
use crate::source::SourceId;
use crate::target::arm64::BranchCondition;

mod aggregate_calls;
mod aggregate_copies;
mod aggregate_fields;
mod arithmetic;
mod calls;
mod control_flow;
mod entry_validation;
mod io_runtime;
mod memory_stores;
mod outcome_values;
mod region_runtime;
mod symbols;
mod views;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn write_temp_executable(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let unique = format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let executable = std::env::temp_dir().join(unique);
    std::fs::write(&executable, bytes).unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();
    executable
}

fn set_return_i32(value: i32) -> Instruction {
    Instruction::SetI32 {
        destination: I32Location::Return,
        value: i32_const(value),
    }
}

fn set_return_usize(value: u64) -> Instruction {
    Instruction::SetUsize {
        destination: UsizeLocation::Return,
        value: usize_const(value),
    }
}

fn set_return_bool(value: bool) -> Instruction {
    Instruction::SetBool {
        destination: BoolLocation::Return,
        value: BoolValue::Const(value),
    }
}

fn tail_call(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::TailCall {
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_i32(destination: I32Location, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallI32 {
        destination,
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_void(function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallVoid {
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn call_bool(destination: BoolLocation, function: &str, arguments: Vec<I32Value>) -> Instruction {
    Instruction::CallBool {
        destination,
        target: CallTarget::same_file(function),
        arguments: i32_arguments(arguments),
    }
}

fn i32_arguments(arguments: Vec<I32Value>) -> Vec<ScalarArgument> {
    arguments.into_iter().map(ScalarArgument::I32).collect()
}

fn i32_const(value: i32) -> I32Value {
    I32Value::Const(value)
}

fn i32_param(index: usize) -> I32Value {
    I32Value::Location(I32Location::Parameter(index))
}

fn i32_local(index: usize) -> I32Value {
    I32Value::Location(I32Location::Local(index))
}

fn u8_const(value: u8) -> U8Value {
    U8Value::Const(value)
}

fn readonly_u8_slice_type() -> Type {
    Type::Slice {
        is_readwrite: false,
    }
}

fn usize_const(value: u64) -> UsizeValue {
    UsizeValue::Const(value)
}

fn contains_instruction(text: &[u8], instruction: [u8; 4]) -> bool {
    text.windows(4).any(|window| window == instruction)
}

fn instruction_offset(text: &[u8], instruction: &[u8]) -> Option<usize> {
    text.windows(instruction.len())
        .position(|window| window == instruction)
}

fn contains_backward_unconditional_branch(text: &[u8]) -> bool {
    text.chunks_exact(4).any(|chunk| {
        let word = u32::from_le_bytes(chunk.try_into().unwrap());
        let is_unconditional_branch = (word & 0xfc00_0000) == 0x1400_0000;
        let offset_is_negative = (word & 0x0200_0000) != 0;
        is_unconditional_branch && offset_is_negative
    })
}

fn contains_bytes(text: &[u8], bytes: &[u8]) -> bool {
    text.windows(bytes.len()).any(|window| window == bytes)
}

fn encoded_mov_u32_to_w(register: WReg, value: u32) -> Vec<u8> {
    let mut encoder = Encoder::new();
    emit_mov_u32_to_w(&mut encoder, register, value);
    encoder.finish()
}

fn encoded_ldr_x_sp(register: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_x_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_x_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_mov_x(destination: XReg, source: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_mov_x(destination, source);
    encoded_instruction(encoder)
}

fn encoded_mov_w(destination: WReg, source: WReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_mov_w(destination, source);
    encoded_instruction(encoder)
}

fn encoded_lsl_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_lsl_x_imm(destination, source, shift);
    encoded_instruction(encoder)
}

fn encoded_adds_x(destination: XReg, left: XReg, right: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_adds_x(destination, left, right);
    encoded_instruction(encoder)
}

fn encoded_lsr_x_imm(destination: XReg, source: XReg, shift: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_lsr_x_imm(destination, source, shift);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_reg(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_ldr_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldr_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldrb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldrb_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_ldrb_w_reg(register: WReg, base: XReg, offset: XReg) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_ldrb_w_reg(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_str_x_sp(register: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_x_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_str_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_strb_w_sp(register: WReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strb_w_sp(register, offset);
    encoded_instruction(encoder)
}

fn encoded_str_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_strb_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strb_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_strh_w_imm(register: WReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_strh_w_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_str_x_imm(register: XReg, base: XReg, offset: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_str_x_imm(register, base, offset);
    encoded_instruction(encoder)
}

fn encoded_instruction(encoder: Encoder) -> [u8; 4] {
    encoder.finish().try_into().unwrap()
}
