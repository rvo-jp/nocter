use super::*;

const DARWIN_MMAP_SYSCALL: u32 = 0x0200_00c5;
const DARWIN_MUNMAP_SYSCALL: u32 = 0x0200_0049;
const REGION_STATE_BYTES: u64 = 8;
const REGION_ALLOCATION_ABORT_STATUS: i32 = 70;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_region_enter(
        &mut self,
        destination: UsizeLocation,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(frame) = frame {
            self.emit_scalar_spills(frame)?;
        }

        emit_mov_u64_to_x(&mut self.encoder, XReg::X0, 0);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X1, REGION_STATE_BYTES);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X2, 3);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X3, 0x1002);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X4, u64::MAX);
        emit_mov_u64_to_x(&mut self.encoder, XReg::X5, 0);
        emit_mov_u32_to_w(&mut self.encoder, WReg::W16, DARWIN_MMAP_SYSCALL);
        self.encoder.emit_svc(DARWIN_SYSCALL_TRAP);

        let success = self.emit_cond_branch_placeholder(BranchCondition::Cc);
        emit_mov_i32_to_w(&mut self.encoder, WReg::W0, REGION_ALLOCATION_ABORT_STATUS);
        emit_darwin_exit_syscall(&mut self.encoder);
        self.patch_branch_placeholder_to_current(success, "region state allocation success")?;

        emit_mov_u64_to_x(&mut self.encoder, XReg::X16, 0);
        self.encoder.emit_str_x_imm(XReg::X16, XReg::X0, 0);
        if let Some(frame) = frame {
            self.emit_scalar_reloads(frame)?;
        }
        self.emit_x_to_usize_location(XReg::X0, destination)?;
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_set_current_allocation_context(
        &mut self,
        state: &UsizeValue,
        kind: &UsizeValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.emit_usize_value_to_x(state, XReg::X20)?;
        self.emit_usize_value_to_x(kind, XReg::X21)
    }

    pub(in crate::backend::codegen) fn emit_region_release(
        &mut self,
        state: &UsizeValue,
        parent_state: &UsizeValue,
        parent_kind: &UsizeValue,
        frame: Option<&FrameLayout>,
    ) -> Result<(), Vec<Diagnostic>> {
        if let Some(frame) = frame {
            self.emit_scalar_spills(frame)?;
        }

        self.emit_usize_value_to_x(state, XReg::X16)?;
        self.encoder.emit_ldr_x_imm(XReg::X8, XReg::X16, 0);
        let loop_start = self.encoder.position();
        self.encoder.emit_cmp_x_zero(XReg::X8);
        let allocations_released = self.emit_cond_branch_placeholder(BranchCondition::Eq);

        self.encoder.emit_ldr_x_imm(XReg::X2, XReg::X8, 0);
        self.encoder.emit_ldr_x_imm(XReg::X1, XReg::X8, 8);
        self.encoder.emit_mov_x(XReg::X0, XReg::X8);
        emit_mov_u32_to_w(&mut self.encoder, WReg::W16, DARWIN_MUNMAP_SYSCALL);
        self.encoder.emit_svc(DARWIN_SYSCALL_TRAP);
        self.encoder.emit_mov_x(XReg::X8, XReg::X2);
        let repeat = self.emit_branch_placeholder();
        self.patch_branch_placeholder_to_offset(repeat, loop_start, "region release loop")?;

        self.patch_branch_placeholder_to_current(
            allocations_released,
            "region allocation release end",
        )?;
        self.emit_usize_value_to_x(state, XReg::X0)?;
        emit_mov_u64_to_x(&mut self.encoder, XReg::X1, REGION_STATE_BYTES);
        emit_mov_u32_to_w(&mut self.encoder, WReg::W16, DARWIN_MUNMAP_SYSCALL);
        self.encoder.emit_svc(DARWIN_SYSCALL_TRAP);

        self.emit_usize_value_to_x(parent_state, XReg::X20)?;
        self.emit_usize_value_to_x(parent_kind, XReg::X21)?;
        if let Some(frame) = frame {
            self.emit_scalar_reloads(frame)?;
        }
        Ok(())
    }
}

pub(super) fn module_uses_allocation_context(module: &IrModule) -> bool {
    module
        .functions
        .iter()
        .any(|function| instructions_use_allocation_context(&function.instructions))
}

fn instructions_use_allocation_context(instructions: &[Instruction]) -> bool {
    instructions.iter().any(|instruction| match instruction {
        Instruction::RegionEnter { .. }
        | Instruction::SetCurrentAllocationContext { .. }
        | Instruction::RegionRelease { .. } => true,
        Instruction::SetUsize { value, .. } | Instruction::StoreAggregateUsize { value, .. } => {
            usize_value_uses_allocation_context(value)
        }
        Instruction::StoreAggregateUsizeIndexed { index, value, .. } => {
            usize_value_uses_allocation_context(index) || usize_value_uses_allocation_context(value)
        }
        Instruction::If {
            then_instructions,
            else_instructions,
            ..
        } => {
            instructions_use_allocation_context(then_instructions)
                || instructions_use_allocation_context(else_instructions)
        }
        Instruction::While {
            condition_instructions,
            body_instructions,
            ..
        } => {
            instructions_use_allocation_context(condition_instructions)
                || instructions_use_allocation_context(body_instructions)
        }
        _ => false,
    })
}

fn usize_value_uses_allocation_context(value: &UsizeValue) -> bool {
    matches!(
        value,
        UsizeValue::CurrentAllocationState | UsizeValue::CurrentAllocationKind
    )
}
