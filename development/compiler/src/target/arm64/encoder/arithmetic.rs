use super::*;

impl Encoder {
    pub(crate) fn emit_add_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(ADD_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_add_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ADD_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_adds_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ADDS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_adds_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(ADDS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_sub_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SUB_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_subs_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SUBS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_subs_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(SUBS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_mul_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(
            MADD_W_BASE | (rm.bits() << 16) | (WZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_mul_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(
            MADD_X_BASE | (rm.bits() << 16) | (XZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_sdiv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SDIV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_sdiv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(SDIV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_udiv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(UDIV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_udiv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(UDIV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lslv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(LSLV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lslv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(LSLV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lsrv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(LSRV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lsrv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(LSRV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lsl_x_imm(&mut self, rd: XReg, rn: XReg, shift: u32) {
        self.emit_word(lsl_x_imm_word(rd, rn, shift));
    }

    pub(crate) fn emit_lsr_x_imm(&mut self, rd: XReg, rn: XReg, shift: u32) {
        self.emit_word(lsr_x_imm_word(rd, rn, shift));
    }

    pub(crate) fn emit_asrv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ASRV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_asrv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(ASRV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_asr_x_imm(&mut self, rd: XReg, rn: XReg, shift: u32) {
        debug_assert!(shift < 64);
        let immr = shift;
        let imms = 63;
        self.emit_word(SBFM_X_BASE | (immr << 16) | (imms << 10) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_msub_w(&mut self, rd: WReg, rn: WReg, rm: WReg, ra: WReg) {
        self.emit_word(
            MSUB_W_BASE | (rm.bits() << 16) | (ra.bits() << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_msub_x(&mut self, rd: XReg, rn: XReg, rm: XReg, ra: XReg) {
        self.emit_word(
            MSUB_X_BASE | (rm.bits() << 16) | (ra.bits() << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_smull_x(&mut self, rd: XReg, rn: WReg, rm: WReg) {
        self.emit_word(
            SMADDL_X_BASE | (rm.bits() << 16) | (WZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_umulh_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(UMULH_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_smulh_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(SMULH_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_sxtw_x_w(&mut self, rd: XReg, rn: WReg) {
        self.emit_word(SXTW_X_BASE | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_sub_sp_imm(&mut self, byte_count: u32) {
        self.emit_word(add_sub_sp_imm_word(SUB_SP_IMM_BASE, byte_count));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_add_sp_imm(&mut self, byte_count: u32) {
        self.emit_word(add_sub_sp_imm_word(ADD_SP_IMM_BASE, byte_count));
    }

    pub(crate) fn emit_add_x_sp_imm(&mut self, rd: XReg, byte_count: u32) {
        self.emit_word(add_x_sp_imm_word(rd, byte_count));
    }

    pub(crate) fn emit_add_x_imm(&mut self, rd: XReg, rn: XReg, byte_count: u32) {
        self.emit_word(add_x_imm_word(rd, rn, byte_count));
    }

    pub(crate) fn emit_cmp_w(&mut self, rn: WReg, rm: WReg) {
        self.emit_word(SUBS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_w_imm(&mut self, rn: WReg, value: u32) {
        debug_assert!(value <= 0x0fff);
        self.emit_word(SUBS_W_IMM_BASE | (value << 10) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_w_zero(&mut self, rn: WReg) {
        self.emit_word(SUBS_W_BASE | (WZR_BITS << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_x(&mut self, rn: XReg, rm: XReg) {
        self.emit_word(SUBS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_x_zero(&mut self, rn: XReg) {
        self.emit_word(SUBS_X_BASE | (XZR_BITS << 16) | (rn.bits() << 5) | XZR_BITS);
    }
}
