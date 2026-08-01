use super::*;

impl EntryEmitter {
    pub(in crate::backend::codegen) fn emit_prologue(
        &mut self,
        frame: &FrameLayout,
    ) -> Result<(), Vec<Diagnostic>> {
        self.encoder.emit_sub_sp_imm(frame.frame_size());
        self.encoder
            .emit_str_x_sp(XReg::X30, frame.saved_x30_offset());
        if let Some(offset) = frame.indirect_return_pointer_offset() {
            self.encoder.emit_str_x_sp(XReg::X8, offset);
        }
        for slot in frame.parameter_spill_slots() {
            self.emit_unspilled_parameter_word_to_x(slot.parameter_index(), XReg::X16)?;
            self.encoder.emit_str_x_sp(XReg::X16, slot.offset());
        }
        Ok(())
    }

    pub(in crate::backend::codegen) fn emit_epilogue(&mut self, frame: &FrameLayout) {
        self.encoder
            .emit_ldr_x_sp(XReg::X30, frame.saved_x30_offset());
        self.encoder.emit_add_sp_imm(frame.frame_size());
    }

    pub(in crate::backend::codegen) fn emit_return(&mut self, frame: Option<&FrameLayout>) {
        if let Some(frame) = frame {
            self.emit_epilogue(frame);
        }
        self.encoder.emit_ret();
    }

    pub(in crate::backend::codegen) fn emit_indirect_return_pointer_to_x8(
        &mut self,
        frame: Option<&FrameLayout>,
    ) {
        if let Some(offset) = frame.and_then(FrameLayout::indirect_return_pointer_offset) {
            self.encoder.emit_ldr_x_sp(XReg::X8, offset);
        }
    }
}
