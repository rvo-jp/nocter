use super::*;

impl Encoder {
    pub(crate) fn emit_movz_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        debug_assert!(shift.is_valid_for_wide_32());
        self.emit_word(MOVZ_W_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_movk_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        debug_assert!(shift.is_valid_for_wide_32());
        self.emit_word(MOVK_W_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_mov_w(&mut self, rd: WReg, rm: WReg) {
        self.emit_word(ORR_W_BASE | (rm.bits() << 16) | (WZR_BITS << 5) | rd.bits());
    }

    pub(crate) fn emit_mov_x(&mut self, rd: XReg, rm: XReg) {
        self.emit_word(ORR_X_BASE | (rm.bits() << 16) | (XZR_BITS << 5) | rd.bits());
    }

    pub(crate) fn emit_orr_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(ORR_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_movz_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVZ_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_movk_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVK_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_adr_x(&mut self, rd: XReg, byte_offset: i32) {
        self.emit_word(adr_x_word(rd, byte_offset));
    }
}
