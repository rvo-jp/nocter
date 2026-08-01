use super::*;

impl Encoder {
    #[allow(dead_code)]
    pub(crate) fn emit_str_x_sp(&mut self, rt: XReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STR_X_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            8,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_ldr_x_sp(&mut self, rt: XReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDR_X_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            8,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_str_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STR_W_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            4,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_ldr_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDR_W_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_strb_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STRB_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_ldrb_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDRB_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_strh_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STRH_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            2,
        ));
    }

    pub(crate) fn emit_ldrh_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDRH_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            2,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_str_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STR_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_ldr_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDR_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_strb_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STRB_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_ldrb_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDRB_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_strh_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STRH_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            2,
        ));
    }

    pub(crate) fn emit_ldrh_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDRH_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            2,
        ));
    }

    pub(crate) fn emit_str_x_imm(&mut self, rt: XReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STR_X_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            8,
        ));
    }

    pub(crate) fn emit_ldr_x_imm(&mut self, rt: XReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDR_X_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            8,
        ));
    }

    pub(crate) fn emit_ldrb_w_reg(&mut self, rt: WReg, rn: XReg, rm: XReg) {
        self.emit_word(LDRB_W_REG_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rt.bits());
    }

    pub(crate) fn emit_ldr_w_reg(&mut self, rt: WReg, rn: XReg, rm: XReg) {
        self.emit_word(LDR_W_REG_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rt.bits());
    }
}
