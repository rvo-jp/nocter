use super::*;

impl Encoder {
    pub(crate) fn emit_b(&mut self, byte_offset: i32) {
        self.emit_word(b_word(byte_offset));
    }

    pub(crate) fn emit_b_cond(&mut self, condition: BranchCondition, byte_offset: i32) {
        self.emit_word(b_cond_word(condition, byte_offset));
    }

    pub(crate) fn emit_bl(&mut self, byte_offset: i32) {
        self.emit_word(bl_word(byte_offset));
    }

    pub(crate) fn emit_ret(&mut self) {
        self.emit_word(RET_X30);
    }

    pub(crate) fn emit_svc(&mut self, imm16: u16) {
        self.emit_word(SVC_BASE | ((imm16 as u32) << 5));
    }

    pub(crate) fn emit_brk(&mut self, imm16: u16) {
        self.emit_word(BRK_BASE | ((imm16 as u32) << 5));
    }

    pub(crate) fn patch_adr_x(&mut self, instruction_offset: usize, rd: XReg, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = adr_x_word(rd, byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_bl(&mut self, instruction_offset: usize, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = bl_word(byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_b(&mut self, instruction_offset: usize, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = b_word(byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_b_cond(
        &mut self,
        instruction_offset: usize,
        condition: BranchCondition,
        byte_offset: i32,
    ) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = b_cond_word(condition, byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }
}
