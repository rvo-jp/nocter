use crate::abi::ReturnPassing;
use crate::backend::frame::{FrameLayout, FunctionFrame, plan_function_frame};
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::ir::{
    BorrowSource, CallTarget, DirectAggregateArgument, FallibleFailureMode, Function, I32Location,
    I32Value, Instruction, IrModule, ScalarArgument, SliceValue, StrLocation, StrValue, Type,
    UsizeLocation, UsizeValue,
};
use crate::target::arm64::{BranchCondition, Encoder, MoveWideShift, WReg, XReg};
use std::collections::HashMap;

mod abi_shapes;
mod calls;
mod control_flow;
mod emission;
mod entry;
mod error_payloads;
mod fallible;
mod finalization;
mod frames;
mod instructions;
mod io_runtime;
mod locations;
mod process_arguments;
mod runtime;
mod symbols;
mod validation;
mod values;

use abi_shapes::*;
use emission::*;
use process_arguments::*;
use runtime::*;
use symbols::*;
use validation::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineCode {
    pub(crate) text: Vec<u8>,
    pub(crate) read_only_data: Vec<u8>,
}

pub(crate) fn generate_arm64_darwin_entry(
    module: &IrModule,
) -> Result<MachineCode, Vec<Diagnostic>> {
    let mut emitter = EntryEmitter::new();
    emitter.emit_module(module)?;
    emitter.finish()
}

#[derive(Debug, Default)]
struct EntryEmitter {
    encoder: Encoder,
    read_only_data: Vec<u8>,
    data_address_patches: Vec<DataAddressPatch>,
    function_offsets: HashMap<FunctionSymbol, usize>,
    call_patches: Vec<FunctionCallPatch>,
    tail_call_patches: Vec<FunctionCallPatch>,
    loop_contexts: Vec<LoopContext>,
    current_frame_size: Option<u32>,
    current_parameter_spill_offsets: HashMap<usize, u32>,
    current_scalar_spill_offsets: HashMap<usize, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LoopContext {
    start_offset: usize,
    break_branches: Vec<control_flow::BranchPatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StaticErrorPayload {
    code: &'static [u8],
    message: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DarwinErrnoPayload {
    errno: i32,
    payload: StaticErrorPayload,
}

#[cfg(test)]
mod tests;
